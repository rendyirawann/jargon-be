//! Penyajian berkas tersimpan (foto pendaftaran wajah, hasil ekspor).
//!
//! Berkas TIDAK dilayani langsung oleh reverse proxy, sengaja: foto wajah
//! siswa adalah data biometrik. Setiap permintaan harus melewati pemeriksaan
//! bahwa pemintanya berwenang atas sekolah siswa tersebut. Object key memuat
//! UUID acak, tapi "URL yang sulit ditebak" bukan kontrol akses.

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/files/{*key}", get(serve))
}

/// Sajikan berkas berdasarkan object key.
#[utoipa::path(
    get, path = "/files/{key}", tag = "Berkas",
    params(("key" = String, Path, description = "Object key, mis. faces/<school>/<student>/<id>.jpg")),
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Isi berkas", content_type = "application/octet-stream"),
        (status = 403, description = "Bukan sekolah Anda"),
        (status = 404, description = "Berkas tidak ditemukan")
    )
)]
pub async fn serve(
    State(state): State<AppState>,
    user: AuthUser,
    Path(key): Path<String>,
) -> ApiResult<Response> {
    authorize(&state, &user, &key).await?;

    let bytes = state.storage.get(&key).await?;
    let mime = guess_mime(&key);

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    // Foto wajah tidak boleh di-cache oleh proxy bersama; hanya browser
    // pengguna yang sudah lolos otorisasi.
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=300"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("inline"),
    );
    // Berkas ini tidak pernah dimaksudkan untuk ditafsirkan sebagai HTML.
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );

    Ok((StatusCode::OK, headers, bytes).into_response())
}

/// Otorisasi berdasarkan struktur object key.
///
/// ```text
/// faces/<school_id>/<student_id>/<uuid>.jpg
/// exports/<school_id|provinsi>/...
/// imports/<school_id|provinsi>/...
/// documents/<user_id>/<uuid>.<ext>
/// panic/<yyyymm>/<uuid>.jpg
/// ```
///
/// Dua bentuk terakhir TIDAK memuat school_id pada path-nya, jadi cakupannya
/// tidak bisa ditentukan dari key saja. Untuk keduanya, berkas ditelusuri
/// balik ke baris pemiliknya lalu diperiksa dengan aturan visibilitas yang
/// sama persis dengan endpoint yang menampilkannya. Menduplikasi aturan itu
/// di sini akan menghasilkan pintu belakang: berkas terlihat lewat `/files`
/// oleh orang yang tidak boleh membuka pengajuan atau laporannya.
pub async fn authorize(state: &AppState, user: &AuthUser, key: &str) -> ApiResult<()> {
    let mut parts = key.split('/');
    let kind = parts.next().unwrap_or("");
    let scope = parts.next().unwrap_or("");

    match kind {
        "faces" => {
            user.require("view_face_enrollment")?;
            let school_id = Uuid::parse_str(scope)
                .map_err(|_| ApiError::NotFound("berkas".into()))?;
            user.resolve_school(Some(school_id))?;

            // Pastikan berkas ini benar-benar tercatat sebagai milik sekolah
            // tersebut, bukan sekadar path yang dikarang penyerang.
            let row: Option<(Uuid,)> = sqlx::query_as(
                "SELECT id FROM face_enrollments WHERE image_key = $1 AND school_id = $2",
            )
            .bind(key)
            .bind(school_id)
            .fetch_optional(&state.db)
            .await?;
            row.map(|_| ()).ok_or_else(|| ApiError::NotFound("berkas".into()))
        }
        "documents" => {
            user.require("view_document_submission")?;

            let row: Option<(Uuid,)> = sqlx::query_as(
                "SELECT submission_id FROM document_files WHERE file_key = $1",
            )
            .bind(key)
            .fetch_optional(&state.db)
            .await?;

            let (submission_id,) = row.ok_or_else(|| ApiError::NotFound("berkas".into()))?;

            // Aturan yang sama dengan membuka pengajuannya: pemilik selalu
            // boleh, selain itu butuh izin verifikasi dan cakupan sekolah.
            // Berkasnya memuat NIK, nomor rekening, dan ijazah.
            super::documents::fetch_submission(state, user, submission_id)
                .await
                .map(|_| ())
                .map_err(|_| ApiError::NotFound("berkas".into()))
        }
        "panic" => {
            user.require("view_panic_feed")?;

            let row: Option<(Uuid,)> =
                sqlx::query_as("SELECT report_id FROM panic_report_media WHERE file_key = $1")
                    .bind(key)
                    .fetch_optional(&state.db)
                    .await?;

            let (report_id,) = row.ok_or_else(|| ApiError::NotFound("berkas".into()))?;

            // Lampiran mengikuti visibilitas laporannya, memakai aturan yang
            // sama persis — bukan salinannya.
            super::panic::authorize_media(state, user, report_id)
                .await
                .map_err(|_| ApiError::NotFound("berkas".into()))
        }
        "exports" | "imports" => {
            user.require_any(&["export_report", "import_student", "view_report"])?;
            if scope == "provinsi" {
                if !user.is_province_scope() {
                    return Err(ApiError::Forbidden(
                        "berkas ini milik tingkat provinsi".into(),
                    ));
                }
                Ok(())
            } else {
                let school_id = Uuid::parse_str(scope)
                    .map_err(|_| ApiError::NotFound("berkas".into()))?;
                user.resolve_school(Some(school_id))?;
                Ok(())
            }
        }
        _ => Err(ApiError::NotFound("berkas".into())),
    }
}

fn guess_mime(key: &str) -> &'static str {
    match key.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase()) {
        Some(ext) if ext == "jpg" || ext == "jpeg" => "image/jpeg",
        Some(ext) if ext == "png" => "image/png",
        Some(ext) if ext == "csv" => "text/csv",
        Some(ext) if ext == "xlsx" => {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        }
        Some(ext) if ext == "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_ditebak_dari_ekstensi() {
        assert_eq!(guess_mime("faces/a/b/c.jpg"), "image/jpeg");
        assert_eq!(guess_mime("faces/a/b/c.JPEG"), "image/jpeg");
        assert_eq!(guess_mime("faces/a/b/c.png"), "image/png");
        assert_eq!(guess_mime("exports/a/rekap.csv"), "text/csv");
        assert_eq!(guess_mime("documents/a/ijazah.pdf"), "application/pdf");
        assert_eq!(guess_mime("panic/202608/foto.jpg"), "image/jpeg");
        assert_eq!(guess_mime("tanpa-ekstensi"), "application/octet-stream");
    }
}
