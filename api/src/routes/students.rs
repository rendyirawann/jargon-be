//! Siswa, wali murid, dan pendaftaran wajah.

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::Router;
use uuid::Uuid;

use crate::auth::{AuthDevice, AuthUser};
use crate::domain::face::{
    EnrollFaceRequest, EnrollFaceResponse, FaceEnrollmentFilter, FaceEnrollmentItem,
};
use crate::domain::student::{
    normalize_phone, CreateGuardianRequest, CreateStudentRequest, Guardian, Student, StudentFilter,
    StudentListItem, UpdateGuardianRequest, UpdateStudentRequest, GUARDIAN_RELATIONS,
    NOTIFY_CHANNELS, STUDENT_STATUS,
};
use crate::error::{ApiError, ApiResult};
use crate::extract::{ValidJson, ValidQuery};
use crate::services::audit::AuditEntry;
use crate::services::enrollment::{self, EnrollActor};
use crate::state::AppState;
use crate::util::{ApiResponse, PageQuery, Paginated};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/students", get(list).post(create))
        .route("/students/{id}", get(detail).patch(update).delete(remove))
        .route("/students/{id}/guardians", get(list_guardians).post(add_guardian))
        .route(
            "/students/{id}/guardians/{guardian_id}",
            axum::routing::patch(update_guardian).delete(delete_guardian),
        )
        // Pendaftaran wajah oleh operator/guru dari dashboard atau aplikasi.
        .route("/students/{id}/face", post(enroll_face).get(list_face_samples))
        .route("/face-enrollments", get(list_enrollments))
        .route("/face-enrollments/{id}", axum::routing::delete(delete_enrollment))
        // Pendaftaran wajah dari tablet bermode `enroll`.
        .route("/kiosk/students/{id}/face", post(enroll_face_kiosk))
}

const SORTABLE: [&str; 5] = ["full_name", "nis", "nisn", "created_at", "status"];

/// Daftar siswa dengan filter kelas/status/kesiapan wajah.
#[utoipa::path(
    get, path = "/v1/students", tag = "Siswa",
    params(PageQuery, StudentFilter),
    security(("bearer" = [])),
    responses((status = 200, description = "Daftar siswa", body = [StudentListItem]))
)]
pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(page): ValidQuery<PageQuery>,
    ValidQuery(filter): ValidQuery<StudentFilter>,
) -> ApiResult<Paginated<StudentListItem>> {
    user.require("view_student")?;
    let school = user.resolve_school(filter.school_id)?;

    // Default hanya siswa aktif: daftar yang memuat siswa lulus/pindah
    // hampir selalu bukan yang diinginkan pengguna.
    let status = match filter.status.as_deref() {
        None => Some("aktif"),
        Some("all") => None,
        Some(s) if STUDENT_STATUS.contains(&s) => Some(s),
        Some(_) => {
            return Err(ApiError::field(
                "status",
                &format!("status harus salah satu dari: {}, atau `all`", STUDENT_STATUS.join(", ")),
            ))
        }
    };
    let search = page.search_pattern();
    let with_today = filter.with_today.unwrap_or(false);
    let today = crate::util::today_wib();

    let (total,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)::bigint FROM students s
        LEFT JOIN classrooms c ON c.id = s.current_classroom_id
        WHERE s.deleted_at IS NULL
          AND ($1::uuid IS NULL OR s.school_id = $1)
          AND ($2::uuid IS NULL OR s.current_classroom_id = $2)
          AND ($3::smallint IS NULL OR c.grade_level = $3)
          AND ($4::text IS NULL OR s.status = $4)
          AND ($5::boolean IS NULL OR s.face_enrolled = $5)
          AND ($6::text IS NULL OR s.full_name ILIKE $6 OR s.nis ILIKE $6 OR s.nisn ILIKE $6)
        "#,
    )
    .bind(school)
    .bind(filter.classroom_id)
    .bind(filter.grade_level)
    .bind(status)
    .bind(filter.face_enrolled)
    .bind(search.as_deref())
    .fetch_one(&state.db)
    .await?;

    let sql = format!(
        r#"
        SELECT s.id, s.school_id, s.nisn, s.nis, s.full_name, s.gender,
               c.name AS classroom_name, s.status, s.face_enrolled, s.face_sample_count,
               CASE WHEN $7::boolean THEN a.status      ELSE NULL END AS today_status,
               CASE WHEN $7::boolean THEN a.check_in_at ELSE NULL END AS today_check_in
        FROM students s
        LEFT JOIN classrooms c ON c.id = s.current_classroom_id
        LEFT JOIN attendances a
               ON $7::boolean AND a.attendance_date = $8 AND a.student_id = s.id
        WHERE s.deleted_at IS NULL
          AND ($1::uuid IS NULL OR s.school_id = $1)
          AND ($2::uuid IS NULL OR s.current_classroom_id = $2)
          AND ($3::smallint IS NULL OR c.grade_level = $3)
          AND ($4::text IS NULL OR s.status = $4)
          AND ($5::boolean IS NULL OR s.face_enrolled = $5)
          AND ($6::text IS NULL OR s.full_name ILIKE $6 OR s.nis ILIKE $6 OR s.nisn ILIKE $6)
        ORDER BY {}
        LIMIT $9 OFFSET $10
        "#,
        page.order_by(&SORTABLE, "full_name")
    );

    let items: Vec<StudentListItem> = sqlx::query_as(&sql)
        .bind(school)
        .bind(filter.classroom_id)
        .bind(filter.grade_level)
        .bind(status)
        .bind(filter.face_enrolled)
        .bind(search.as_deref())
        .bind(with_today)
        .bind(today)
        .bind(page.per_page())
        .bind(page.offset())
        .fetch_all(&state.db)
        .await?;

    Ok(Paginated::new(items, page.page(), page.per_page(), total))
}

/// Detail siswa.
#[utoipa::path(
    get, path = "/v1/students/{id}", tag = "Siswa",
    params(("id" = Uuid, Path, description = "ID siswa")),
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Detail siswa", body = Student),
        (status = 404, description = "Tidak ditemukan")
    )
)]
pub async fn detail(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<Student>> {
    user.require("view_student")?;
    let s = fetch(&state, id).await?;
    user.resolve_school(Some(s.school_id))?;
    Ok(ApiResponse::new(s))
}

/// Tambah siswa (sekaligus wali murid bila dikirim).
#[utoipa::path(
    post, path = "/v1/students", tag = "Siswa",
    request_body = CreateStudentRequest,
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Siswa ditambahkan", body = Student),
        (status = 409, description = "NISN/NIS sudah terdaftar")
    )
)]
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    ValidJson(body): ValidJson<CreateStudentRequest>,
) -> ApiResult<ApiResponse<Student>> {
    user.require("create_student")?;
    let school_id = user.require_school(body.school_id)?;

    if let Some(g) = &body.gender {
        if g != "L" && g != "P" {
            return Err(ApiError::field("gender", "jenis kelamin harus L atau P"));
        }
    }
    if let Some(classroom_id) = body.current_classroom_id {
        ensure_classroom_in_school(&state, classroom_id, school_id).await?;
    }
    for g in &body.guardians {
        validate_guardian_enums(&g.relation, &g.preferred_channel)?;
    }

    let mut tx = state.db.begin().await?;

    let (id,): (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO students (
            school_id, current_classroom_id, nisn, nis, full_name, gender,
            birth_place, birth_date, religion, address, phone,
            father_name, mother_name, entry_year
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
        RETURNING id
        "#,
    )
    .bind(school_id)
    .bind(body.current_classroom_id)
    .bind(body.nisn.as_deref().map(str::trim))
    .bind(body.nis.as_deref().map(str::trim))
    .bind(body.full_name.trim())
    .bind(body.gender.as_deref())
    .bind(body.birth_place.as_deref())
    .bind(body.birth_date)
    .bind(body.religion.as_deref())
    .bind(body.address.as_deref())
    .bind(body.phone.as_deref())
    .bind(body.father_name.as_deref())
    .bind(body.mother_name.as_deref())
    .bind(body.entry_year)
    .fetch_one(&mut *tx)
    .await?;

    // Wali pertama otomatis jadi kontak utama bila operator tidak menandai
    // salah satu — tanpa kontak utama, notifikasi tidak punya tujuan jelas.
    let mut has_primary = body.guardians.iter().any(|g| g.is_primary);
    for (idx, g) in body.guardians.iter().enumerate() {
        let is_primary = g.is_primary || (!has_primary && idx == 0);
        if is_primary {
            has_primary = true;
        }
        insert_guardian(&mut tx, id, school_id, g, is_primary).await?;
    }

    if let Some(classroom_id) = body.current_classroom_id {
        record_class_enrollment(&mut tx, id, school_id, classroom_id).await?;
    }

    tx.commit().await?;

    let created = fetch(&state, id).await?;
    AuditEntry::by_user(&user, "student.create")
        .school(school_id)
        .entity("student", id)
        .after(&created)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        created,
        format!("Siswa {} berhasil ditambahkan", body.full_name.trim()),
    ))
}

/// Perbarui data siswa.
#[utoipa::path(
    patch, path = "/v1/students/{id}", tag = "Siswa",
    params(("id" = Uuid, Path, description = "ID siswa")),
    request_body = UpdateStudentRequest,
    security(("bearer" = [])),
    responses((status = 200, description = "Siswa diperbarui", body = Student))
)]
pub async fn update(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ValidJson(body): ValidJson<UpdateStudentRequest>,
) -> ApiResult<ApiResponse<Student>> {
    user.require("update_student")?;
    let before = fetch(&state, id).await?;
    user.resolve_school(Some(before.school_id))?;

    if let Some(st) = &body.status {
        if !STUDENT_STATUS.contains(&st.as_str()) {
            return Err(ApiError::field(
                "status",
                &format!("pilih salah satu: {}", STUDENT_STATUS.join(", ")),
            ));
        }
    }
    if let Some(classroom_id) = body.current_classroom_id {
        ensure_classroom_in_school(&state, classroom_id, before.school_id).await?;
    }

    let mut tx = state.db.begin().await?;

    sqlx::query(
        r#"
        UPDATE students SET
            current_classroom_id = COALESCE($2, current_classroom_id),
            nisn        = COALESCE($3, nisn),
            nis         = COALESCE($4, nis),
            full_name   = COALESCE($5, full_name),
            gender      = COALESCE($6, gender),
            birth_place = COALESCE($7, birth_place),
            birth_date  = COALESCE($8, birth_date),
            religion    = COALESCE($9, religion),
            address     = COALESCE($10, address),
            phone       = COALESCE($11, phone),
            father_name = COALESCE($12, father_name),
            mother_name = COALESCE($13, mother_name),
            status      = COALESCE($14, status),
            entry_year  = COALESCE($15, entry_year),
            updated_at  = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(body.current_classroom_id)
    .bind(body.nisn.as_deref().map(str::trim))
    .bind(body.nis.as_deref().map(str::trim))
    .bind(body.full_name.as_deref().map(str::trim))
    .bind(body.gender.as_deref())
    .bind(body.birth_place.as_deref())
    .bind(body.birth_date)
    .bind(body.religion.as_deref())
    .bind(body.address.as_deref())
    .bind(body.phone.as_deref())
    .bind(body.father_name.as_deref())
    .bind(body.mother_name.as_deref())
    .bind(body.status.as_deref())
    .bind(body.entry_year)
    .execute(&mut *tx)
    .await?;

    if let Some(classroom_id) = body.current_classroom_id {
        if before.current_classroom_id != Some(classroom_id) {
            record_class_enrollment(&mut tx, id, before.school_id, classroom_id).await?;
        }
    }

    tx.commit().await?;

    let after = fetch(&state, id).await?;

    // Siswa yang tidak lagi aktif harus berhenti dikenali oleh tablet.
    if before.status == "aktif" && after.status != "aktif" {
        state.broadcast_face_invalidation(after.school_id).await;
    }

    AuditEntry::by_user(&user, "student.update")
        .school(after.school_id)
        .entity("student", id)
        .before(&before)
        .after(&after)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(after, "Data siswa diperbarui"))
}

/// Hapus siswa (soft delete) beserta data biometriknya.
///
/// Data wajah dihapus permanen — ini kewajiban perlindungan data pribadi.
/// Riwayat absensi tetap disimpan karena bersifat dokumen administrasi.
#[utoipa::path(
    delete, path = "/v1/students/{id}", tag = "Siswa",
    params(("id" = Uuid, Path, description = "ID siswa")),
    security(("bearer" = [])),
    responses((status = 200, description = "Siswa dihapus"))
)]
pub async fn remove(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require("delete_student")?;
    let before = fetch(&state, id).await?;
    user.resolve_school(Some(before.school_id))?;

    // Ambil object key gambar sebelum baris dihapus.
    let keys: Vec<(String,)> =
        sqlx::query_as("SELECT image_key FROM face_enrollments WHERE student_id = $1")
            .bind(id)
            .fetch_all(&state.db)
            .await?;

    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM face_enrollments WHERE student_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM face_embeddings WHERE student_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"
        UPDATE students
           SET deleted_at = NOW(), status = 'keluar',
               face_enrolled = FALSE, face_sample_count = 0
         WHERE id = $1
        "#,
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    for (key,) in keys {
        let _ = state.storage.delete(&key).await;
    }
    state.broadcast_face_invalidation(before.school_id).await;

    AuditEntry::by_user(&user, "student.delete")
        .school(before.school_id)
        .entity("student", id)
        .before(&before)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        serde_json::json!({ "deleted": true, "biometric_purged": true }),
        "Siswa dihapus dan seluruh data wajahnya dimusnahkan",
    ))
}

// =====================================================================
// Wali murid
// =====================================================================

/// Daftar wali murid seorang siswa.
#[utoipa::path(
    get, path = "/v1/students/{id}/guardians", tag = "Siswa",
    params(("id" = Uuid, Path, description = "ID siswa")),
    security(("bearer" = [])),
    responses((status = 200, description = "Daftar wali", body = [Guardian]))
)]
pub async fn list_guardians(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<Vec<Guardian>>> {
    user.require("view_student")?;
    let s = fetch(&state, id).await?;
    user.resolve_school(Some(s.school_id))?;

    let rows: Vec<Guardian> = sqlx::query_as(
        r#"
        SELECT id, student_id, relation, full_name, phone, whatsapp, email,
               telegram_chat_id, preferred_channel, is_primary, notify_enabled
        FROM student_guardians
        WHERE student_id = $1
        ORDER BY is_primary DESC, created_at
        "#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    Ok(ApiResponse::new(rows))
}

/// Tambah wali murid.
#[utoipa::path(
    post, path = "/v1/students/{id}/guardians", tag = "Siswa",
    params(("id" = Uuid, Path, description = "ID siswa")),
    request_body = CreateGuardianRequest,
    security(("bearer" = [])),
    responses((status = 200, description = "Wali ditambahkan", body = Guardian))
)]
pub async fn add_guardian(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ValidJson(body): ValidJson<CreateGuardianRequest>,
) -> ApiResult<ApiResponse<Guardian>> {
    user.require("manage_guardian")?;
    let s = fetch(&state, id).await?;
    user.resolve_school(Some(s.school_id))?;
    validate_guardian_enums(&body.relation, &body.preferred_channel)?;

    let mut tx = state.db.begin().await?;
    if body.is_primary {
        clear_primary(&mut tx, id).await?;
    }
    let guardian_id = insert_guardian(&mut tx, id, s.school_id, &body, body.is_primary).await?;
    tx.commit().await?;

    let row: Guardian = sqlx::query_as(
        r#"
        SELECT id, student_id, relation, full_name, phone, whatsapp, email,
               telegram_chat_id, preferred_channel, is_primary, notify_enabled
        FROM student_guardians WHERE id = $1
        "#,
    )
    .bind(guardian_id)
    .fetch_one(&state.db)
    .await?;

    AuditEntry::by_user(&user, "guardian.create")
        .school(s.school_id)
        .entity("student_guardian", guardian_id)
        .after(&row)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(row, "Wali murid ditambahkan"))
}

/// Perbarui data wali murid.
#[utoipa::path(
    patch, path = "/v1/students/{id}/guardians/{guardian_id}", tag = "Siswa",
    params(
        ("id" = Uuid, Path, description = "ID siswa"),
        ("guardian_id" = Uuid, Path, description = "ID wali")
    ),
    request_body = UpdateGuardianRequest,
    security(("bearer" = [])),
    responses((status = 200, description = "Wali diperbarui", body = Guardian))
)]
pub async fn update_guardian(
    State(state): State<AppState>,
    user: AuthUser,
    Path((id, guardian_id)): Path<(Uuid, Uuid)>,
    ValidJson(body): ValidJson<UpdateGuardianRequest>,
) -> ApiResult<ApiResponse<Guardian>> {
    user.require("manage_guardian")?;
    let s = fetch(&state, id).await?;
    user.resolve_school(Some(s.school_id))?;

    if let Some(r) = &body.relation {
        if !GUARDIAN_RELATIONS.contains(&r.as_str()) {
            return Err(ApiError::field("relation", "hubungan tidak dikenal"));
        }
    }
    if let Some(c) = &body.preferred_channel {
        if !NOTIFY_CHANNELS.contains(&c.as_str()) {
            return Err(ApiError::field("preferred_channel", "kanal tidak dikenal"));
        }
    }

    let mut tx = state.db.begin().await?;
    if body.is_primary == Some(true) {
        clear_primary(&mut tx, id).await?;
    }

    let affected = sqlx::query(
        r#"
        UPDATE student_guardians SET
            relation          = COALESCE($3, relation),
            full_name         = COALESCE($4, full_name),
            phone             = COALESCE($5, phone),
            whatsapp          = COALESCE($6, whatsapp),
            email             = COALESCE($7, email),
            telegram_chat_id  = COALESCE($8, telegram_chat_id),
            preferred_channel = COALESCE($9, preferred_channel),
            is_primary        = COALESCE($10, is_primary),
            notify_enabled    = COALESCE($11, notify_enabled),
            updated_at        = NOW()
        WHERE id = $1 AND student_id = $2
        "#,
    )
    .bind(guardian_id)
    .bind(id)
    .bind(body.relation.as_deref())
    .bind(body.full_name.as_deref().map(str::trim))
    .bind(body.phone.as_deref().and_then(|p| normalize_phone(p)))
    .bind(body.whatsapp.as_deref().and_then(|p| normalize_phone(p)))
    .bind(body.email.as_deref())
    .bind(body.telegram_chat_id.as_deref())
    .bind(body.preferred_channel.as_deref())
    .bind(body.is_primary)
    .bind(body.notify_enabled)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    if affected == 0 {
        tx.rollback().await?;
        return Err(ApiError::NotFound(format!("wali `{guardian_id}`")));
    }
    tx.commit().await?;

    let row: Guardian = sqlx::query_as(
        r#"
        SELECT id, student_id, relation, full_name, phone, whatsapp, email,
               telegram_chat_id, preferred_channel, is_primary, notify_enabled
        FROM student_guardians WHERE id = $1
        "#,
    )
    .bind(guardian_id)
    .fetch_one(&state.db)
    .await?;

    Ok(ApiResponse::with_message(row, "Data wali diperbarui"))
}

/// Hapus wali murid.
#[utoipa::path(
    delete, path = "/v1/students/{id}/guardians/{guardian_id}", tag = "Siswa",
    params(
        ("id" = Uuid, Path, description = "ID siswa"),
        ("guardian_id" = Uuid, Path, description = "ID wali")
    ),
    security(("bearer" = [])),
    responses((status = 200, description = "Wali dihapus"))
)]
pub async fn delete_guardian(
    State(state): State<AppState>,
    user: AuthUser,
    Path((id, guardian_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require("manage_guardian")?;
    let s = fetch(&state, id).await?;
    user.resolve_school(Some(s.school_id))?;

    let affected =
        sqlx::query("DELETE FROM student_guardians WHERE id = $1 AND student_id = $2")
            .bind(guardian_id)
            .bind(id)
            .execute(&state.db)
            .await?
            .rows_affected();

    if affected == 0 {
        return Err(ApiError::NotFound(format!("wali `{guardian_id}`")));
    }

    Ok(ApiResponse::with_message(
        serde_json::json!({ "deleted": true }),
        "Wali murid dihapus",
    ))
}

// =====================================================================
// Pendaftaran wajah
// =====================================================================

/// Daftarkan satu sampel wajah siswa (dipanggil dashboard / aplikasi guru).
#[utoipa::path(
    post, path = "/v1/students/{id}/face", tag = "Biometrik",
    params(("id" = Uuid, Path, description = "ID siswa")),
    request_body = EnrollFaceRequest,
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Sampel wajah tersimpan", body = EnrollFaceResponse),
        (status = 409, description = "Wajah bertabrakan dengan siswa lain"),
        (status = 422, description = "Kualitas foto tidak memadai")
    )
)]
pub async fn enroll_face(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ValidJson(body): ValidJson<EnrollFaceRequest>,
) -> ApiResult<ApiResponse<EnrollFaceResponse>> {
    user.require("create_face_enrollment")?;
    let s = fetch(&state, id).await?;
    let school_id = user.resolve_school(Some(s.school_id))?.unwrap_or(s.school_id);

    let result = enrollment::enroll(
        &state,
        id,
        school_id,
        EnrollActor { user_id: Some(user.id), device_id: None },
        body,
    )
    .await?;

    AuditEntry::by_user(&user, "face.enroll")
        .school(school_id)
        .entity("student", id)
        .after(&serde_json::json!({
            "enrollment_id": result.enrollment_id,
            "sample_count": result.sample_count,
            "quality_score": result.quality.score,
        }))
        .write(&state.db)
        .await;

    let message = result.message.clone();
    Ok(ApiResponse::with_message(result, message))
}

/// Pendaftaran wajah dari tablet bermode `enroll`.
#[utoipa::path(
    post, path = "/v1/kiosk/students/{id}/face", tag = "Kios",
    params(("id" = Uuid, Path, description = "ID siswa")),
    request_body = EnrollFaceRequest,
    security(("device" = [])),
    responses(
        (status = 200, description = "Sampel wajah tersimpan", body = EnrollFaceResponse),
        (status = 403, description = "Perangkat tidak dalam mode enroll")
    )
)]
pub async fn enroll_face_kiosk(
    State(state): State<AppState>,
    device: AuthDevice,
    Path(id): Path<Uuid>,
    ValidJson(body): ValidJson<EnrollFaceRequest>,
) -> ApiResult<ApiResponse<EnrollFaceResponse>> {
    if !device.can_enroll() {
        return Err(ApiError::Forbidden(
            "Perangkat ini tidak diizinkan mendaftarkan wajah. Ubah mode perangkat menjadi `enroll` di dashboard.".into(),
        ));
    }

    let result = enrollment::enroll(
        &state,
        id,
        device.school_id,
        EnrollActor { user_id: None, device_id: Some(device.id) },
        body,
    )
    .await?;

    let message = result.message.clone();
    Ok(ApiResponse::with_message(result, message))
}

/// Sampel wajah milik satu siswa.
#[utoipa::path(
    get, path = "/v1/students/{id}/face", tag = "Biometrik",
    params(("id" = Uuid, Path, description = "ID siswa")),
    security(("bearer" = [])),
    responses((status = 200, description = "Daftar sampel", body = [FaceEnrollmentItem]))
)]
pub async fn list_face_samples(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<Vec<FaceEnrollmentItem>>> {
    user.require("view_face_enrollment")?;
    let s = fetch(&state, id).await?;
    user.resolve_school(Some(s.school_id))?;

    let rows = query_enrollments(&state, Some(s.school_id), None, Some(id), None, 100, 0).await?;
    Ok(ApiResponse::new(rows))
}

/// Daftar seluruh pendaftaran wajah (mis. untuk verifikasi kepala sekolah).
#[utoipa::path(
    get, path = "/v1/face-enrollments", tag = "Biometrik",
    params(PageQuery, FaceEnrollmentFilter),
    security(("bearer" = [])),
    responses((status = 200, description = "Daftar pendaftaran wajah", body = [FaceEnrollmentItem]))
)]
pub async fn list_enrollments(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(page): ValidQuery<PageQuery>,
    ValidQuery(filter): ValidQuery<FaceEnrollmentFilter>,
) -> ApiResult<Paginated<FaceEnrollmentItem>> {
    user.require("view_face_enrollment")?;
    let school = user.resolve_school(filter.school_id)?;

    let (total,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)::bigint FROM face_enrollments fe
        JOIN students s ON s.id = fe.student_id
        WHERE ($1::uuid IS NULL OR fe.school_id = $1)
          AND ($2::uuid IS NULL OR s.current_classroom_id = $2)
          AND ($3::uuid IS NULL OR fe.student_id = $3)
          AND ($4::text IS NULL OR fe.status = $4)
        "#,
    )
    .bind(school)
    .bind(filter.classroom_id)
    .bind(filter.student_id)
    .bind(filter.status.as_deref())
    .fetch_one(&state.db)
    .await?;

    let items = query_enrollments(
        &state,
        school,
        filter.classroom_id,
        filter.student_id,
        filter.status.as_deref(),
        page.per_page(),
        page.offset(),
    )
    .await?;

    Ok(Paginated::new(items, page.page(), page.per_page(), total))
}

/// Hapus satu sampel wajah.
#[utoipa::path(
    delete, path = "/v1/face-enrollments/{id}", tag = "Biometrik",
    params(("id" = Uuid, Path, description = "ID pendaftaran wajah")),
    security(("bearer" = [])),
    responses((status = 200, description = "Sampel dihapus"))
)]
pub async fn delete_enrollment(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require("delete_face_enrollment")?;

    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT school_id FROM face_enrollments WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
    let (school_id,) = row.ok_or_else(|| ApiError::NotFound(format!("data wajah `{id}`")))?;
    user.resolve_school(Some(school_id))?;

    enrollment::delete_sample(&state, id, school_id).await?;

    AuditEntry::by_user(&user, "face.delete")
        .school(school_id)
        .entity("face_enrollment", id)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        serde_json::json!({ "deleted": true }),
        "Sampel wajah dihapus",
    ))
}

// =====================================================================
// Helper
// =====================================================================

pub async fn fetch(state: &AppState, id: Uuid) -> ApiResult<Student> {
    let row: Option<Student> = sqlx::query_as(
        r#"
        SELECT s.id, s.school_id, sc.name AS school_name, s.current_classroom_id,
               c.name AS classroom_name, c.grade_level, s.nisn, s.nis, s.full_name,
               s.gender, s.birth_place, s.birth_date, s.religion, s.address, s.phone,
               s.photo_path, s.father_name, s.mother_name, s.status, s.entry_year,
               s.face_enrolled, s.face_enrolled_at, s.face_sample_count,
               s.created_at, s.updated_at
        FROM students s
        JOIN schools sc ON sc.id = s.school_id
        LEFT JOIN classrooms c ON c.id = s.current_classroom_id
        WHERE s.id = $1 AND s.deleted_at IS NULL
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    row.ok_or_else(|| ApiError::NotFound(format!("siswa `{id}`")))
}

pub async fn query_enrollments(
    state: &AppState,
    school_id: Option<Uuid>,
    classroom_id: Option<Uuid>,
    student_id: Option<Uuid>,
    status: Option<&str>,
    limit: i64,
    offset: i64,
) -> ApiResult<Vec<FaceEnrollmentItem>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        student_id: Uuid,
        student_name: String,
        classroom_name: Option<String>,
        pose: String,
        quality_score: Option<f32>,
        status: String,
        reject_reason: Option<String>,
        image_key: String,
        created_at: chrono::DateTime<chrono::Utc>,
        reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    let rows: Vec<Row> = sqlx::query_as(
        r#"
        SELECT fe.id, fe.student_id, s.full_name AS student_name, c.name AS classroom_name,
               fe.pose, fe.quality_score, fe.status, fe.reject_reason, fe.image_key,
               fe.created_at, fe.reviewed_at
        FROM face_enrollments fe
        JOIN students s ON s.id = fe.student_id
        LEFT JOIN classrooms c ON c.id = s.current_classroom_id
        WHERE ($1::uuid IS NULL OR fe.school_id = $1)
          AND ($2::uuid IS NULL OR s.current_classroom_id = $2)
          AND ($3::uuid IS NULL OR fe.student_id = $3)
          AND ($4::text IS NULL OR fe.status = $4)
        ORDER BY fe.created_at DESC
        LIMIT $5 OFFSET $6
        "#,
    )
    .bind(school_id)
    .bind(classroom_id)
    .bind(student_id)
    .bind(status)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| FaceEnrollmentItem {
            id: r.id,
            student_id: r.student_id,
            student_name: r.student_name,
            classroom_name: r.classroom_name,
            pose: r.pose,
            quality_score: r.quality_score,
            status: r.status,
            reject_reason: r.reject_reason,
            image_url: state.storage.public_url(&r.image_key),
            created_at: r.created_at,
            reviewed_at: r.reviewed_at,
        })
        .collect())
}

pub async fn insert_guardian(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    student_id: Uuid,
    school_id: Uuid,
    g: &CreateGuardianRequest,
    is_primary: bool,
) -> ApiResult<Uuid> {
    let (id,): (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO student_guardians (
            student_id, school_id, relation, full_name, phone, whatsapp, email,
            telegram_chat_id, preferred_channel, is_primary, notify_enabled
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
        RETURNING id
        "#,
    )
    .bind(student_id)
    .bind(school_id)
    .bind(&g.relation)
    .bind(g.full_name.trim())
    .bind(g.phone.as_deref().and_then(|p| normalize_phone(p)))
    .bind(g.whatsapp.as_deref().and_then(|p| normalize_phone(p)))
    .bind(g.email.as_deref())
    .bind(g.telegram_chat_id.as_deref())
    .bind(&g.preferred_channel)
    .bind(is_primary)
    .bind(g.notify_enabled)
    .fetch_one(&mut **tx)
    .await?;
    Ok(id)
}

pub async fn clear_primary(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    student_id: Uuid,
) -> ApiResult<()> {
    sqlx::query("UPDATE student_guardians SET is_primary = FALSE WHERE student_id = $1")
        .bind(student_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub async fn record_class_enrollment(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    student_id: Uuid,
    school_id: Uuid,
    classroom_id: Uuid,
) -> ApiResult<()> {
    let year: Option<(Uuid,)> =
        sqlx::query_as("SELECT academic_year_id FROM classrooms WHERE id = $1")
            .bind(classroom_id)
            .fetch_optional(&mut **tx)
            .await?;
    let Some((academic_year_id,)) = year else {
        return Ok(());
    };

    // Tutup penempatan sebelumnya pada tahun ajaran yang sama sebelum
    // membuka yang baru, agar unique index `is_current` tidak dilanggar.
    sqlx::query(
        r#"
        UPDATE student_class_enrollments
           SET is_current = FALSE, ended_at = CURRENT_DATE
         WHERE student_id = $1 AND academic_year_id = $2 AND is_current
        "#,
    )
    .bind(student_id)
    .bind(academic_year_id)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO student_class_enrollments
            (student_id, classroom_id, school_id, academic_year_id, is_current)
        VALUES ($1,$2,$3,$4,TRUE)
        "#,
    )
    .bind(student_id)
    .bind(classroom_id)
    .bind(school_id)
    .bind(academic_year_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn ensure_classroom_in_school(
    state: &AppState,
    classroom_id: Uuid,
    school_id: Uuid,
) -> ApiResult<()> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM classrooms WHERE id = $1 AND school_id = $2 AND deleted_at IS NULL",
    )
    .bind(classroom_id)
    .bind(school_id)
    .fetch_optional(&state.db)
    .await?;
    row.map(|_| ()).ok_or_else(|| {
        ApiError::field(
            "current_classroom_id",
            "kelas tidak ditemukan pada sekolah ini",
        )
    })
}

fn validate_guardian_enums(relation: &str, channel: &str) -> ApiResult<()> {
    if !GUARDIAN_RELATIONS.contains(&relation) {
        return Err(ApiError::field(
            "relation",
            &format!("pilih salah satu: {}", GUARDIAN_RELATIONS.join(", ")),
        ));
    }
    if !NOTIFY_CHANNELS.contains(&channel) {
        return Err(ApiError::field(
            "preferred_channel",
            &format!("pilih salah satu: {}", NOTIFY_CHANNELS.join(", ")),
        ));
    }
    Ok(())
}
