//! Pemberkasan — unggah dan verifikasi berkas kepegawaian.
//!
//! SIAPA BOLEH APA
//!   * Guru/staff : membuat pengajuan, mengunggah berkas miliknya sendiri.
//!   * Kepala sekolah : memverifikasi pengajuan pegawai di sekolahnya.
//!   * Admin dinas : memverifikasi seluruh pengajuan.
//!
//! Setelah pengajuan berstatus `diajukan`, guru tidak bisa lagi mengganti
//! berkas. Tanpa penguncian itu, verifikator bisa menyetujui berkas yang
//! sudah ditukar setelah sebagian diperiksa.

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::Router;
use base64::Engine as _;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::domain::document::{
    ChecklistItem, CreateSubmissionRequest, DocumentType, ReviewFileRequest,
    ReviewSubmissionRequest, SubmissionDetail, SubmissionEvent, SubmissionFile, SubmissionFilter,
    SubmissionListItem, UploadFileRequest, UploadFileResponse, EDITABLE_STATUSES, PURPOSES,
};
use crate::error::{ApiError, ApiResult};
use crate::extract::{ValidJson, ValidQuery};
use crate::face::vector;
use crate::services::audit::AuditEntry;
use crate::state::AppState;
use crate::util::{ApiResponse, PageQuery, Paginated};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/documents/types", get(list_types))
        .route("/documents/submissions", get(list_submissions).post(create_submission))
        .route("/documents/submissions/{id}", get(submission_detail))
        .route("/documents/submissions/{id}/files", post(upload_file))
        .route("/documents/submissions/{id}/submit", post(submit))
        .route("/documents/submissions/{id}/review", post(review_submission))
        .route("/documents/files/{file_id}", axum::routing::delete(delete_file))
        .route("/documents/files/{file_id}/review", post(review_file))
}

/// Daftar jenis dokumen yang diminta untuk sebuah keperluan.
#[utoipa::path(
    get, path = "/v1/documents/types", tag = "Pemberkasan",
    params(("purpose" = Option<String>, Query, description = "Filter keperluan")),
    security(("bearer" = [])),
    responses((status = 200, description = "Jenis dokumen", body = [DocumentType]))
)]
pub async fn list_types(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(q): ValidQuery<PurposeQuery>,
) -> ApiResult<ApiResponse<Vec<DocumentType>>> {
    user.require_any(&["view_document_submission", "create_document_submission"])?;

    let rows: Vec<DocumentType> = sqlx::query_as(
        r#"
        SELECT id, code, name, description, purpose, is_required,
               max_bytes, allowed_mime, sort_order
        FROM document_types
        WHERE is_active AND ($1::text IS NULL OR purpose = $1)
        ORDER BY purpose, sort_order, name
        "#,
    )
    .bind(q.purpose.as_deref())
    .fetch_all(&state.db)
    .await?;

    Ok(ApiResponse::new(rows))
}

#[derive(Debug, serde::Deserialize)]
pub struct PurposeQuery {
    pub purpose: Option<String>,
}

/// Daftar pengajuan.
#[utoipa::path(
    get, path = "/v1/documents/submissions", tag = "Pemberkasan",
    params(PageQuery, SubmissionFilter),
    security(("bearer" = [])),
    responses((status = 200, description = "Daftar pengajuan", body = [SubmissionListItem]))
)]
pub async fn list_submissions(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(page): ValidQuery<PageQuery>,
    ValidQuery(filter): ValidQuery<SubmissionFilter>,
) -> ApiResult<Paginated<SubmissionListItem>> {
    user.require("view_document_submission")?;

    // Tanpa izin verifikasi, pengguna hanya melihat pengajuannya sendiri —
    // berkas kepegawaian memuat data pribadi (NIK, rekening, ijazah).
    let can_verify = user.has_permission("verify_document_submission");
    let mine_only = filter.mine.unwrap_or(false) || !can_verify;
    let school_scope = if can_verify {
        user.resolve_school(filter.school_id)?
    } else {
        None
    };

    let (total,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)::bigint FROM document_submissions s
        WHERE (NOT $2 OR s.user_id = $1)
          AND ($3::uuid IS NULL OR s.school_id = $3)
          AND ($4::text IS NULL OR s.purpose = $4)
          AND ($5::text IS NULL OR s.status = $5)
          -- Draft milik orang lain tidak pernah terlihat verifikator.
          AND (s.user_id = $1 OR s.status <> 'draft')
        "#,
    )
    .bind(user.id)
    .bind(mine_only)
    .bind(school_scope)
    .bind(filter.purpose.as_deref())
    .bind(filter.status.as_deref())
    .fetch_one(&state.db)
    .await?;

    let items: Vec<SubmissionListItem> = sqlx::query_as(
        r#"
        SELECT s.id, s.user_id, u.name AS owner_name, s.school_id,
               sc.name AS school_name, s.purpose, s.period, s.title, s.status,
               s.file_count, s.approved_file_count, s.rejected_file_count,
               s.submitted_at, s.reviewed_at, s.created_at
        FROM document_submissions s
        JOIN users u ON u.id = s.user_id
        LEFT JOIN schools sc ON sc.id = s.school_id
        WHERE (NOT $2 OR s.user_id = $1)
          AND ($3::uuid IS NULL OR s.school_id = $3)
          AND ($4::text IS NULL OR s.purpose = $4)
          AND ($5::text IS NULL OR s.status = $5)
          AND (s.user_id = $1 OR s.status <> 'draft')
        ORDER BY
            -- Yang menunggu diperiksa naik ke atas bagi verifikator.
            (s.status = 'diajukan') DESC,
            COALESCE(s.submitted_at, s.created_at) DESC
        LIMIT $6 OFFSET $7
        "#,
    )
    .bind(user.id)
    .bind(mine_only)
    .bind(school_scope)
    .bind(filter.purpose.as_deref())
    .bind(filter.status.as_deref())
    .bind(page.per_page())
    .bind(page.offset())
    .fetch_all(&state.db)
    .await?;

    Ok(Paginated::new(items, page.page(), page.per_page(), total))
}

/// Detail pengajuan beserta daftar periksa dan lini masa.
#[utoipa::path(
    get, path = "/v1/documents/submissions/{id}", tag = "Pemberkasan",
    params(("id" = Uuid, Path, description = "ID pengajuan")),
    security(("bearer" = [])),
    responses((status = 200, description = "Detail pengajuan", body = SubmissionDetail))
)]
pub async fn submission_detail(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<SubmissionDetail>> {
    user.require("view_document_submission")?;
    let submission = fetch_submission(&state, &user, id).await?;

    let files: Vec<SubmissionFile> = sqlx::query_as(
        r#"
        SELECT f.id, f.document_type_id, dt.name AS document_type_name,
               f.original_name, f.mime_type, f.bytes, f.status, f.reject_reason,
               f.file_key AS file_url, f.uploaded_at
        FROM document_files f
        LEFT JOIN document_types dt ON dt.id = f.document_type_id
        WHERE f.submission_id = $1
        ORDER BY dt.sort_order NULLS LAST, f.uploaded_at
        "#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    // Object key diubah menjadi URL berotorisasi; berkas kepegawaian tidak
    // pernah dilayani langsung oleh web server.
    let files: Vec<SubmissionFile> = files
        .into_iter()
        .map(|mut f| {
            f.file_url = state.storage.public_url(&f.file_url);
            f
        })
        .collect();

    let checklist = build_checklist(&state, &submission.purpose, id).await?;

    let timeline: Vec<SubmissionEvent> = sqlx::query_as(
        r#"
        SELECT status, note, actor_label, created_at
        FROM document_submission_events
        WHERE submission_id = $1 ORDER BY created_at
        "#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let extra: (Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        r#"
        SELECT s.note, s.review_note, r.name AS reviewer_name
        FROM document_submissions s
        LEFT JOIN users r ON r.id = s.reviewed_by
        WHERE s.id = $1
        "#,
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    let is_editable =
        submission.user_id == user.id && EDITABLE_STATUSES.contains(&submission.status.as_str());

    Ok(ApiResponse::new(SubmissionDetail {
        submission,
        note: extra.0,
        review_note: extra.1,
        reviewer_name: extra.2,
        files,
        checklist,
        timeline,
        is_editable,
    }))
}

/// Buat pengajuan baru.
#[utoipa::path(
    post, path = "/v1/documents/submissions", tag = "Pemberkasan",
    request_body = CreateSubmissionRequest,
    security(("bearer" = [])),
    responses((status = 200, description = "Pengajuan dibuat", body = SubmissionListItem))
)]
pub async fn create_submission(
    State(state): State<AppState>,
    user: AuthUser,
    ValidJson(body): ValidJson<CreateSubmissionRequest>,
) -> ApiResult<ApiResponse<SubmissionListItem>> {
    user.require("create_document_submission")?;

    if !PURPOSES.contains(&body.purpose.as_str()) {
        return Err(ApiError::field(
            "purpose",
            &format!("pilih salah satu: {}", PURPOSES.join(", ")),
        ));
    }

    // Satu pengajuan aktif per keperluan: dua berkas kenaikan pangkat yang
    // berjalan bersamaan hanya akan membingungkan verifikator.
    let (existing,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)::bigint FROM document_submissions
        WHERE user_id = $1 AND purpose = $2
          AND status IN ('draft', 'diajukan', 'diperiksa', 'revisi')
        "#,
    )
    .bind(user.id)
    .bind(&body.purpose)
    .fetch_one(&state.db)
    .await?;

    if existing > 0 {
        return Err(ApiError::Conflict(
            "Anda masih memiliki pengajuan aktif untuk keperluan ini. Selesaikan \
             atau batalkan pengajuan tersebut terlebih dahulu."
                .into(),
        ));
    }

    let (id,): (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO document_submissions (user_id, school_id, purpose, period, title, note)
        VALUES ($1,$2,$3,$4,$5,$6)
        RETURNING id
        "#,
    )
    .bind(user.id)
    .bind(user.school_id)
    .bind(&body.purpose)
    .bind(body.period.as_deref())
    .bind(body.title.trim())
    .bind(body.note.as_deref())
    .fetch_one(&state.db)
    .await?;

    record_event(&state, id, "draft", "Pengajuan dibuat", Some(&user)).await?;

    Ok(ApiResponse::with_message(
        fetch_submission(&state, &user, id).await?,
        "Pengajuan dibuat. Unggah berkas sesuai daftar periksa, lalu tekan Ajukan.",
    ))
}

/// Unggah satu berkas ke pengajuan.
#[utoipa::path(
    post, path = "/v1/documents/submissions/{id}/files", tag = "Pemberkasan",
    params(("id" = Uuid, Path, description = "ID pengajuan")),
    request_body = UploadFileRequest,
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Berkas terunggah", body = UploadFileResponse),
        (status = 409, description = "Pengajuan sudah dikunci")
    )
)]
pub async fn upload_file(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ValidJson(body): ValidJson<UploadFileRequest>,
) -> ApiResult<ApiResponse<UploadFileResponse>> {
    user.require("create_document_submission")?;
    let submission = fetch_submission(&state, &user, id).await?;

    if submission.user_id != user.id {
        return Err(ApiError::Forbidden(
            "Anda hanya dapat mengunggah berkas pada pengajuan sendiri".into(),
        ));
    }
    if !EDITABLE_STATUSES.contains(&submission.status.as_str()) {
        return Err(ApiError::Conflict(format!(
            "Pengajuan berstatus `{}` sudah dikunci dan tidak dapat diubah.",
            submission.status
        )));
    }

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(strip_data_uri(&body.content_base64))
        .map_err(|_| ApiError::field("content_base64", "berkas bukan base64 yang valid"))?;

    // Aturan per jenis dokumen (ukuran & tipe) diambil dari database, bukan
    // dikodekan di sini, agar operator bisa menyesuaikannya tanpa rilis baru.
    let (max_bytes, allowed_mime, type_name) = match body.document_type_id {
        Some(type_id) => {
            let row: Option<(i32, Vec<String>, String)> = sqlx::query_as(
                "SELECT max_bytes, allowed_mime, name FROM document_types
                 WHERE id = $1 AND is_active",
            )
            .bind(type_id)
            .fetch_optional(&state.db)
            .await?;
            let row = row.ok_or_else(|| {
                ApiError::field("document_type_id", "jenis dokumen tidak dikenal")
            })?;
            (row.0 as usize, row.1, Some(row.2))
        }
        None => (
            state.cfg.max_upload_bytes,
            vec![
                "application/pdf".to_string(),
                "image/jpeg".to_string(),
                "image/png".to_string(),
            ],
            None,
        ),
    };

    if bytes.len() > max_bytes {
        return Err(ApiError::field(
            "content_base64",
            &format!(
                "ukuran berkas {} KB melebihi batas {} KB untuk jenis dokumen ini",
                bytes.len() / 1024,
                max_bytes / 1024
            ),
        ));
    }

    let mime = sniff_document_mime(&bytes)?;
    if !allowed_mime.iter().any(|m| m == mime) {
        return Err(ApiError::field(
            "content_base64",
            &format!(
                "tipe berkas {mime} tidak diizinkan; gunakan {}",
                allowed_mime.join(" atau ")
            ),
        ));
    }

    let ext = match mime {
        "application/pdf" => "pdf",
        "image/png" => "png",
        _ => "jpg",
    };
    let key = format!("documents/{}/{}.{ext}", submission.user_id, Uuid::new_v4());
    let sha = vector::sha256(&bytes);

    state.storage.put(&key, &bytes).await?;

    // Unggahan baru untuk jenis dokumen yang sama menggantikan yang lama —
    // guru yang salah unggah tidak perlu menghubungi operator.
    let old_key: Option<(String,)> = if let Some(type_id) = body.document_type_id {
        sqlx::query_as(
            "SELECT file_key FROM document_files
             WHERE submission_id = $1 AND document_type_id = $2",
        )
        .bind(id)
        .bind(type_id)
        .fetch_optional(&state.db)
        .await?
    } else {
        None
    };

    let (file_id,): (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO document_files
            (submission_id, document_type_id, file_key, original_name, mime_type, bytes, sha256)
        VALUES ($1,$2,$3,$4,$5,$6,$7)
        ON CONFLICT (submission_id, document_type_id) WHERE document_type_id IS NOT NULL
        DO UPDATE SET file_key = EXCLUDED.file_key,
                      original_name = EXCLUDED.original_name,
                      mime_type = EXCLUDED.mime_type,
                      bytes = EXCLUDED.bytes,
                      sha256 = EXCLUDED.sha256,
                      status = 'menunggu',
                      reject_reason = NULL,
                      reviewed_by = NULL,
                      reviewed_at = NULL,
                      uploaded_at = NOW()
        RETURNING id
        "#,
    )
    .bind(id)
    .bind(body.document_type_id)
    .bind(&key)
    .bind(body.original_name.trim())
    .bind(mime)
    .bind(bytes.len() as i32)
    .bind(&sha)
    .fetch_one(&state.db)
    .await?;

    if let Some((old,)) = old_key {
        if old != key {
            let _ = state.storage.delete(&old).await;
        }
    }

    let missing = missing_required(&state, &submission.purpose, id).await?;

    Ok(ApiResponse::with_message(
        UploadFileResponse {
            file_id,
            document_type_name: type_name.clone(),
            bytes: bytes.len() as i32,
            missing_required: missing.clone(),
            message: if missing.is_empty() {
                "Seluruh dokumen wajib sudah lengkap. Anda dapat mengajukan berkas.".into()
            } else {
                format!("Masih kurang {} dokumen wajib.", missing.len())
            },
        },
        type_name
            .map(|n| format!("{n} berhasil diunggah"))
            .unwrap_or_else(|| "Berkas berhasil diunggah".into()),
    ))
}

/// Hapus satu berkas dari pengajuan yang masih bisa diubah.
#[utoipa::path(
    delete, path = "/v1/documents/files/{file_id}", tag = "Pemberkasan",
    params(("file_id" = Uuid, Path, description = "ID berkas")),
    security(("bearer" = [])),
    responses((status = 200, description = "Berkas dihapus"))
)]
pub async fn delete_file(
    State(state): State<AppState>,
    user: AuthUser,
    Path(file_id): Path<Uuid>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require("create_document_submission")?;

    let row: Option<(Uuid, Uuid, String, String)> = sqlx::query_as(
        r#"
        SELECT f.id, s.user_id, s.status, f.file_key
        FROM document_files f
        JOIN document_submissions s ON s.id = f.submission_id
        WHERE f.id = $1
        "#,
    )
    .bind(file_id)
    .fetch_optional(&state.db)
    .await?;

    let (_, owner, status, key) =
        row.ok_or_else(|| ApiError::NotFound(format!("berkas `{file_id}`")))?;

    if owner != user.id {
        return Err(ApiError::Forbidden(
            "Anda hanya dapat menghapus berkas pada pengajuan sendiri".into(),
        ));
    }
    if !EDITABLE_STATUSES.contains(&status.as_str()) {
        return Err(ApiError::Conflict(
            "Pengajuan sudah dikunci dan berkasnya tidak dapat dihapus".into(),
        ));
    }

    sqlx::query("DELETE FROM document_files WHERE id = $1")
        .bind(file_id)
        .execute(&state.db)
        .await?;
    let _ = state.storage.delete(&key).await;

    Ok(ApiResponse::with_message(
        serde_json::json!({ "deleted": true }),
        "Berkas dihapus",
    ))
}

/// Ajukan pengajuan untuk diperiksa.
#[utoipa::path(
    post, path = "/v1/documents/submissions/{id}/submit", tag = "Pemberkasan",
    params(("id" = Uuid, Path, description = "ID pengajuan")),
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Pengajuan dikirim"),
        (status = 422, description = "Masih ada dokumen wajib yang belum diunggah")
    )
)]
pub async fn submit(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require("create_document_submission")?;
    let submission = fetch_submission(&state, &user, id).await?;

    if submission.user_id != user.id {
        return Err(ApiError::Forbidden(
            "Anda hanya dapat mengajukan berkas sendiri".into(),
        ));
    }
    if !EDITABLE_STATUSES.contains(&submission.status.as_str()) {
        return Err(ApiError::Conflict(format!(
            "Pengajuan sudah berstatus `{}`.",
            submission.status
        )));
    }

    // Dicek di sini, bukan hanya di aplikasi: pengajuan tak lengkap yang
    // lolos akan memantul bolak-balik antara guru dan verifikator.
    let missing = missing_required(&state, &submission.purpose, id).await?;
    if !missing.is_empty() {
        return Err(ApiError::validation(vec![crate::error::FieldError::new(
            "files",
            format!(
                "Dokumen wajib yang belum diunggah: {}.",
                missing.join(", ")
            ),
        )]));
    }

    sqlx::query(
        "UPDATE document_submissions
            SET status = 'diajukan', submitted_at = NOW(), updated_at = NOW()
          WHERE id = $1",
    )
    .bind(id)
    .execute(&state.db)
    .await?;

    record_event(&state, id, "diajukan", "Pengajuan dikirim untuk diperiksa", Some(&user)).await?;

    Ok(ApiResponse::with_message(
        serde_json::json!({ "status": "diajukan" }),
        "Pengajuan terkirim. Berkas kini terkunci dan menunggu pemeriksaan.",
    ))
}

/// Verifikasi pengajuan (kepala sekolah / dinas).
#[utoipa::path(
    post, path = "/v1/documents/submissions/{id}/review", tag = "Pemberkasan",
    params(("id" = Uuid, Path, description = "ID pengajuan")),
    request_body = ReviewSubmissionRequest,
    security(("bearer" = [])),
    responses((status = 200, description = "Hasil pemeriksaan disimpan"))
)]
pub async fn review_submission(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ValidJson(body): ValidJson<ReviewSubmissionRequest>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require("verify_document_submission")?;
    body.validate_transition()?;

    let submission = fetch_submission(&state, &user, id).await?;

    if submission.user_id == user.id {
        return Err(ApiError::Forbidden(
            "Anda tidak dapat memverifikasi pengajuan Anda sendiri".into(),
        ));
    }
    if submission.status == "draft" {
        return Err(ApiError::Conflict(
            "Pengajuan masih berupa draft dan belum dikirim pemiliknya".into(),
        ));
    }

    sqlx::query(
        r#"
        UPDATE document_submissions
           SET status = $2, reviewed_by = $3, reviewed_at = NOW(),
               review_note = $4, updated_at = NOW()
         WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(&body.status)
    .bind(user.id)
    .bind(body.note.trim())
    .execute(&state.db)
    .await?;

    record_event(&state, id, &body.status, body.note.trim(), Some(&user)).await?;

    AuditEntry::by_user(&user, "document.review")
        .entity("document_submission", id)
        .after(&serde_json::json!({ "status": body.status }))
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        serde_json::json!({ "status": body.status }),
        "Hasil pemeriksaan disimpan",
    ))
}

/// Setujui / tolak satu berkas.
#[utoipa::path(
    post, path = "/v1/documents/files/{file_id}/review", tag = "Pemberkasan",
    params(("file_id" = Uuid, Path, description = "ID berkas")),
    request_body = ReviewFileRequest,
    security(("bearer" = [])),
    responses((status = 200, description = "Status berkas diperbarui"))
)]
pub async fn review_file(
    State(state): State<AppState>,
    user: AuthUser,
    Path(file_id): Path<Uuid>,
    ValidJson(body): ValidJson<ReviewFileRequest>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require("verify_document_submission")?;

    if !matches!(body.status.as_str(), "disetujui" | "ditolak") {
        return Err(ApiError::field("status", "pilih `disetujui` atau `ditolak`"));
    }
    // Penolakan tanpa alasan memaksa guru menebak apa yang salah.
    if body.status == "ditolak" && body.reject_reason.as_deref().unwrap_or("").trim().len() < 5 {
        return Err(ApiError::field(
            "reject_reason",
            "sebutkan alasan penolakan agar berkas dapat diperbaiki",
        ));
    }

    let affected = sqlx::query(
        r#"
        UPDATE document_files
           SET status = $2, reject_reason = $3, reviewed_by = $4, reviewed_at = NOW()
         WHERE id = $1
        "#,
    )
    .bind(file_id)
    .bind(&body.status)
    .bind(body.reject_reason.as_deref())
    .bind(user.id)
    .execute(&state.db)
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(ApiError::NotFound(format!("berkas `{file_id}`")));
    }

    Ok(ApiResponse::with_message(
        serde_json::json!({ "status": body.status }),
        "Status berkas diperbarui",
    ))
}

// =====================================================================
// Helper
// =====================================================================

pub(crate) async fn fetch_submission(
    state: &AppState,
    user: &AuthUser,
    id: Uuid,
) -> ApiResult<SubmissionListItem> {
    let row: Option<SubmissionListItem> = sqlx::query_as(
        r#"
        SELECT s.id, s.user_id, u.name AS owner_name, s.school_id,
               sc.name AS school_name, s.purpose, s.period, s.title, s.status,
               s.file_count, s.approved_file_count, s.rejected_file_count,
               s.submitted_at, s.reviewed_at, s.created_at
        FROM document_submissions s
        JOIN users u ON u.id = s.user_id
        LEFT JOIN schools sc ON sc.id = s.school_id
        WHERE s.id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    let row = row.ok_or_else(|| ApiError::NotFound(format!("pengajuan `{id}`")))?;

    // Pemilik selalu boleh. Selain itu perlu izin verifikasi DAN pengajuan
    // harus berada dalam cakupan sekolahnya.
    if row.user_id == user.id {
        return Ok(row);
    }
    if !user.has_permission("verify_document_submission") {
        return Err(ApiError::NotFound(format!("pengajuan `{id}`")));
    }
    if let Some(school_id) = row.school_id {
        user.resolve_school(Some(school_id))?;
    }
    if row.status == "draft" {
        return Err(ApiError::NotFound(format!("pengajuan `{id}`")));
    }

    Ok(row)
}

/// Daftar periksa: jenis dokumen yang diminta beserta status keterisiannya.
async fn build_checklist(
    state: &AppState,
    purpose: &str,
    submission_id: Uuid,
) -> ApiResult<Vec<ChecklistItem>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        document_type_id: Uuid,
        code: String,
        name: String,
        description: Option<String>,
        is_required: bool,
        uploaded: bool,
        status: Option<String>,
        reject_reason: Option<String>,
    }

    let rows: Vec<Row> = sqlx::query_as(
        r#"
        SELECT dt.id AS document_type_id, dt.code, dt.name, dt.description,
               dt.is_required,
               (f.id IS NOT NULL) AS uploaded,
               f.status, f.reject_reason
        FROM document_types dt
        LEFT JOIN document_files f
               ON f.document_type_id = dt.id AND f.submission_id = $2
        WHERE dt.is_active AND dt.purpose = $1
        ORDER BY dt.sort_order, dt.name
        "#,
    )
    .bind(purpose)
    .bind(submission_id)
    .fetch_all(&state.db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ChecklistItem {
            document_type_id: r.document_type_id,
            code: r.code,
            name: r.name,
            description: r.description,
            is_required: r.is_required,
            uploaded: r.uploaded,
            status: r.status,
            reject_reason: r.reject_reason,
        })
        .collect())
}

/// Nama dokumen wajib yang belum diunggah.
async fn missing_required(
    state: &AppState,
    purpose: &str,
    submission_id: Uuid,
) -> ApiResult<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT dt.name FROM document_types dt
        WHERE dt.is_active AND dt.is_required AND dt.purpose = $1
          AND NOT EXISTS (
              SELECT 1 FROM document_files f
              WHERE f.submission_id = $2 AND f.document_type_id = dt.id
          )
        ORDER BY dt.sort_order
        "#,
    )
    .bind(purpose)
    .bind(submission_id)
    .fetch_all(&state.db)
    .await?;

    Ok(rows.into_iter().map(|r| r.0).collect())
}

async fn record_event(
    state: &AppState,
    submission_id: Uuid,
    status: &str,
    note: &str,
    actor: Option<&AuthUser>,
) -> ApiResult<()> {
    sqlx::query(
        r#"
        INSERT INTO document_submission_events
            (submission_id, status, note, actor_user_id, actor_label)
        VALUES ($1,$2,$3,$4,$5)
        "#,
    )
    .bind(submission_id)
    .bind(status)
    .bind(note)
    .bind(actor.map(|a| a.id))
    .bind(actor.map(|a| format!("{} ({})", a.name, a.role_label())))
    .execute(&state.db)
    .await?;
    Ok(())
}

/// Deteksi tipe berkas dari magic bytes; header dari klien tidak dipercaya.
fn sniff_document_mime(bytes: &[u8]) -> ApiResult<&'static str> {
    if bytes.len() < 8 {
        return Err(ApiError::field("content_base64", "berkas terlalu kecil"));
    }
    if bytes.starts_with(b"%PDF-") {
        Ok("application/pdf")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Ok("image/jpeg")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Ok("image/png")
    } else {
        Err(ApiError::field(
            "content_base64",
            "format berkas tidak dikenali; gunakan PDF, JPEG, atau PNG",
        ))
    }
}

fn strip_data_uri(input: &str) -> &str {
    let trimmed = input.trim();
    match trimmed.find(";base64,") {
        Some(idx) if trimmed.starts_with("data:") => &trimmed[idx + 8..],
        _ => trimmed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_dikenali_dari_magic_bytes() {
        let mut pdf = b"%PDF-1.7".to_vec();
        pdf.extend_from_slice(&[0u8; 8]);
        assert_eq!(sniff_document_mime(&pdf).unwrap(), "application/pdf");

        let mut jpg = vec![0xFF, 0xD8, 0xFF, 0xE0];
        jpg.extend_from_slice(&[0u8; 8]);
        assert_eq!(sniff_document_mime(&jpg).unwrap(), "image/jpeg");

        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        png.extend_from_slice(&[0u8; 4]);
        assert_eq!(sniff_document_mime(&png).unwrap(), "image/png");
    }

    #[test]
    fn berkas_dengan_ekstensi_palsu_ditolak() {
        // Nama berkas "ijazah.pdf" tidak berarti isinya PDF.
        let fake = b"MZ\x90\x00\x03\x00\x00\x00executable".to_vec();
        assert!(sniff_document_mime(&fake).is_err());
    }

    #[test]
    fn berkas_kosong_ditolak() {
        assert!(sniff_document_mime(b"%PD").is_err());
    }

    #[test]
    fn data_uri_dibuang() {
        assert_eq!(strip_data_uri("data:application/pdf;base64,QUJD"), "QUJD");
        assert_eq!(strip_data_uri("QUJD"), "QUJD");
    }
}
