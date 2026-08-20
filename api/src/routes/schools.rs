//! Master data sekolah, wilayah, dan tahun ajaran.
//!
//! Hanya peran tingkat provinsi yang boleh membuat/menghapus sekolah.
//! Pengguna tingkat sekolah tetap boleh MEMBACA sekolahnya sendiri (dipakai
//! untuk menampilkan kop laporan dan mengatur ambang pengenalan).

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::Router;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::domain::school::{
    AcademicYear, CreateSchoolRequest, Region, School, SchoolFilter, SchoolListItem,
    UpdateSchoolRequest, SCHOOL_JENJANG, SCHOOL_STATUS,
};
use crate::error::{ApiError, ApiResult};
use crate::extract::{ValidJson, ValidQuery};
use crate::services::audit::AuditEntry;
use crate::services::notify;
use crate::state::AppState;
use crate::util::{ApiResponse, PageQuery, Paginated};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/schools", get(list).post(create))
        .route(
            "/schools/{id}",
            get(detail).patch(update).delete(soft_delete),
        )
        .route("/schools/{id}/restore", post(restore))
        .route("/regions", get(list_regions))
        .route("/academic-years", get(list_academic_years))
}

const SORTABLE: [&str; 5] = ["name", "npsn", "jenjang", "created_at", "student_count"];

/// Daftar sekolah beserta jumlah siswa, cakupan wajah, dan jumlah perangkat.
#[utoipa::path(
    get, path = "/v1/schools", tag = "Sekolah",
    params(PageQuery, SchoolFilter),
    security(("bearer" = [])),
    responses((status = 200, description = "Daftar sekolah", body = [SchoolListItem]))
)]
pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(page): ValidQuery<PageQuery>,
    ValidQuery(filter): ValidQuery<SchoolFilter>,
) -> ApiResult<Paginated<SchoolListItem>> {
    user.require("view_school")?;

    // Pengguna sekolah hanya melihat sekolahnya. Superadmin melihat semua.
    let scoped = user.accessible_schools();
    let search = page.search_pattern();

    let (total,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)::bigint FROM schools s
        WHERE s.deleted_at IS NULL
          AND ($1::uuid[] IS NULL OR s.id = ANY($1))
          AND ($2::text IS NULL OR s.jenjang = $2)
          AND ($3::text IS NULL OR s.status = $3)
          AND ($4::uuid IS NULL OR s.region_id = $4)
          AND ($5::boolean IS NULL OR s.is_active = $5)
          AND ($6::text IS NULL OR s.name ILIKE $6 OR s.npsn ILIKE $6)
        "#,
    )
    .bind(scoped.as_deref())
    .bind(filter.jenjang.as_deref())
    .bind(filter.status.as_deref())
    .bind(filter.region_id)
    .bind(filter.is_active)
    .bind(search.as_deref())
    .fetch_one(&state.db)
    .await?;

    // Sub-select terpisah untuk tiap hitungan: lebih cepat daripada tiga LEFT
    // JOIN + GROUP BY pada tabel berisi ratusan ribu siswa.
    let sql = format!(
        r#"
        SELECT s.id, s.npsn, s.name, s.jenjang, s.status,
               r.name AS region_name, s.is_active,
               (SELECT COUNT(*)::bigint FROM students st
                 WHERE st.school_id = s.id AND st.deleted_at IS NULL AND st.status = 'aktif')
                 AS student_count,
               (SELECT COUNT(*)::bigint FROM students st
                 WHERE st.school_id = s.id AND st.deleted_at IS NULL
                   AND st.status = 'aktif' AND st.face_enrolled)
                 AS enrolled_face_count,
               (SELECT COUNT(*)::bigint FROM devices d
                 WHERE d.school_id = s.id AND d.deleted_at IS NULL AND d.is_active)
                 AS device_count
        FROM schools s
        LEFT JOIN regions r ON r.id = s.region_id
        WHERE s.deleted_at IS NULL
          AND ($1::uuid[] IS NULL OR s.id = ANY($1))
          AND ($2::text IS NULL OR s.jenjang = $2)
          AND ($3::text IS NULL OR s.status = $3)
          AND ($4::uuid IS NULL OR s.region_id = $4)
          AND ($5::boolean IS NULL OR s.is_active = $5)
          AND ($6::text IS NULL OR s.name ILIKE $6 OR s.npsn ILIKE $6)
        ORDER BY {}
        LIMIT $7 OFFSET $8
        "#,
        page.order_by(&SORTABLE, "name")
    );

    let items: Vec<SchoolListItem> = sqlx::query_as(&sql)
        .bind(scoped.as_deref())
        .bind(filter.jenjang.as_deref())
        .bind(filter.status.as_deref())
        .bind(filter.region_id)
        .bind(filter.is_active)
        .bind(search.as_deref())
        .bind(page.per_page())
        .bind(page.offset())
        .fetch_all(&state.db)
        .await?;

    Ok(Paginated::new(items, page.page(), page.per_page(), total))
}

/// Detail satu sekolah.
#[utoipa::path(
    get, path = "/v1/schools/{id}", tag = "Sekolah",
    params(("id" = Uuid, Path, description = "ID sekolah")),
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Detail sekolah", body = School),
        (status = 403, description = "Bukan sekolah Anda"),
        (status = 404, description = "Tidak ditemukan")
    )
)]
pub async fn detail(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<School>> {
    user.require("view_school")?;
    user.resolve_school(Some(id))?;

    let school = fetch_school(&state, id).await?;
    Ok(ApiResponse::new(school))
}

/// Tambah sekolah baru (khusus Superadmin/Dinas).
#[utoipa::path(
    post, path = "/v1/schools", tag = "Sekolah",
    request_body = CreateSchoolRequest,
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Sekolah dibuat", body = School),
        (status = 409, description = "NPSN sudah terdaftar")
    )
)]
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    ValidJson(body): ValidJson<CreateSchoolRequest>,
) -> ApiResult<ApiResponse<School>> {
    user.require("create_school")?;
    validate_enums(&body.jenjang, &body.status)?;

    let slug = slugify(&body.name, &body.npsn);

    let (id,): (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO schools (
            npsn, name, slug, jenjang, status, region_id, address, village, district,
            postal_code, latitude, longitude, geofence_radius_m, phone, email,
            principal_name, face_match_threshold
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,COALESCE($13,250),$14,$15,$16,$17)
        RETURNING id
        "#,
    )
    .bind(body.npsn.trim())
    .bind(body.name.trim())
    .bind(&slug)
    .bind(&body.jenjang)
    .bind(&body.status)
    .bind(body.region_id)
    .bind(body.address.as_deref())
    .bind(body.village.as_deref())
    .bind(body.district.as_deref())
    .bind(body.postal_code.as_deref())
    .bind(body.latitude)
    .bind(body.longitude)
    .bind(body.geofence_radius_m)
    .bind(body.phone.as_deref())
    .bind(body.email.as_deref())
    .bind(body.principal_name.as_deref())
    .bind(body.face_match_threshold)
    .fetch_one(&state.db)
    .await?;

    // Sekolah baru langsung mendapat kebijakan notifikasi default supaya
    // operator tidak perlu mengonfigurasi apa pun sebelum bisa dipakai.
    notify::ensure_policy(&state.db, id).await?;

    let school = fetch_school(&state, id).await?;
    AuditEntry::by_user(&user, "school.create")
        .school(id)
        .entity("school", id)
        .after(&school)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        school,
        format!("Sekolah {} berhasil ditambahkan", body.name.trim()),
    ))
}

/// Perbarui data sekolah.
#[utoipa::path(
    patch, path = "/v1/schools/{id}", tag = "Sekolah",
    params(("id" = Uuid, Path, description = "ID sekolah")),
    request_body = UpdateSchoolRequest,
    security(("bearer" = [])),
    responses((status = 200, description = "Sekolah diperbarui", body = School))
)]
pub async fn update(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ValidJson(body): ValidJson<UpdateSchoolRequest>,
) -> ApiResult<ApiResponse<School>> {
    user.require("update_school")?;
    user.resolve_school(Some(id))?;

    if let Some(j) = &body.jenjang {
        if !SCHOOL_JENJANG.contains(&j.as_str()) {
            return Err(ApiError::field("jenjang", "jenjang tidak dikenal"));
        }
    }
    if let Some(s) = &body.status {
        if !SCHOOL_STATUS.contains(&s.as_str()) {
            return Err(ApiError::field("status", "status harus negeri atau swasta"));
        }
    }

    let before = fetch_school(&state, id).await?;

    // COALESCE membuat field yang tidak dikirim tetap seperti semula —
    // PATCH parsial tanpa perlu membangun SQL dinamis.
    sqlx::query(
        r#"
        UPDATE schools SET
            name              = COALESCE($2, name),
            jenjang           = COALESCE($3, jenjang),
            status            = COALESCE($4, status),
            region_id         = COALESCE($5, region_id),
            address           = COALESCE($6, address),
            village           = COALESCE($7, village),
            district          = COALESCE($8, district),
            postal_code       = COALESCE($9, postal_code),
            latitude          = COALESCE($10, latitude),
            longitude         = COALESCE($11, longitude),
            geofence_radius_m = COALESCE($12, geofence_radius_m),
            phone             = COALESCE($13, phone),
            email             = COALESCE($14, email),
            principal_name    = COALESCE($15, principal_name),
            face_match_threshold = COALESCE($16, face_match_threshold),
            is_active         = COALESCE($17, is_active),
            updated_at        = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(body.name.as_deref().map(str::trim))
    .bind(body.jenjang.as_deref())
    .bind(body.status.as_deref())
    .bind(body.region_id)
    .bind(body.address.as_deref())
    .bind(body.village.as_deref())
    .bind(body.district.as_deref())
    .bind(body.postal_code.as_deref())
    .bind(body.latitude)
    .bind(body.longitude)
    .bind(body.geofence_radius_m)
    .bind(body.phone.as_deref())
    .bind(body.email.as_deref())
    .bind(body.principal_name.as_deref())
    .bind(body.face_match_threshold)
    .bind(body.is_active)
    .execute(&state.db)
    .await?;

    let after = fetch_school(&state, id).await?;

    // Ambang kemiripan berubah -> paksa muat ulang index agar berlaku segera.
    if before.face_match_threshold != after.face_match_threshold {
        state.broadcast_face_invalidation(id).await;
    }

    AuditEntry::by_user(&user, "school.update")
        .school(id)
        .entity("school", id)
        .before(&before)
        .after(&after)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(after, "Data sekolah diperbarui"))
}

/// Arsipkan sekolah (soft delete).
///
/// Data absensi historis TIDAK dihapus — laporan tahun-tahun sebelumnya
/// harus tetap bisa dibuka. Sekolah hanya disembunyikan dan perangkatnya
/// dinonaktifkan.
#[utoipa::path(
    delete, path = "/v1/schools/{id}", tag = "Sekolah",
    params(("id" = Uuid, Path, description = "ID sekolah")),
    security(("bearer" = [])),
    responses((status = 200, description = "Sekolah diarsipkan"))
)]
pub async fn soft_delete(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require("delete_school")?;

    let mut tx = state.db.begin().await?;
    let affected = sqlx::query(
        "UPDATE schools SET deleted_at = NOW(), is_active = FALSE WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    if affected == 0 {
        tx.rollback().await?;
        return Err(ApiError::NotFound(format!("sekolah `{id}`")));
    }

    sqlx::query("UPDATE devices SET is_active = FALSE, token_revoked_at = NOW() WHERE school_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    state.invalidate_device_cache();
    state.face_index.invalidate(id);

    AuditEntry::by_user(&user, "school.archive")
        .school(id)
        .entity("school", id)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        serde_json::json!({ "archived": true, "school_id": id }),
        "Sekolah diarsipkan. Data absensi historis tetap tersimpan.",
    ))
}

/// Pulihkan sekolah yang diarsipkan.
#[utoipa::path(
    post, path = "/v1/schools/{id}/restore", tag = "Sekolah",
    params(("id" = Uuid, Path, description = "ID sekolah")),
    security(("bearer" = [])),
    responses((status = 200, description = "Sekolah dipulihkan"))
)]
pub async fn restore(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<School>> {
    user.require("delete_school")?;

    let affected = sqlx::query(
        "UPDATE schools SET deleted_at = NULL, is_active = TRUE WHERE id = $1 AND deleted_at IS NOT NULL",
    )
    .bind(id)
    .execute(&state.db)
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(ApiError::NotFound(format!("sekolah terarsip `{id}`")));
    }

    AuditEntry::by_user(&user, "school.restore")
        .school(id)
        .entity("school", id)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        fetch_school(&state, id).await?,
        "Sekolah dipulihkan",
    ))
}

/// Daftar kabupaten/kota beserta jumlah sekolahnya.
#[utoipa::path(
    get, path = "/v1/regions", tag = "Sekolah",
    security(("bearer" = [])),
    responses((status = 200, description = "Daftar wilayah", body = [Region]))
)]
pub async fn list_regions(
    State(state): State<AppState>,
    _user: AuthUser,
) -> ApiResult<ApiResponse<Vec<Region>>> {
    let rows: Vec<Region> = sqlx::query_as(
        r#"
        SELECT r.id, r.code, r.name, r.kind,
               (SELECT COUNT(*)::bigint FROM schools s
                 WHERE s.region_id = r.id AND s.deleted_at IS NULL) AS school_count
        FROM regions r
        ORDER BY r.kind DESC, r.name
        "#,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(ApiResponse::new(rows))
}

/// Daftar tahun ajaran.
#[utoipa::path(
    get, path = "/v1/academic-years", tag = "Sekolah",
    security(("bearer" = [])),
    responses((status = 200, description = "Daftar tahun ajaran", body = [AcademicYear]))
)]
pub async fn list_academic_years(
    State(state): State<AppState>,
    _user: AuthUser,
) -> ApiResult<ApiResponse<Vec<AcademicYear>>> {
    let rows: Vec<AcademicYear> = sqlx::query_as(
        "SELECT id, name, start_date, end_date, is_active FROM academic_years ORDER BY start_date DESC",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(ApiResponse::new(rows))
}

// =====================================================================
// Helper
// =====================================================================

pub async fn fetch_school(state: &AppState, id: Uuid) -> ApiResult<School> {
    let row: Option<School> = sqlx::query_as(
        r#"
        SELECT s.id, s.npsn, s.name, s.slug, s.jenjang, s.status, s.region_id,
               r.name AS region_name, s.address, s.village, s.district, s.postal_code,
               s.latitude, s.longitude, s.geofence_radius_m, s.phone, s.email,
               s.principal_name, s.logo_path, s.timezone, s.face_match_threshold,
               s.is_active, s.created_at, s.updated_at
        FROM schools s
        LEFT JOIN regions r ON r.id = s.region_id
        WHERE s.id = $1 AND s.deleted_at IS NULL
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    row.ok_or_else(|| ApiError::NotFound(format!("sekolah `{id}`")))
}

/// Tahun ajaran aktif; dipakai sebagai default saat membuat kelas.
pub async fn active_academic_year(state: &AppState) -> ApiResult<Uuid> {
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM academic_years WHERE is_active LIMIT 1")
            .fetch_optional(&state.db)
            .await?;
    row.map(|r| r.0).ok_or_else(|| {
        ApiError::Conflict(
            "Belum ada tahun ajaran aktif. Aktifkan tahun ajaran terlebih dahulu.".into(),
        )
    })
}

fn validate_enums(jenjang: &str, status: &str) -> ApiResult<()> {
    if !SCHOOL_JENJANG.contains(&jenjang) {
        return Err(ApiError::field(
            "jenjang",
            &format!("pilih salah satu: {}", SCHOOL_JENJANG.join(", ")),
        ));
    }
    if !SCHOOL_STATUS.contains(&status) {
        return Err(ApiError::field("status", "pilih negeri atau swasta"));
    }
    Ok(())
}

/// Slug unik & aman URL. NPSN disertakan agar dua sekolah bernama sama
/// (sangat umum: "SD Negeri 1") tidak bertabrakan.
fn slugify(name: &str, npsn: &str) -> String {
    let base: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let mut cleaned = String::with_capacity(base.len());
    let mut prev_dash = false;
    for c in base.chars() {
        if c == '-' {
            if !prev_dash && !cleaned.is_empty() {
                cleaned.push('-');
            }
            prev_dash = true;
        } else {
            cleaned.push(c);
            prev_dash = false;
        }
    }
    let trimmed = cleaned.trim_matches('-');
    let head: String = trimmed.chars().take(180).collect();
    format!("{}-{}", head.trim_matches('-'), npsn.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_aman_url() {
        assert_eq!(
            slugify("SMA Negeri 1 Medan", "10259876"),
            "sma-negeri-1-medan-10259876"
        );
    }

    #[test]
    fn slug_menghapus_karakter_ganda_dan_khusus() {
        assert_eq!(
            slugify("SD  Negeri // 05 (Baru)", "12345678"),
            "sd-negeri-05-baru-12345678"
        );
    }

    #[test]
    fn slug_menyertakan_npsn_agar_unik() {
        let a = slugify("SD Negeri 1", "11111111");
        let b = slugify("SD Negeri 1", "22222222");
        assert_ne!(a, b);
    }

    #[test]
    fn slug_tidak_diawali_atau_diakhiri_tanda_hubung() {
        let s = slugify("--- Sekolah ---", "99999999");
        assert!(!s.starts_with('-'), "slug = {s}");
        assert_eq!(s, "sekolah-99999999");
    }

    #[test]
    fn enum_jenjang_divalidasi() {
        assert!(validate_enums("SMA", "negeri").is_ok());
        assert!(validate_enums("KULIAH", "negeri").is_err());
        assert!(validate_enums("SMA", "internasional").is_err());
    }
}
