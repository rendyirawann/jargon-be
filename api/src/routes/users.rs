//! Manajemen akun pengguna (guru, staff TU, kepala sekolah, admin dinas).

use axum::extract::{Path, State};
use axum::routing::{delete, get, post};
use axum::Router;
use uuid::Uuid;

use crate::auth::{password, AuthUser, PROVINCE_ROLES, ROLE_SUPERADMIN};
use crate::domain::user::{
    BulkStudentAccountRequest, BulkStudentAccountResponse, CreateUserRequest, InitialCredential,
    LinkChildRequest, UpdateUserRequest, UserFilter, UserListItem, ASSIGNABLE_ROLES,
    SCHOOL_BOUND_ROLES,
};
use crate::error::{ApiError, ApiResult};
use crate::extract::{ValidJson, ValidQuery};
use crate::services::audit::AuditEntry;
use crate::state::AppState;
use crate::util::{ApiResponse, PageQuery, Paginated};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/users", get(list).post(create))
        .route("/users/{id}", get(detail).patch(update).delete(remove))
        .route("/users/{id}/ban", post(ban))
        .route("/users/{id}/unban", post(unban))
        .route("/users/{id}/children", post(link_child))
        .route("/users/{id}/children/{student_id}", delete(unlink_child))
        .route("/users/students/bulk", post(bulk_student_accounts))
}

/// Buat akun aplikasi untuk siswa secara massal.
///
/// Untuk 700.000 siswa, membuat akun satu per satu tidak mungkin. Endpoint ini
/// membuat akun bagi seluruh siswa aktif di satu kelas/sekolah dan
/// mengembalikan kata sandi awal **sekali** — nilai itu tidak pernah bisa
/// dilihat lagi, jadi harus dicetak atau diunduh saat itu juga.
///
/// Kata sandi dibuat acak, BUKAN diturunkan dari NISN atau tanggal lahir.
/// Keduanya tercetak pada dokumen sekolah dan diketahui teman sekelas —
/// memakainya sebagai kata sandi awal berarti setiap siswa bisa masuk ke akun
/// temannya pada hari pertama.
#[utoipa::path(
    post, path = "/v1/users/students/bulk", tag = "Pengguna",
    request_body = BulkStudentAccountRequest,
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Akun siswa dibuat", body = BulkStudentAccountResponse),
        (status = 403, description = "Tidak berwenang membuat akun aplikasi")
    )
)]
pub async fn bulk_student_accounts(
    State(state): State<AppState>,
    user: AuthUser,
    ValidJson(body): ValidJson<BulkStudentAccountRequest>,
) -> ApiResult<ApiResponse<BulkStudentAccountResponse>> {
    user.require_any(&["manage_app_account", "create_user"])?;
    let school_id = user.require_school(body.school_id)?;
    let limit = body.limit.unwrap_or(200).clamp(1, 1000);

    #[derive(sqlx::FromRow)]
    struct Candidate {
        id: Uuid,
        full_name: String,
        nisn: Option<String>,
        classroom_name: Option<String>,
        has_account: bool,
    }

    let candidates: Vec<Candidate> = sqlx::query_as(
        r#"
        SELECT s.id, s.full_name, s.nisn, c.name AS classroom_name,
               EXISTS (
                   SELECT 1 FROM users u
                   WHERE u.student_id = s.id AND u.deleted_at IS NULL
               ) AS has_account
        FROM students s
        LEFT JOIN classrooms c ON c.id = s.current_classroom_id
        WHERE s.school_id = $1
          AND s.deleted_at IS NULL
          AND s.status = 'aktif'
          AND ($2::uuid IS NULL OR s.current_classroom_id = $2)
        ORDER BY c.name NULLS LAST, s.full_name
        LIMIT $3
        "#,
    )
    .bind(school_id)
    .bind(body.classroom_id)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;

    let mut created = Vec::new();
    let mut skipped = 0usize;
    let mut notes = Vec::new();

    for c in candidates {
        if c.has_account {
            skipped += 1;
            if !body.skip_existing {
                notes.push(format!("{} sudah memiliki akun", c.full_name));
            }
            continue;
        }

        // NISN adalah identitas login siswa; tanpa itu akun tidak bisa dibuat.
        let Some(nisn) = c.nisn.clone().filter(|n| n.len() == 10) else {
            skipped += 1;
            notes.push(format!(
                "{} dilewati: NISN belum diisi atau tidak 10 digit",
                c.full_name
            ));
            continue;
        };

        let initial_password = password::generate_initial_password();
        let hash = password::hash_password(&initial_password)?;

        let insert = sqlx::query(
            r#"
            INSERT INTO users (
                name, username, email, password, school_id, student_id,
                identity_number, identity_type, is_active, must_change_password,
                email_verified_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,'nisn',TRUE,TRUE,NOW())
            "#,
        )
        .bind(&c.full_name)
        // Username teknis dari NISN; siswa tetap login memakai NISN.
        .bind(format!("siswa{nisn}"))
        .bind(format!("{nisn}@siswa.jargon.local"))
        .bind(&hash)
        .bind(school_id)
        .bind(c.id)
        .bind(&nisn)
        .execute(&state.db)
        .await;

        match insert {
            Ok(_) => {
                let (uid,): (Uuid,) =
                    sqlx::query_as("SELECT id FROM users WHERE student_id = $1")
                        .bind(c.id)
                        .fetch_one(&state.db)
                        .await?;

                let mut tx = state.db.begin().await?;
                assign_role(&mut tx, uid, "siswa").await?;
                tx.commit().await?;

                created.push(InitialCredential {
                    student_id: c.id,
                    full_name: c.full_name,
                    classroom_name: c.classroom_name,
                    nisn,
                    initial_password,
                });
            }
            Err(e) => {
                skipped += 1;
                notes.push(format!("{}: gagal dibuat ({e})", c.full_name));
            }
        }
    }

    AuditEntry::by_user(&user, "user.bulk_student_accounts")
        .school(school_id)
        .after(&serde_json::json!({
            "created": created.len(),
            "skipped": skipped,
        }))
        .write(&state.db)
        .await;

    let message = format!(
        "{} akun siswa dibuat, {skipped} dilewati. Kata sandi awal hanya \
         ditampilkan sekali — unduh atau cetak sekarang.",
        created.len()
    );

    Ok(ApiResponse::with_message(
        BulkStudentAccountResponse {
            created: created.len(),
            skipped,
            credentials: created,
            notes,
        },
        message,
    ))
}

/// Daftar pengguna.
#[utoipa::path(
    get, path = "/v1/users", tag = "Pengguna",
    params(PageQuery, UserFilter),
    security(("bearer" = [])),
    responses((status = 200, description = "Daftar pengguna", body = [UserListItem]))
)]
pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(page): ValidQuery<PageQuery>,
    ValidQuery(filter): ValidQuery<UserFilter>,
) -> ApiResult<Paginated<UserListItem>> {
    user.require_any(&["view_user", "view_resources"])?;
    let scoped = user.accessible_schools();
    let search = page.search_pattern();

    let (total,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(DISTINCT u.id)::bigint
        FROM users u
        LEFT JOIN model_has_roles mhr
               ON mhr.model_id = u.id AND mhr.model_type = 'App\Models\User'
        LEFT JOIN roles r ON r.id = mhr.role_id
        WHERE u.deleted_at IS NULL
          AND ($1::uuid[] IS NULL OR u.school_id = ANY($1))
          AND ($2::uuid IS NULL OR u.school_id = $2)
          AND ($3::text IS NULL OR r.name = $3)
          AND ($4::boolean IS NULL OR u.is_active = $4)
          AND ($5::text IS NULL OR u.name ILIKE $5 OR u.username ILIKE $5 OR u.email ILIKE $5)
        "#,
    )
    .bind(scoped.as_deref())
    .bind(filter.school_id)
    .bind(filter.role.as_deref())
    .bind(filter.is_active)
    .bind(search.as_deref())
    .fetch_one(&state.db)
    .await?;

    let items: Vec<UserListItem> = sqlx::query_as(
        r#"
        SELECT u.id, u.name, u.username, u.email, u.phone, u.position,
               u.school_id, s.name AS school_name,
               COALESCE(
                   ARRAY(
                       SELECT r.name FROM roles r
                       JOIN model_has_roles m2 ON m2.role_id = r.id
                       WHERE m2.model_id = u.id AND m2.model_type = 'App\Models\User'
                       ORDER BY r.name
                   ),
                   ARRAY[]::text[]
               ) AS roles,
               u.is_active,
               (u.banned_at IS NOT NULL) AS is_banned,
               u.last_login, u.created_at
        FROM users u
        LEFT JOIN schools s ON s.id = u.school_id
        WHERE u.deleted_at IS NULL
          AND ($1::uuid[] IS NULL OR u.school_id = ANY($1))
          AND ($2::uuid IS NULL OR u.school_id = $2)
          AND ($3::text IS NULL OR EXISTS (
                SELECT 1 FROM roles r
                JOIN model_has_roles m3 ON m3.role_id = r.id
                WHERE m3.model_id = u.id AND m3.model_type = 'App\Models\User' AND r.name = $3
          ))
          AND ($4::boolean IS NULL OR u.is_active = $4)
          AND ($5::text IS NULL OR u.name ILIKE $5 OR u.username ILIKE $5 OR u.email ILIKE $5)
        ORDER BY u.name
        LIMIT $6 OFFSET $7
        "#,
    )
    .bind(scoped.as_deref())
    .bind(filter.school_id)
    .bind(filter.role.as_deref())
    .bind(filter.is_active)
    .bind(search.as_deref())
    .bind(page.per_page())
    .bind(page.offset())
    .fetch_all(&state.db)
    .await?;

    Ok(Paginated::new(items, page.page(), page.per_page(), total))
}

/// Detail pengguna.
#[utoipa::path(
    get, path = "/v1/users/{id}", tag = "Pengguna",
    params(("id" = Uuid, Path, description = "ID pengguna")),
    security(("bearer" = [])),
    responses((status = 200, description = "Detail pengguna", body = UserListItem))
)]
pub async fn detail(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<UserListItem>> {
    user.require_any(&["view_user", "view_resources"])?;
    let target = fetch(&state, id).await?;
    ensure_can_manage(&user, target.school_id)?;
    Ok(ApiResponse::new(target))
}

/// Buat akun pengguna.
///
/// Kepala sekolah/staff hanya boleh membuat akun untuk sekolahnya sendiri dan
/// tidak boleh membuat peran tingkat provinsi — itu wewenang Superadmin.
#[utoipa::path(
    post, path = "/v1/users", tag = "Pengguna",
    request_body = CreateUserRequest,
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Pengguna dibuat", body = UserListItem),
        (status = 403, description = "Tidak berwenang memberi peran tersebut"),
        (status = 409, description = "Username/email sudah dipakai")
    )
)]
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    ValidJson(body): ValidJson<CreateUserRequest>,
) -> ApiResult<ApiResponse<UserListItem>> {
    user.require("create_user")?;
    body.validate_role_scope()?;

    // Hanya Superadmin boleh mencetak peran bercakupan provinsi. Tanpa
    // penjagaan ini, seorang staff TU bisa mengangkat dirinya jadi dinas.
    if PROVINCE_ROLES.contains(&body.role.as_str()) && !user.is_superadmin() {
        return Err(ApiError::Forbidden(format!(
            "Hanya Superadmin yang dapat membuat akun dengan peran `{}`",
            body.role
        )));
    }
    if let Some(school_id) = body.school_id {
        ensure_can_manage(&user, Some(school_id))?;
    }

    // Siswa yang ditautkan harus berada dalam cakupan pembuat akun; kalau
    // tidak, seorang staff bisa membuat akun orang tua untuk anak di sekolah
    // lain dan lewat akun itu membaca datanya.
    for student_id in &body.student_ids {
        let row: Option<(Uuid,)> = sqlx::query_as(
            "SELECT school_id FROM students WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(student_id)
        .fetch_optional(&state.db)
        .await?;
        let (school_id,) = row.ok_or_else(|| {
            ApiError::field("student_ids", &format!("siswa `{student_id}` tidak ditemukan"))
        })?;
        user.resolve_school(Some(school_id))?;
    }

    let identity_type = match body.role.as_str() {
        "siswa" => "nisn",
        _ => "nik",
    };
    let identity = body
        .identity_number
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    // Akun siswa mewarisi sekolah dari data siswanya, bukan dari input —
    // keduanya harus selalu sama.
    let school_id = if body.role == "siswa" {
        let (sid,): (Uuid,) = sqlx::query_as("SELECT school_id FROM students WHERE id = $1")
            .bind(body.student_ids[0])
            .fetch_one(&state.db)
            .await?;
        Some(sid)
    } else if body.role == "orang_tua" {
        None
    } else {
        body.school_id
    };

    let hash = password::hash_password(&body.password)?;
    let mut tx = state.db.begin().await?;

    let (id,): (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO users (
            name, username, email, password, school_id, employee_no, position,
            phone, telegram_chat_id, identity_number, identity_type, student_id,
            is_active, must_change_password, email_verified_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,TRUE,TRUE,NOW())
        RETURNING id
        "#,
    )
    .bind(body.name.trim())
    .bind(body.username.trim().to_lowercase())
    .bind(body.email.trim().to_lowercase())
    .bind(&hash)
    .bind(school_id)
    .bind(body.employee_no.as_deref())
    .bind(body.position.as_deref())
    .bind(body.phone.as_deref())
    .bind(body.telegram_chat_id.as_deref())
    .bind(identity)
    .bind(identity.map(|_| identity_type))
    .bind((body.role == "siswa").then(|| body.student_ids[0]))
    .fetch_one(&mut *tx)
    .await?;

    // Akun orang tua ditautkan lewat student_guardians. Baris wali yang sudah
    // ada (dibuat saat input siswa) dipakai ulang bila hubungannya cocok,
    // agar kontak notifikasi tidak terduplikasi.
    if body.role == "orang_tua" {
        let relation = normalize_relation(body.guardian_relation.as_deref())?;

        for student_id in &body.student_ids {
            link_guardian(
                &mut tx,
                id,
                *student_id,
                relation,
                body.name.trim(),
                body.phone.as_deref(),
            )
            .await?;
        }
    }

    assign_role(&mut tx, id, &body.role).await?;

    // Cakupan multi-sekolah hanya bermakna untuk peran provinsi/pengawas dan
    // hanya Superadmin yang boleh menetapkannya.
    if user.is_superadmin() {
        for school_id in &body.extra_school_ids {
            sqlx::query(
                "INSERT INTO user_school_scopes (user_id, school_id) VALUES ($1,$2) ON CONFLICT DO NOTHING",
            )
            .bind(id)
            .bind(school_id)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;

    let created = fetch(&state, id).await?;
    AuditEntry::by_user(&user, "user.create")
        .entity("user", id)
        .after(&created)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        created,
        format!(
            "Akun {} dibuat. Pengguna wajib mengganti kata sandi saat login pertama.",
            body.username.trim()
        ),
    ))
}

/// Perbarui pengguna (termasuk reset kata sandi).
#[utoipa::path(
    patch, path = "/v1/users/{id}", tag = "Pengguna",
    params(("id" = Uuid, Path, description = "ID pengguna")),
    request_body = UpdateUserRequest,
    security(("bearer" = [])),
    responses((status = 200, description = "Pengguna diperbarui", body = UserListItem))
)]
pub async fn update(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ValidJson(body): ValidJson<UpdateUserRequest>,
) -> ApiResult<ApiResponse<UserListItem>> {
    user.require("update_user")?;
    let before = fetch(&state, id).await?;
    ensure_can_manage(&user, before.school_id)?;

    if let Some(role) = &body.role {
        if !ASSIGNABLE_ROLES.contains(&role.as_str()) {
            return Err(ApiError::field("role", "peran tidak dikenal"));
        }
        if PROVINCE_ROLES.contains(&role.as_str()) && !user.is_superadmin() {
            return Err(ApiError::Forbidden(
                "Hanya Superadmin yang dapat mengubah peran menjadi tingkat provinsi".into(),
            ));
        }
        // Peran tingkat sekolah tanpa sekolah = akun yang tidak bisa apa-apa.
        let target_school = body.school_id.or(before.school_id);
        if SCHOOL_BOUND_ROLES.contains(&role.as_str()) && target_school.is_none() {
            return Err(ApiError::field(
                "school_id",
                "peran ini wajib ditautkan ke satu sekolah",
            ));
        }
    }

    // Superadmin terakhir tidak boleh menonaktifkan dirinya sendiri —
    // sistem akan terkunci tanpa jalan masuk.
    if body.is_active == Some(false) && before.roles.iter().any(|r| r == ROLE_SUPERADMIN) {
        let (remaining,): (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)::bigint FROM users u
            JOIN model_has_roles mhr
                 ON mhr.model_id = u.id AND mhr.model_type = 'App\Models\User'
            JOIN roles r ON r.id = mhr.role_id
            WHERE r.name = 'superadmin' AND u.is_active AND u.deleted_at IS NULL
              AND u.id <> $1
            "#,
        )
        .bind(id)
        .fetch_one(&state.db)
        .await?;
        if remaining == 0 {
            return Err(ApiError::Conflict(
                "Tidak dapat menonaktifkan Superadmin terakhir".into(),
            ));
        }
    }

    let new_hash = match &body.new_password {
        Some(p) => Some(password::hash_password(p)?),
        None => None,
    };

    let mut tx = state.db.begin().await?;

    sqlx::query(
        r#"
        UPDATE users SET
            name             = COALESCE($2, name),
            email            = COALESCE($3, email),
            school_id        = COALESCE($4, school_id),
            employee_no      = COALESCE($5, employee_no),
            position         = COALESCE($6, position),
            phone            = COALESCE($7, phone),
            telegram_chat_id = COALESCE($8, telegram_chat_id),
            is_active        = COALESCE($9, is_active),
            password         = COALESCE($10, password),
            must_change_password = CASE WHEN $10::text IS NULL
                                        THEN must_change_password ELSE TRUE END,
            updated_at       = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(body.name.as_deref().map(str::trim))
    .bind(body.email.as_deref().map(|e| e.trim().to_lowercase()))
    .bind(body.school_id)
    .bind(body.employee_no.as_deref())
    .bind(body.position.as_deref())
    .bind(body.phone.as_deref())
    .bind(body.telegram_chat_id.as_deref())
    .bind(body.is_active)
    .bind(new_hash.as_deref())
    .execute(&mut *tx)
    .await?;

    if let Some(role) = &body.role {
        sqlx::query(
            r#"DELETE FROM model_has_roles WHERE model_id = $1 AND model_type = 'App\Models\User'"#,
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
        assign_role(&mut tx, id, role).await?;
    }

    if let (true, Some(schools)) = (user.is_superadmin(), &body.extra_school_ids) {
        sqlx::query("DELETE FROM user_school_scopes WHERE user_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        for school_id in schools {
            sqlx::query(
                "INSERT INTO user_school_scopes (user_id, school_id) VALUES ($1,$2) ON CONFLICT DO NOTHING",
            )
            .bind(id)
            .bind(school_id)
            .execute(&mut *tx)
            .await?;
        }
    }

    // Perubahan peran/kata sandi harus memutus sesi lama, karena izin ikut
    // tersimpan di dalam access token.
    if body.role.is_some() || new_hash.is_some() || body.is_active == Some(false) {
        sqlx::query(
            "UPDATE refresh_tokens SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    let after = fetch(&state, id).await?;
    AuditEntry::by_user(&user, "user.update")
        .entity("user", id)
        .before(&before)
        .after(&after)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(after, "Data pengguna diperbarui"))
}

/// Hapus pengguna (soft delete).
#[utoipa::path(
    delete, path = "/v1/users/{id}", tag = "Pengguna",
    params(("id" = Uuid, Path, description = "ID pengguna")),
    security(("bearer" = [])),
    responses((status = 200, description = "Pengguna dihapus"))
)]
pub async fn remove(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require("delete_user")?;
    if id == user.id {
        return Err(ApiError::Conflict(
            "Anda tidak dapat menghapus akun Anda sendiri".into(),
        ));
    }
    let before = fetch(&state, id).await?;
    ensure_can_manage(&user, before.school_id)?;

    let mut tx = state.db.begin().await?;
    sqlx::query(
        "UPDATE users SET deleted_at = NOW(), is_active = FALSE, updated_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE refresh_tokens SET revoked_at = NOW() WHERE user_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    // Kelas yang ia ampu kehilangan wali kelas, bukan ikut terhapus.
    sqlx::query("UPDATE classrooms SET homeroom_teacher_id = NULL WHERE homeroom_teacher_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    AuditEntry::by_user(&user, "user.delete")
        .entity("user", id)
        .before(&before)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        serde_json::json!({ "deleted": true }),
        "Pengguna dihapus",
    ))
}

/// Blokir pengguna (kompatibel dengan cybercog/laravel-ban di dashboard).
#[utoipa::path(
    post, path = "/v1/users/{id}/ban", tag = "Pengguna",
    params(("id" = Uuid, Path, description = "ID pengguna")),
    security(("bearer" = [])),
    responses((status = 200, description = "Pengguna diblokir"))
)]
pub async fn ban(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require("ban_user")?;
    if id == user.id {
        return Err(ApiError::Conflict(
            "Anda tidak dapat memblokir akun Anda sendiri".into(),
        ));
    }
    let before = fetch(&state, id).await?;
    ensure_can_manage(&user, before.school_id)?;

    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE users SET banned_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"
        INSERT INTO bans (bannable_type, bannable_id, created_by_type, created_by_id,
                          comment, created_at, updated_at)
        VALUES ('App\Models\User', $1, 'App\Models\User', $2, $3, NOW(), NOW())
        "#,
    )
    .bind(id)
    .bind(user.id)
    .bind("Diblokir melalui API")
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE refresh_tokens SET revoked_at = NOW() WHERE user_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    AuditEntry::by_user(&user, "user.ban")
        .entity("user", id)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        serde_json::json!({ "banned": true }),
        "Pengguna diblokir dan seluruh sesinya diakhiri",
    ))
}

/// Cabut blokir pengguna.
#[utoipa::path(
    post, path = "/v1/users/{id}/unban", tag = "Pengguna",
    params(("id" = Uuid, Path, description = "ID pengguna")),
    security(("bearer" = [])),
    responses((status = 200, description = "Blokir dicabut"))
)]
pub async fn unban(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require("ban_user")?;
    let before = fetch(&state, id).await?;
    ensure_can_manage(&user, before.school_id)?;

    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE users SET banned_at = NULL WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"UPDATE bans SET deleted_at = NOW() WHERE bannable_id = $1 AND deleted_at IS NULL"#,
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    AuditEntry::by_user(&user, "user.unban")
        .entity("user", id)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        serde_json::json!({ "banned": false }),
        "Blokir pengguna dicabut",
    ))
}

/// Tautkan akun orang tua ke seorang siswa.
///
/// Dipisahkan dari pembuatan akun karena seorang wali bisa bertambah anaknya
/// kapan saja — adik masuk sekolah tahun berikutnya, atau seorang paman
/// menggantikan orang tua yang meninggal. Tanpa endpoint ini, satu-satunya
/// cara menambah anak adalah menghapus akunnya dan membuat ulang, yang berarti
/// kehilangan riwayat pengaduan dan sesi loginnya.
#[utoipa::path(
    post, path = "/v1/users/{id}/children", tag = "Pengguna",
    params(("id" = Uuid, Path, description = "ID akun orang tua")),
    request_body = LinkChildRequest,
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Anak ditautkan"),
        (status = 403, description = "Siswa berada di luar cakupan Anda"),
        (status = 409, description = "Akun ini bukan akun orang tua")
    )
)]
pub async fn link_child(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ValidJson(body): ValidJson<LinkChildRequest>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require_any(&["manage_app_account", "update_user"])?;

    let target = fetch(&state, id).await?;
    // Akun orang tua bercakupan provinsi (school_id NULL) karena anaknya bisa
    // berbeda sekolah; `ensure_can_manage` akan menolak semua orang selain
    // dinas. Yang menentukan kewenangan di sini adalah SEKOLAH SI ANAK, yang
    // diperiksa di bawah.
    if !target.roles.iter().any(|r| r == "orang_tua") {
        return Err(ApiError::Conflict(
            "Tautan anak hanya berlaku untuk akun dengan peran `orang_tua`".into(),
        ));
    }

    let relation = normalize_relation(body.relation.as_deref())?;
    let student = load_student_for_link(&state, &user, body.student_id).await?;

    let mut tx = state.db.begin().await?;
    link_guardian(
        &mut tx,
        id,
        body.student_id,
        relation,
        &target.name,
        target.phone.as_deref(),
    )
    .await?;
    tx.commit().await?;

    AuditEntry::by_user(&user, "user.link_child")
        .entity("user", id)
        .school(student.school_id)
        .after(&serde_json::json!({
            "student_id": body.student_id,
            "student_name": student.full_name,
            "relation": relation,
        }))
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        serde_json::json!({ "linked": true }),
        format!("{} ditautkan sebagai {relation}", student.full_name),
    ))
}

/// Putuskan tautan akun orang tua dari seorang siswa.
///
/// Baris `student_guardians` sengaja TIDAK dihapus, hanya `user_id`-nya
/// dikosongkan: baris itu juga memuat nomor kontak untuk notifikasi absensi,
/// yang tetap dibutuhkan sekolah meskipun akun aplikasinya dicabut.
#[utoipa::path(
    delete, path = "/v1/users/{id}/children/{student_id}", tag = "Pengguna",
    params(
        ("id" = Uuid, Path, description = "ID akun orang tua"),
        ("student_id" = Uuid, Path, description = "ID siswa")
    ),
    security(("bearer" = [])),
    responses((status = 200, description = "Tautan diputus"))
)]
pub async fn unlink_child(
    State(state): State<AppState>,
    user: AuthUser,
    Path((id, student_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require_any(&["manage_app_account", "update_user"])?;
    let student = load_student_for_link(&state, &user, student_id).await?;

    let affected = sqlx::query(
        "UPDATE student_guardians SET user_id = NULL, updated_at = NOW() \
         WHERE user_id = $1 AND student_id = $2",
    )
    .bind(id)
    .bind(student_id)
    .execute(&state.db)
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(ApiError::NotFound(format!(
            "tautan akun `{id}` ke siswa `{student_id}`"
        )));
    }

    // Token akses menyimpan daftar anak; tanpa pencabutan sesi, orang tua yang
    // sudah dilepas masih bisa membaca data anak itu sampai token kedaluwarsa.
    sqlx::query(
        "UPDATE refresh_tokens SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(id)
    .execute(&state.db)
    .await?;

    AuditEntry::by_user(&user, "user.unlink_child")
        .entity("user", id)
        .school(student.school_id)
        .before(&serde_json::json!({
            "student_id": student_id,
            "student_name": student.full_name,
        }))
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        serde_json::json!({ "linked": false }),
        format!("Tautan ke {} diputus", student.full_name),
    ))
}

// =====================================================================

struct LinkableStudent {
    school_id: Uuid,
    full_name: String,
}

/// Muat siswa dan pastikan pemanggil berwenang atas sekolahnya.
///
/// Tanpa pemeriksaan ini seorang staff TU bisa menautkan akun orang tua ke
/// siswa sekolah lain, dan lewat akun itu membaca absensi anak orang lain.
async fn load_student_for_link(
    state: &AppState,
    actor: &AuthUser,
    student_id: Uuid,
) -> ApiResult<LinkableStudent> {
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT school_id, full_name FROM students WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(student_id)
    .fetch_optional(&state.db)
    .await?;

    let (school_id, full_name) =
        row.ok_or_else(|| ApiError::NotFound(format!("siswa `{student_id}`")))?;
    actor.resolve_school(Some(school_id))?;

    Ok(LinkableStudent {
        school_id,
        full_name,
    })
}

fn normalize_relation(relation: Option<&str>) -> ApiResult<&str> {
    let relation = relation.map(str::trim).unwrap_or("wali");
    let relation = if relation.is_empty() { "wali" } else { relation };

    if !crate::domain::student::GUARDIAN_RELATIONS.contains(&relation) {
        return Err(ApiError::field("relation", "pilih ayah, ibu, atau wali"));
    }

    Ok(relation)
}

/// Tautkan akun ke baris wali siswa.
///
/// Baris wali yang sudah ada (dibuat saat input data siswa) dipakai ulang bila
/// hubungannya cocok, agar nomor kontak notifikasi tidak terduplikasi menjadi
/// dua baris yang sama-sama menerima pesan.
async fn link_guardian(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    student_id: Uuid,
    relation: &str,
    fallback_name: &str,
    fallback_phone: Option<&str>,
) -> ApiResult<()> {
    let linked = sqlx::query(
        r#"
        UPDATE student_guardians
           SET user_id = $1, updated_at = NOW()
         WHERE student_id = $2 AND relation = $3
           AND (user_id IS NULL OR user_id = $1)
        "#,
    )
    .bind(user_id)
    .bind(student_id)
    .bind(relation)
    .execute(&mut **tx)
    .await?
    .rows_affected();

    if linked == 0 {
        sqlx::query(
            r#"
            INSERT INTO student_guardians
                (student_id, school_id, relation, full_name, phone, user_id,
                 preferred_channel, is_primary, notify_enabled)
            SELECT s.id, s.school_id, $3, $4, $5, $1, 'whatsapp',
                   NOT EXISTS (
                       SELECT 1 FROM student_guardians g2
                       WHERE g2.student_id = s.id AND g2.is_primary
                   ),
                   TRUE
            FROM students s WHERE s.id = $2
            "#,
        )
        .bind(user_id)
        .bind(student_id)
        .bind(relation)
        .bind(fallback_name)
        .bind(fallback_phone)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

pub async fn fetch(state: &AppState, id: Uuid) -> ApiResult<UserListItem> {
    let row: Option<UserListItem> = sqlx::query_as(
        r#"
        SELECT u.id, u.name, u.username, u.email, u.phone, u.position,
               u.school_id, s.name AS school_name,
               COALESCE(
                   ARRAY(
                       SELECT r.name FROM roles r
                       JOIN model_has_roles m ON m.role_id = r.id
                       WHERE m.model_id = u.id AND m.model_type = 'App\Models\User'
                       ORDER BY r.name
                   ),
                   ARRAY[]::text[]
               ) AS roles,
               u.is_active, (u.banned_at IS NOT NULL) AS is_banned,
               u.last_login, u.created_at
        FROM users u
        LEFT JOIN schools s ON s.id = u.school_id
        WHERE u.id = $1 AND u.deleted_at IS NULL
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    row.ok_or_else(|| ApiError::NotFound(format!("pengguna `{id}`")))
}

/// Pengguna tingkat sekolah hanya boleh mengelola akun di sekolahnya, dan
/// tidak boleh menyentuh akun bercakupan provinsi (school_id NULL).
fn ensure_can_manage(actor: &AuthUser, target_school: Option<Uuid>) -> ApiResult<()> {
    if actor.is_province_scope() {
        return Ok(());
    }
    match target_school {
        Some(school_id) => actor.resolve_school(Some(school_id)).map(|_| ()),
        None => Err(ApiError::Forbidden(
            "Anda tidak berwenang mengelola akun tingkat provinsi".into(),
        )),
    }
}

pub async fn assign_role(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    role: &str,
) -> ApiResult<()> {
    // Peran dicari lebih dulu agar "peran tidak ada" bisa dibedakan dari
    // "peran sudah terpasang" — keduanya menghasilkan 0 baris pada INSERT
    // ... ON CONFLICT DO NOTHING.
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM roles WHERE name = $1 AND guard_name = 'web'")
            .bind(role)
            .fetch_optional(&mut **tx)
            .await?;

    let (role_id,) = row.ok_or_else(|| {
        ApiError::field("role", &format!("peran `{role}` belum terdaftar di sistem"))
    })?;

    sqlx::query(
        r#"
        INSERT INTO model_has_roles (role_id, model_type, model_id)
        VALUES ($1, 'App\Models\User', $2)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(role_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hubungan_wali_default_ke_wali() {
        // Formulir yang tidak mengisi hubungan tidak boleh menggagalkan
        // penautan — "wali" adalah jawaban yang benar untuk kasus umum
        // (nenek, paman, kakak) dan tidak mengklaim yang tidak diketahui.
        assert_eq!(normalize_relation(None).unwrap(), "wali");
        assert_eq!(normalize_relation(Some("")).unwrap(), "wali");
        assert_eq!(normalize_relation(Some("   ")).unwrap(), "wali");
    }

    #[test]
    fn hubungan_wali_dipangkas_spasinya() {
        assert_eq!(normalize_relation(Some(" ibu ")).unwrap(), "ibu");
        assert_eq!(normalize_relation(Some("ayah")).unwrap(), "ayah");
    }

    #[test]
    fn hubungan_di_luar_daftar_ditolak() {
        // Nilai bebas akan merusak pencocokan baris wali yang sudah ada
        // (`relation = $3`), sehingga tiap penautan membuat baris kontak
        // duplikat yang sama-sama menerima notifikasi.
        let err = normalize_relation(Some("orangtua")).unwrap_err();
        assert!(matches!(err, ApiError::Validation(_)));
    }
}
