//! Manajemen perangkat tablet + alur pairing.

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::Router;
use chrono::{Duration as ChronoDuration, Utc};
use uuid::Uuid;

use crate::auth::{password, AuthUser};
use crate::domain::device::{
    CreateDeviceRequest, Device, DeviceFilter, PairDeviceRequest, PairDeviceResponse,
    PairingCodeResponse, UpdateDeviceRequest, DEVICE_MODES, DEVICE_PLACEMENTS,
    PAIRING_TTL_MINUTES,
};
use crate::error::{ApiError, ApiResult};
use crate::extract::{ValidJson, ValidQuery};
use crate::face::vector;
use crate::routes::kiosk::runtime_config;
use crate::services::audit::AuditEntry;
use crate::state::AppState;
use crate::util::{ApiResponse, PageQuery, Paginated};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/devices", get(list).post(create))
        .route("/devices/{id}", get(detail).patch(update).delete(remove))
        .route("/devices/{id}/pairing-code", post(regenerate_pairing))
        .route("/devices/{id}/revoke", post(revoke))
}

/// Router tanpa autentikasi pengguna: kode pairing adalah kredensialnya.
pub fn public_router() -> Router<AppState> {
    Router::new().route("/devices/pair", post(pair))
}

/// Daftar perangkat beserta status online-nya.
#[utoipa::path(
    get, path = "/v1/devices", tag = "Perangkat",
    params(PageQuery, DeviceFilter),
    security(("bearer" = [])),
    responses((status = 200, description = "Daftar perangkat", body = [Device]))
)]
pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(page): ValidQuery<PageQuery>,
    ValidQuery(filter): ValidQuery<DeviceFilter>,
) -> ApiResult<Paginated<Device>> {
    user.require("view_device")?;
    let school = user.resolve_school(filter.school_id)?;
    let online_only = filter.online.unwrap_or(false);
    let search = page.search_pattern();

    let (total,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)::bigint FROM devices d
        WHERE d.deleted_at IS NULL
          AND ($1::uuid IS NULL OR d.school_id = $1)
          AND ($2::text IS NULL OR d.placement = $2)
          AND ($3::boolean IS NULL OR d.is_active = $3)
          AND (NOT $4::boolean OR d.last_seen_at > NOW() - INTERVAL '10 minutes')
          AND ($5::text IS NULL OR d.name ILIKE $5 OR d.code ILIKE $5)
        "#,
    )
    .bind(school)
    .bind(filter.placement.as_deref())
    .bind(filter.is_active)
    .bind(online_only)
    .bind(search.as_deref())
    .fetch_one(&state.db)
    .await?;

    let items: Vec<Device> = sqlx::query_as(
        r#"
        SELECT d.id, d.school_id, s.name AS school_name, d.code, d.name, d.placement,
               d.classroom_id, c.name AS classroom_name, d.mode,
               (d.token_hash IS NOT NULL AND d.token_revoked_at IS NULL) AS is_paired,
               d.app_version, d.os_version, d.last_seen_at, d.last_ip,
               (d.last_seen_at > NOW() - INTERVAL '10 minutes') AS is_online,
               d.is_active, d.created_at
        FROM devices d
        JOIN schools s ON s.id = d.school_id
        LEFT JOIN classrooms c ON c.id = d.classroom_id
        WHERE d.deleted_at IS NULL
          AND ($1::uuid IS NULL OR d.school_id = $1)
          AND ($2::text IS NULL OR d.placement = $2)
          AND ($3::boolean IS NULL OR d.is_active = $3)
          AND (NOT $4::boolean OR d.last_seen_at > NOW() - INTERVAL '10 minutes')
          AND ($5::text IS NULL OR d.name ILIKE $5 OR d.code ILIKE $5)
        ORDER BY s.name, d.code
        LIMIT $6 OFFSET $7
        "#,
    )
    .bind(school)
    .bind(filter.placement.as_deref())
    .bind(filter.is_active)
    .bind(online_only)
    .bind(search.as_deref())
    .bind(page.per_page())
    .bind(page.offset())
    .fetch_all(&state.db)
    .await?;

    Ok(Paginated::new(items, page.page(), page.per_page(), total))
}

/// Detail perangkat.
#[utoipa::path(
    get, path = "/v1/devices/{id}", tag = "Perangkat",
    params(("id" = Uuid, Path, description = "ID perangkat")),
    security(("bearer" = [])),
    responses((status = 200, description = "Detail perangkat", body = Device))
)]
pub async fn detail(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<Device>> {
    user.require("view_device")?;
    let d = fetch(&state, id).await?;
    user.resolve_school(Some(d.school_id))?;
    Ok(ApiResponse::new(d))
}

/// Daftarkan perangkat baru. Responsnya berisi kode pairing yang dimasukkan
/// di tablet.
#[utoipa::path(
    post, path = "/v1/devices", tag = "Perangkat",
    request_body = CreateDeviceRequest,
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Perangkat dibuat + kode pairing", body = PairingCodeResponse),
        (status = 409, description = "Kode perangkat sudah dipakai")
    )
)]
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    ValidJson(body): ValidJson<CreateDeviceRequest>,
) -> ApiResult<ApiResponse<PairingCodeResponse>> {
    user.require("create_device")?;
    let school_id = user.require_school(body.school_id)?;

    if !DEVICE_PLACEMENTS.contains(&body.placement.as_str()) {
        return Err(ApiError::field(
            "placement",
            &format!("pilih salah satu: {}", DEVICE_PLACEMENTS.join(", ")),
        ));
    }
    if !DEVICE_MODES.contains(&body.mode.as_str()) {
        return Err(ApiError::field(
            "mode",
            &format!("pilih salah satu: {}", DEVICE_MODES.join(", ")),
        ));
    }
    // Tablet di dalam kelas harus tahu kelas mana, agar daftar siswa yang
    // diunduh terbatas dan arah scan bisa mengikuti aturan kelas itu.
    if body.placement == "classroom" && body.classroom_id.is_none() {
        return Err(ApiError::field(
            "classroom_id",
            "perangkat di dalam kelas wajib menyebutkan kelasnya",
        ));
    }

    let (pairing_code, expires_at) = new_pairing_code();

    let (id,): (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO devices (
            school_id, code, name, placement, classroom_id, mode,
            pairing_code, pairing_expires_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
        RETURNING id
        "#,
    )
    .bind(school_id)
    .bind(body.code.trim().to_uppercase())
    .bind(body.name.trim())
    .bind(&body.placement)
    .bind(body.classroom_id)
    .bind(&body.mode)
    .bind(&pairing_code)
    .bind(expires_at)
    .fetch_one(&state.db)
    .await?;

    AuditEntry::by_user(&user, "device.create")
        .school(school_id)
        .entity("device", id)
        .after(&serde_json::json!({ "code": body.code, "mode": body.mode }))
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        PairingCodeResponse {
            device_id: id,
            code: body.code.trim().to_uppercase(),
            pairing_code,
            expires_at,
        },
        format!("Perangkat dibuat. Masukkan kode pairing di tablet dalam {PAIRING_TTL_MINUTES} menit."),
    ))
}

/// Perbarui perangkat.
#[utoipa::path(
    patch, path = "/v1/devices/{id}", tag = "Perangkat",
    params(("id" = Uuid, Path, description = "ID perangkat")),
    request_body = UpdateDeviceRequest,
    security(("bearer" = [])),
    responses((status = 200, description = "Perangkat diperbarui", body = Device))
)]
pub async fn update(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ValidJson(body): ValidJson<UpdateDeviceRequest>,
) -> ApiResult<ApiResponse<Device>> {
    user.require("update_device")?;
    let before = fetch(&state, id).await?;
    user.resolve_school(Some(before.school_id))?;

    if let Some(p) = &body.placement {
        if !DEVICE_PLACEMENTS.contains(&p.as_str()) {
            return Err(ApiError::field("placement", "penempatan tidak dikenal"));
        }
    }
    if let Some(m) = &body.mode {
        if !DEVICE_MODES.contains(&m.as_str()) {
            return Err(ApiError::field("mode", "mode tidak dikenal"));
        }
    }

    sqlx::query(
        r#"
        UPDATE devices SET
            name         = COALESCE($2, name),
            placement    = COALESCE($3, placement),
            classroom_id = COALESCE($4, classroom_id),
            mode         = COALESCE($5, mode),
            is_active    = COALESCE($6, is_active),
            updated_at   = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(body.name.as_deref().map(str::trim))
    .bind(body.placement.as_deref())
    .bind(body.classroom_id)
    .bind(body.mode.as_deref())
    .bind(body.is_active)
    .execute(&state.db)
    .await?;

    // Cache perangkat memuat mode & kelas, jadi harus dibuang setelah diubah.
    state.invalidate_device_cache();

    let after = fetch(&state, id).await?;
    AuditEntry::by_user(&user, "device.update")
        .school(after.school_id)
        .entity("device", id)
        .before(&before)
        .after(&after)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(after, "Perangkat diperbarui"))
}

/// Hapus perangkat (soft delete + cabut token).
#[utoipa::path(
    delete, path = "/v1/devices/{id}", tag = "Perangkat",
    params(("id" = Uuid, Path, description = "ID perangkat")),
    security(("bearer" = [])),
    responses((status = 200, description = "Perangkat dihapus"))
)]
pub async fn remove(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require("delete_device")?;
    let before = fetch(&state, id).await?;
    user.resolve_school(Some(before.school_id))?;

    sqlx::query(
        r#"
        UPDATE devices
           SET deleted_at = NOW(), is_active = FALSE,
               token_revoked_at = NOW(), pairing_code = NULL
         WHERE id = $1
        "#,
    )
    .bind(id)
    .execute(&state.db)
    .await?;

    state.invalidate_device_cache();

    AuditEntry::by_user(&user, "device.delete")
        .school(before.school_id)
        .entity("device", id)
        .before(&before)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        serde_json::json!({ "deleted": true }),
        "Perangkat dihapus dan tokennya dicabut",
    ))
}

/// Buat ulang kode pairing (mis. tablet diganti atau token hilang).
#[utoipa::path(
    post, path = "/v1/devices/{id}/pairing-code", tag = "Perangkat",
    params(("id" = Uuid, Path, description = "ID perangkat")),
    security(("bearer" = [])),
    responses((status = 200, description = "Kode pairing baru", body = PairingCodeResponse))
)]
pub async fn regenerate_pairing(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<PairingCodeResponse>> {
    user.require("pair_device")?;
    let device = fetch(&state, id).await?;
    user.resolve_school(Some(device.school_id))?;

    let (pairing_code, expires_at) = new_pairing_code();

    sqlx::query(
        r#"
        UPDATE devices
           SET pairing_code = $2, pairing_expires_at = $3, updated_at = NOW()
         WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(&pairing_code)
    .bind(expires_at)
    .execute(&state.db)
    .await?;

    AuditEntry::by_user(&user, "device.pairing_code")
        .school(device.school_id)
        .entity("device", id)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        PairingCodeResponse {
            device_id: id,
            code: device.code,
            pairing_code,
            expires_at,
        },
        format!("Kode pairing baru berlaku {PAIRING_TTL_MINUTES} menit"),
    ))
}

/// Cabut token perangkat tanpa menghapusnya.
#[utoipa::path(
    post, path = "/v1/devices/{id}/revoke", tag = "Perangkat",
    params(("id" = Uuid, Path, description = "ID perangkat")),
    security(("bearer" = [])),
    responses((status = 200, description = "Token dicabut"))
)]
pub async fn revoke(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require("update_device")?;
    let device = fetch(&state, id).await?;
    user.resolve_school(Some(device.school_id))?;

    sqlx::query("UPDATE devices SET token_revoked_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    // Pencabutan harus berlaku seketika, bukan setelah TTL cache habis.
    state.invalidate_device_cache();

    AuditEntry::by_user(&user, "device.revoke")
        .school(device.school_id)
        .entity("device", id)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        serde_json::json!({ "revoked": true }),
        "Token perangkat dicabut. Tablet harus dipasangkan ulang.",
    ))
}

/// **Dipanggil tablet.** Tukar kode pairing dengan device token permanen.
///
/// Token hanya diberikan sekali di sini dan tidak dapat dibaca ulang, sesuai
/// prinsip bahwa server tidak menyimpan kredensial dalam bentuk yang bisa
/// dibocorkan (hanya SHA-256-nya).
#[utoipa::path(
    post, path = "/v1/devices/pair", tag = "Perangkat",
    request_body = PairDeviceRequest,
    responses(
        (status = 200, description = "Berhasil dipasangkan", body = PairDeviceResponse),
        (status = 404, description = "Kode tidak ditemukan / sudah kedaluwarsa")
    )
)]
pub async fn pair(
    State(state): State<AppState>,
    ValidJson(body): ValidJson<PairDeviceRequest>,
) -> ApiResult<ApiResponse<PairDeviceResponse>> {
    // Kode pairing hanya 8 digit; tanpa rate limit ia bisa ditebak brute force.
    state
        .rate_limit("device_pair", 30, std::time::Duration::from_secs(60))
        .await?;

    let token = password::generate_token();
    let token_hash = vector::sha256(token.as_bytes());
    let hmac_secret = password::generate_secret();

    // Kode dikonsumsi dalam satu UPDATE: dua tablet yang memakai kode sama
    // secara bersamaan, hanya satu yang berhasil.
    let row: Option<(Uuid,)> = sqlx::query_as(
        r#"
        UPDATE devices SET
            token_hash         = $2,
            hmac_secret        = $3,
            token_issued_at    = NOW(),
            token_revoked_at   = NULL,
            pairing_code       = NULL,
            pairing_expires_at = NULL,
            app_version        = COALESCE($4, app_version),
            os_version         = COALESCE($5, os_version),
            last_seen_at       = NOW(),
            is_active          = TRUE,
            updated_at         = NOW()
        WHERE pairing_code = $1
          AND pairing_expires_at > NOW()
          AND deleted_at IS NULL
        RETURNING id
        "#,
    )
    .bind(body.pairing_code.trim())
    .bind(&token_hash)
    .bind(&hmac_secret)
    .bind(body.app_version.as_deref())
    .bind(body.os_version.as_deref())
    .fetch_optional(&state.db)
    .await?;

    let Some((device_id,)) = row else {
        return Err(ApiError::NotFound(
            "kode pairing tidak valid atau sudah kedaluwarsa".into(),
        ));
    };

    let device = fetch(&state, device_id).await?;
    state.invalidate_device_cache();

    AuditEntry::by_system("device.paired")
        .school(device.school_id)
        .entity("device", device_id)
        .after(&serde_json::json!({
            "hardware_id": body.hardware_id,
            "app_version": body.app_version
        }))
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        PairDeviceResponse {
            device_id,
            device_code: device.code,
            device_name: device.name,
            school_id: device.school_id,
            school_name: device.school_name,
            classroom_id: device.classroom_id,
            classroom_name: device.classroom_name,
            mode: device.mode,
            placement: device.placement,
            device_token: token,
            hmac_secret: hex_encode(&hmac_secret),
            config: runtime_config(&state),
        },
        "Perangkat berhasil dipasangkan. Simpan token ini — tidak dapat dilihat lagi.",
    ))
}

// =====================================================================

pub async fn fetch(state: &AppState, id: Uuid) -> ApiResult<Device> {
    let row: Option<Device> = sqlx::query_as(
        r#"
        SELECT d.id, d.school_id, s.name AS school_name, d.code, d.name, d.placement,
               d.classroom_id, c.name AS classroom_name, d.mode,
               (d.token_hash IS NOT NULL AND d.token_revoked_at IS NULL) AS is_paired,
               d.app_version, d.os_version, d.last_seen_at, d.last_ip,
               (d.last_seen_at > NOW() - INTERVAL '10 minutes') AS is_online,
               d.is_active, d.created_at
        FROM devices d
        JOIN schools s ON s.id = d.school_id
        LEFT JOIN classrooms c ON c.id = d.classroom_id
        WHERE d.id = $1 AND d.deleted_at IS NULL
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    row.ok_or_else(|| ApiError::NotFound(format!("perangkat `{id}`")))
}

fn new_pairing_code() -> (String, chrono::DateTime<Utc>) {
    (
        password::generate_pairing_code(),
        Utc::now() + ChronoDuration::minutes(PAIRING_TTL_MINUTES),
    )
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_encode_benar() {
        assert_eq!(hex_encode(&[0x00, 0x0f, 0xff, 0xa5]), "000fffa5");
        assert_eq!(hex_encode(&[]), "");
    }

    #[test]
    fn kode_pairing_kedaluwarsa_di_masa_depan() {
        let (code, exp) = new_pairing_code();
        assert_eq!(code.len(), 8);
        assert!(exp > Utc::now());
        assert!(exp <= Utc::now() + ChronoDuration::minutes(PAIRING_TTL_MINUTES + 1));
    }
}
