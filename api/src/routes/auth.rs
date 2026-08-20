//! Login, refresh token, profil, ganti kata sandi.
//!
//! IDENTITAS LOGIN JARGON GO
//!   * Siswa                                   -> NISN (10 digit)
//!   * Guru, staff, kepala sekolah, orang tua,
//!     dinas                                   -> NIK (16 digit)
//!   * Dashboard `/admin`                      -> juga menerima username/email
//!
//! Pendaftaran akun TIDAK swalayan: seluruh akun dibuat operator lewat
//! `/admin/users` atau lewat pembuatan massal akun siswa. Karena itu tidak ada
//! endpoint register di sini — hanya login.

use axum::extract::State;
use axum::routing::{get, post};
use axum::Router;
use chrono::{Duration as ChronoDuration, Utc};
use uuid::Uuid;

use crate::auth::jwt::AccessTokenInput;
use crate::auth::{password, AuthUser, STUDENT_SCOPED_ROLES};
use crate::domain::user::{
    ChangePasswordRequest, HomeroomRef, LinkedStudent, LoginRequest, LoginResponse, RefreshRequest,
    UserProfile,
};
use crate::error::{ApiError, ApiResult};
use crate::extract::ValidJson;
use crate::face::vector;
use crate::services::audit::AuditEntry;
use crate::state::AppState;
use crate::util::ApiResponse;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
        .route("/auth/change-password", post(change_password))
}

#[derive(Debug, sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    name: String,
    username: String,
    email: String,
    password: String,
    identity_number: Option<String>,
    identity_type: Option<String>,
    student_id: Option<Uuid>,
    avatar: Option<String>,
    phone: Option<String>,
    position: Option<String>,
    employee_no: Option<String>,
    school_id: Option<Uuid>,
    school_name: Option<String>,
    is_active: bool,
    banned_at: Option<chrono::DateTime<Utc>>,
    must_change_password: bool,
    last_login: Option<chrono::DateTime<Utc>>,
}

/// Kolom yang diambil untuk setiap pembacaan profil.
const USER_COLUMNS: &str = r#"
    u.id, u.name, u.username, u.email, u.password,
    u.identity_number, u.identity_type, u.student_id,
    u.avatar, u.phone, u.position, u.employee_no,
    u.school_id, s.name AS school_name,
    u.is_active, u.banned_at, u.must_change_password, u.last_login
"#;

/// Login pengguna Jargon GO dan dashboard.
#[utoipa::path(
    post, path = "/v1/auth/login", tag = "Autentikasi",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Berhasil login", body = LoginResponse),
        (status = 401, description = "NIK/NISN atau kata sandi salah"),
        (status = 403, description = "Akun nonaktif atau diblokir"),
        (status = 422, description = "Input tidak valid")
    )
)]
pub async fn login(
    State(state): State<AppState>,
    ValidJson(body): ValidJson<LoginRequest>,
) -> ApiResult<ApiResponse<LoginResponse>> {
    let identifier = body.identifier.trim().to_string();
    let kind = body.identity_kind();

    // Rate limit per identitas: mencegah percobaan kata sandi bertubi-tubi
    // tanpa mengganggu pengguna lain yang sedang login normal.
    state
        .rate_limit(
            &format!("login:{}", identifier.to_lowercase()),
            10,
            std::time::Duration::from_secs(300),
        )
        .await?;

    // Satu query untuk semua bentuk identitas. `identity_number` diperiksa
    // apa adanya (angka), sedangkan username/email tidak peka huruf besar.
    let sql = format!(
        r#"
        SELECT {USER_COLUMNS}
        FROM users u
        LEFT JOIN schools s ON s.id = u.school_id
        WHERE u.deleted_at IS NULL
          AND (u.identity_number = $1 OR LOWER(u.username) = $2 OR LOWER(u.email) = $2)
        LIMIT 1
        "#
    );

    let user: Option<UserRow> = sqlx::query_as(&sql)
        .bind(&identifier)
        .bind(identifier.to_lowercase())
        .fetch_optional(&state.db)
        .await?;

    // Pesan seragam untuk akun tidak ada maupun kata sandi salah, supaya
    // tidak bisa dipakai menebak NIK/NISN mana yang terdaftar. Sebutan
    // jenis identitas tetap disertakan agar pengguna tahu ia salah kotak.
    let invalid = || {
        ApiError::Unauthorized(format!(
            "{} atau kata sandi salah",
            kind.label()
        ))
    };

    let Some(user) = user else {
        return Err(invalid());
    };
    if !password::verify_password(&body.password, &user.password) {
        return Err(invalid());
    }
    if !user.is_active {
        return Err(ApiError::Forbidden(
            "Akun Anda dinonaktifkan. Hubungi operator sekolah atau Dinas.".into(),
        ));
    }
    if let Some(banned_at) = user.banned_at {
        return Err(ApiError::Forbidden(format!(
            "Akun Anda diblokir sejak {}.",
            banned_at.format("%d-%m-%Y")
        )));
    }

    let (roles, permissions) = load_roles_and_permissions(&state, user.id).await?;
    if roles.is_empty() {
        return Err(ApiError::Forbidden(
            "Akun Anda belum memiliki peran. Hubungi administrator.".into(),
        ));
    }

    let extra_schools = load_extra_schools(&state, user.id).await?;
    let students = load_linked_students(&state, &user, &roles).await?;

    // Akun siswa/orang tua yang belum tertaut ke data siswa tidak akan bisa
    // melihat apa pun. Menggagalkan login di sini memberi pesan yang jauh
    // lebih berguna daripada aplikasi yang terbuka tapi kosong.
    if roles.iter().any(|r| STUDENT_SCOPED_ROLES.contains(&r.as_str())) && students.is_empty() {
        return Err(ApiError::Forbidden(
            "Akun Anda belum ditautkan ke data siswa. Hubungi operator sekolah \
             untuk melengkapinya."
                .into(),
        ));
    }

    let (access_token, expires_at) = state.jwt.issue_access(
        AccessTokenInput {
            user_id: user.id,
            username: user.username.clone(),
            name: user.name.clone(),
            identity: user.identity_number.clone(),
            school_id: user.school_id,
            scopes: extra_schools,
            students: students.iter().map(|s| s.id).collect(),
            roles: roles.clone(),
            perms: permissions.clone(),
        },
        state.cfg.access_token_ttl,
    )?;

    let refresh_token = issue_refresh_token(&state, user.id, body.device_name.as_deref()).await?;

    sqlx::query("UPDATE users SET last_login = NOW() WHERE id = $1")
        .bind(user.id)
        .execute(&state.db)
        .await?;

    let profile = build_profile(&state, user, roles, permissions, students).await?;

    Ok(ApiResponse::with_message(
        LoginResponse {
            access_token,
            refresh_token,
            token_type: "Bearer",
            expires_at,
            user: profile,
        },
        "Berhasil masuk",
    ))
}

/// Tukar refresh token dengan access token baru (rotasi token).
#[utoipa::path(
    post, path = "/v1/auth/refresh", tag = "Autentikasi",
    request_body = RefreshRequest,
    responses(
        (status = 200, description = "Token diperbarui", body = LoginResponse),
        (status = 401, description = "Refresh token tidak valid / kedaluwarsa")
    )
)]
pub async fn refresh(
    State(state): State<AppState>,
    ValidJson(body): ValidJson<RefreshRequest>,
) -> ApiResult<ApiResponse<LoginResponse>> {
    let hash = vector::sha256(body.refresh_token.as_bytes());

    let row: Option<(Uuid, Uuid)> = sqlx::query_as(
        r#"
        SELECT id, user_id FROM refresh_tokens
        WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > NOW()
        "#,
    )
    .bind(&hash)
    .fetch_optional(&state.db)
    .await?;

    let Some((token_id, user_id)) = row else {
        return Err(ApiError::Unauthorized(
            "Sesi Anda sudah berakhir, silakan masuk kembali".into(),
        ));
    };

    let user = fetch_user_row(&state, user_id)
        .await?
        .ok_or_else(|| ApiError::Unauthorized("Akun tidak ditemukan".into()))?;

    if !user.is_active || user.banned_at.is_some() {
        // Pencabutan hak berlaku seketika pada saat refresh, meski access
        // token lama masih berlaku sampai kedaluwarsa.
        sqlx::query("UPDATE refresh_tokens SET revoked_at = NOW() WHERE id = $1")
            .bind(token_id)
            .execute(&state.db)
            .await?;
        return Err(ApiError::Forbidden("Akun Anda tidak aktif".into()));
    }

    let (roles, permissions) = load_roles_and_permissions(&state, user.id).await?;
    let extra_schools = load_extra_schools(&state, user.id).await?;
    let students = load_linked_students(&state, &user, &roles).await?;

    let (access_token, expires_at) = state.jwt.issue_access(
        AccessTokenInput {
            user_id: user.id,
            username: user.username.clone(),
            name: user.name.clone(),
            identity: user.identity_number.clone(),
            school_id: user.school_id,
            scopes: extra_schools,
            students: students.iter().map(|s| s.id).collect(),
            roles: roles.clone(),
            perms: permissions.clone(),
        },
        state.cfg.access_token_ttl,
    )?;

    // Rotasi: token lama dicabut dan digantikan yang baru. Bila token lama
    // dipakai lagi setelah ini, itu indikasi pencurian token.
    let new_refresh = issue_refresh_token(&state, user.id, None).await?;
    sqlx::query("UPDATE refresh_tokens SET revoked_at = NOW() WHERE id = $1")
        .bind(token_id)
        .execute(&state.db)
        .await?;

    let profile = build_profile(&state, user, roles, permissions, students).await?;

    Ok(ApiResponse::new(LoginResponse {
        access_token,
        refresh_token: new_refresh,
        token_type: "Bearer",
        expires_at,
        user: profile,
    }))
}

/// Cabut refresh token (logout).
#[utoipa::path(
    post, path = "/v1/auth/logout", tag = "Autentikasi",
    request_body = RefreshRequest,
    security(("bearer" = [])),
    responses((status = 200, description = "Berhasil keluar"))
)]
pub async fn logout(
    State(state): State<AppState>,
    user: AuthUser,
    ValidJson(body): ValidJson<RefreshRequest>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    let hash = vector::sha256(body.refresh_token.as_bytes());
    sqlx::query(
        r#"
        UPDATE refresh_tokens SET revoked_at = NOW()
        WHERE token_hash = $1 AND user_id = $2 AND revoked_at IS NULL
        "#,
    )
    .bind(&hash)
    .bind(user.id)
    .execute(&state.db)
    .await?;

    Ok(ApiResponse::with_message(
        serde_json::json!({ "logged_out": true }),
        "Berhasil keluar",
    ))
}

/// Profil pengguna yang sedang masuk, termasuk izin dan siswa yang tertaut.
#[utoipa::path(
    get, path = "/v1/auth/me", tag = "Autentikasi",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Profil pengguna", body = UserProfile),
        (status = 401, description = "Belum terautentikasi")
    )
)]
pub async fn me(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<ApiResponse<UserProfile>> {
    let row = fetch_user_row(&state, user.id)
        .await?
        .ok_or_else(|| ApiError::NotFound("pengguna".into()))?;

    let (roles, permissions) = load_roles_and_permissions(&state, user.id).await?;
    let students = load_linked_students(&state, &row, &roles).await?;
    let profile = build_profile(&state, row, roles, permissions, students).await?;

    Ok(ApiResponse::new(profile))
}

/// Ganti kata sandi sendiri.
#[utoipa::path(
    post, path = "/v1/auth/change-password", tag = "Autentikasi",
    request_body = ChangePasswordRequest,
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Kata sandi diperbarui"),
        (status = 401, description = "Kata sandi saat ini salah")
    )
)]
pub async fn change_password(
    State(state): State<AppState>,
    user: AuthUser,
    ValidJson(body): ValidJson<ChangePasswordRequest>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    let (current_hash,): (String,) = sqlx::query_as("SELECT password FROM users WHERE id = $1")
        .bind(user.id)
        .fetch_one(&state.db)
        .await?;

    if !password::verify_password(&body.current_password, &current_hash) {
        return Err(ApiError::Unauthorized("Kata sandi saat ini salah".into()));
    }
    if password::verify_password(&body.new_password, &current_hash) {
        return Err(ApiError::field(
            "new_password",
            "kata sandi baru tidak boleh sama dengan yang sekarang",
        ));
    }

    let new_hash = password::hash_password(&body.new_password)?;
    sqlx::query(
        r#"
        UPDATE users
           SET password = $2, must_change_password = FALSE, updated_at = NOW()
         WHERE id = $1
        "#,
    )
    .bind(user.id)
    .bind(&new_hash)
    .execute(&state.db)
    .await?;

    // Semua sesi lain dipaksa keluar: kalau kata sandi diganti karena diduga
    // bocor, sesi penyerang harus ikut mati.
    sqlx::query(
        r#"
        UPDATE refresh_tokens SET revoked_at = NOW()
        WHERE user_id = $1 AND revoked_at IS NULL
        "#,
    )
    .bind(user.id)
    .execute(&state.db)
    .await?;

    AuditEntry::by_user(&user, "auth.change_password")
        .entity("user", user.id)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        serde_json::json!({ "changed": true, "other_sessions_revoked": true }),
        "Kata sandi berhasil diperbarui. Silakan masuk kembali di perangkat lain.",
    ))
}

// =====================================================================
// Helper
// =====================================================================

async fn fetch_user_row(state: &AppState, user_id: Uuid) -> ApiResult<Option<UserRow>> {
    let sql = format!(
        r#"
        SELECT {USER_COLUMNS}
        FROM users u
        LEFT JOIN schools s ON s.id = u.school_id
        WHERE u.id = $1 AND u.deleted_at IS NULL
        "#
    );
    let row: Option<UserRow> = sqlx::query_as(&sql)
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?;
    Ok(row)
}

async fn build_profile(
    state: &AppState,
    row: UserRow,
    roles: Vec<String>,
    permissions: Vec<String>,
    students: Vec<LinkedStudent>,
) -> ApiResult<UserProfile> {
    let homeroom_classrooms = load_homerooms(state, row.id).await?;

    let role_label = match roles.first().map(String::as_str) {
        Some("superadmin") => "Superadmin",
        Some("admin_dinas") => "Admin Dinas",
        Some("petugas_pengaduan") => "Petugas Pengaduan",
        Some("kepala_sekolah") => "Kepala Sekolah",
        Some("guru") => "Guru",
        Some("staff_tu") => "Staff TU",
        Some("siswa") => "Siswa",
        Some("orang_tua") => "Orang Tua",
        _ => "Pengguna",
    };

    Ok(UserProfile {
        id: row.id,
        name: row.name,
        username: row.username,
        email: row.email,
        identity_number: row.identity_number,
        identity_type: row.identity_type,
        avatar_url: row.avatar.map(|a| state.storage.public_url(&a)),
        phone: row.phone,
        position: row.position,
        employee_no: row.employee_no,
        school_id: row.school_id,
        school_name: row.school_name,
        roles,
        role_label: role_label.to_string(),
        permissions,
        homeroom_classrooms,
        students,
        must_change_password: row.must_change_password,
        last_login: row.last_login,
    })
}

/// Ambil peran + izin efektif (izin dari peran ditambah izin langsung).
pub async fn load_roles_and_permissions(
    state: &AppState,
    user_id: Uuid,
) -> ApiResult<(Vec<String>, Vec<String>)> {
    let roles: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT r.name FROM roles r
        JOIN model_has_roles mhr ON mhr.role_id = r.id
        WHERE mhr.model_id = $1 AND mhr.model_type = 'App\Models\User'
        ORDER BY r.name
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    let permissions: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT p.name FROM permissions p
        WHERE p.id IN (
            SELECT rhp.permission_id
            FROM role_has_permissions rhp
            JOIN model_has_roles mhr ON mhr.role_id = rhp.role_id
            WHERE mhr.model_id = $1 AND mhr.model_type = 'App\Models\User'
          UNION
            SELECT mhp.permission_id
            FROM model_has_permissions mhp
            WHERE mhp.model_id = $1 AND mhp.model_type = 'App\Models\User'
        )
        ORDER BY p.name
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    Ok((
        roles.into_iter().map(|r| r.0).collect(),
        permissions.into_iter().map(|p| p.0).collect(),
    ))
}

async fn load_extra_schools(state: &AppState, user_id: Uuid) -> ApiResult<Vec<Uuid>> {
    let rows: Vec<(Uuid,)> =
        sqlx::query_as("SELECT school_id FROM user_school_scopes WHERE user_id = $1")
            .bind(user_id)
            .fetch_all(&state.db)
            .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

/// Siswa yang tertaut ke akun ini.
///
/// Dua sumber berbeda, sesuai perannya:
///   * `siswa`     -> `users.student_id`, satu baris
///   * `orang_tua` -> `student_guardians.user_id`, bisa lintas sekolah
///
/// Peran lain mengembalikan daftar kosong — dan justru daftar kosong itulah
/// yang membuat [`AuthUser::accessible_students`] mengembalikan `None`,
/// artinya tidak dibatasi pada siswa tertentu.
async fn load_linked_students(
    state: &AppState,
    user: &UserRow,
    roles: &[String],
) -> ApiResult<Vec<LinkedStudent>> {
    if !roles.iter().any(|r| STUDENT_SCOPED_ROLES.contains(&r.as_str())) {
        return Ok(Vec::new());
    }

    let rows: Vec<LinkedStudent> = sqlx::query_as(
        r#"
        -- Akun siswa: dirinya sendiri.
        SELECT st.id, st.full_name, st.nisn, st.nis, st.school_id,
               sc.name AS school_name, c.name AS classroom_name,
               'diri_sendiri'::text AS relation
        FROM students st
        JOIN schools sc ON sc.id = st.school_id
        LEFT JOIN classrooms c ON c.id = st.current_classroom_id
        WHERE st.id = $2 AND st.deleted_at IS NULL

        UNION

        -- Akun orang tua: anak-anak yang diwalinya.
        SELECT st.id, st.full_name, st.nisn, st.nis, st.school_id,
               sc.name AS school_name, c.name AS classroom_name,
               g.relation
        FROM student_guardians g
        JOIN students st ON st.id = g.student_id
        JOIN schools sc  ON sc.id = st.school_id
        LEFT JOIN classrooms c ON c.id = st.current_classroom_id
        WHERE g.user_id = $1 AND st.deleted_at IS NULL

        ORDER BY full_name
        "#,
    )
    .bind(user.id)
    .bind(user.student_id)
    .fetch_all(&state.db)
    .await?;

    Ok(rows)
}

async fn load_homerooms(state: &AppState, user_id: Uuid) -> ApiResult<Vec<HomeroomRef>> {
    let rows: Vec<HomeroomRef> = sqlx::query_as(
        r#"
        SELECT id, name, grade_level FROM classrooms
        WHERE homeroom_teacher_id = $1 AND deleted_at IS NULL AND is_active
        ORDER BY grade_level, name
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

async fn issue_refresh_token(
    state: &AppState,
    user_id: Uuid,
    device_name: Option<&str>,
) -> ApiResult<String> {
    let token = password::generate_token();
    let hash = vector::sha256(token.as_bytes());
    let expires_at =
        Utc::now() + ChronoDuration::seconds(state.cfg.refresh_token_ttl.as_secs() as i64);

    sqlx::query(
        r#"
        INSERT INTO refresh_tokens (user_id, token_hash, expires_at, user_agent)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(user_id)
    .bind(&hash)
    .bind(expires_at)
    .bind(device_name)
    .execute(&state.db)
    .await?;

    Ok(token)
}
