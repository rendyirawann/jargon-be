//! Endpoint yang dipanggil tablet kios.
//!
//! Seluruh endpoint di sini diautentikasi dengan **device token**, bukan akun
//! pengguna — tablet dipasang di gerbang/kelas dan tidak boleh menyimpan
//! kredensial seorang guru.

use axum::extract::State;
use axum::routing::{get, post};
use axum::Router;
use chrono::Utc;

use crate::auth::AuthDevice;
use crate::domain::attendance::{RecognizeRequest, RecognizeResponse};
use crate::domain::device::{
    DeviceHeartbeatRequest, DeviceHeartbeatResponse, DeviceRuntimeConfig, RosterEntry, TodayWindows,
};
use crate::error::ApiResult;
use crate::extract::ValidJson;
use crate::services::recognition;
use crate::services::rules;
use crate::state::AppState;
use crate::util::{self, ApiResponse};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/kiosk/recognize", post(recognize))
        .route("/kiosk/heartbeat", post(heartbeat))
        .route("/kiosk/roster", get(roster))
        .route("/kiosk/config", get(config))
}

/// **Endpoint absensi.** Tablet mengirim embedding wajah, server membalas
/// identitas siswa dan status absensinya.
///
/// Yang TIDAK terjadi di sini: gambar tidak dikirim, embedding tidak disimpan.
/// Yang tersimpan hanyalah baris absensi (id & nama siswa, kelas, sekolah,
/// jam masuk/pulang) dan satu baris log berisi hash embedding untuk
/// mendeteksi pengiriman ulang.
#[utoipa::path(
    post, path = "/v1/kiosk/recognize", tag = "Kios",
    request_body = RecognizeRequest,
    security(("device" = [])),
    responses(
        (status = 200, description = "Hasil pengenalan (periksa field `action`)", body = RecognizeResponse),
        (status = 401, description = "Token perangkat tidak valid"),
        (status = 422, description = "Payload tidak valid"),
        (status = 429, description = "Terlalu banyak permintaan")
    )
)]
pub async fn recognize(
    State(state): State<AppState>,
    device: AuthDevice,
    ValidJson(body): ValidJson<RecognizeRequest>,
) -> ApiResult<ApiResponse<RecognizeResponse>> {
    // Satu tablet realistis memindai <2 wajah/detik. Batas 120/menit memberi
    // ruang besar untuk antrean pagi sekaligus menghentikan perangkat yang
    // rusak/terinfeksi agar tidak membanjiri server.
    state
        .rate_limit(
            &format!("kiosk:{}", device.id),
            120,
            std::time::Duration::from_secs(60),
        )
        .await?;

    let result = recognition::recognize(&state, &device, body).await?;
    let message = result.message.clone();
    Ok(ApiResponse::with_message(result, message))
}

/// Denyut nadi perangkat. Balasannya juga menjadi kanal perintah ringan
/// dari server ke tablet (mis. minta muat ulang daftar siswa).
#[utoipa::path(
    post, path = "/v1/kiosk/heartbeat", tag = "Kios",
    request_body = DeviceHeartbeatRequest,
    security(("device" = [])),
    responses((status = 200, description = "Status & konfigurasi terkini", body = DeviceHeartbeatResponse))
)]
pub async fn heartbeat(
    State(state): State<AppState>,
    device: AuthDevice,
    ValidJson(body): ValidJson<DeviceHeartbeatRequest>,
) -> ApiResult<ApiResponse<DeviceHeartbeatResponse>> {
    sqlx::query(
        r#"
        INSERT INTO device_heartbeats
            (device_id, school_id, battery_pct, queued_events, app_version, network,
             embedding_model_version)
        VALUES ($1,$2,$3,$4,$5,$6,$7)
        "#,
    )
    .bind(device.id)
    .bind(device.school_id)
    .bind(body.battery_pct)
    .bind(body.queued_events.unwrap_or(0))
    .bind(body.app_version.as_deref())
    .bind(body.network.as_deref())
    .bind(body.embedding_model_version.as_deref())
    .execute(&state.db)
    .await?;

    sqlx::query(
        r#"
        UPDATE devices
           SET last_seen_at = NOW(),
               app_version  = COALESCE($2, app_version)
         WHERE id = $1
        "#,
    )
    .bind(device.id)
    .bind(body.app_version.as_deref())
    .execute(&state.db)
    .await?;

    // Versi roster = jumlah + waktu perubahan terakhir data wajah sekolah.
    // Tablet membandingkannya dengan nilai yang ia pegang untuk tahu apakah
    // perlu mengunduh daftar siswa lagi.
    let (roster_version,): (i64,) = sqlx::query_as(
        r#"
        SELECT COALESCE(
                 EXTRACT(EPOCH FROM MAX(created_at))::bigint + COUNT(*)::bigint,
                 0
               )
        FROM face_embeddings WHERE school_id = $1 AND is_active
        "#,
    )
    .bind(device.school_id)
    .fetch_one(&state.db)
    .await?;

    let mut commands = Vec::new();
    if let Some(v) = &body.embedding_model_version {
        if v != &state.cfg.face.model_version {
            commands.push("update_app".to_string());
        }
    }

    let today = util::today_wib();
    let rule = rules::resolve_rule(&state.db, device.school_id, device.classroom_id, today).await?;
    let holiday = rules::holiday_name(&state.db, device.school_id, today).await?;

    let today_windows = TodayWindows {
        is_active_day: rule.is_active_day(today) && holiday.is_none(),
        is_holiday: holiday.is_some(),
        holiday_name: holiday,
        check_in_opens_at: Some(rule.check_in_opens_at.format("%H:%M").to_string()),
        check_in_due_at: Some(rule.check_in_due_at.format("%H:%M").to_string()),
        check_in_closes_at: Some(rule.check_in_closes_at.format("%H:%M").to_string()),
        check_out_opens_at: Some(rule.check_out_opens_at.format("%H:%M").to_string()),
        check_out_closes_at: Some(rule.check_out_closes_at.format("%H:%M").to_string()),
    };

    Ok(ApiResponse::new(DeviceHeartbeatResponse {
        server_time: Utc::now(),
        config: runtime_config(&state),
        roster_version,
        commands,
        today_windows: Some(today_windows),
    }))
}

/// Daftar siswa sekolah ini untuk cache tampilan offline di tablet.
///
/// Tidak memuat embedding maupun gambar: tablet tidak melakukan pencocokan
/// sendiri, hanya menampilkan nama setelah server mengonfirmasi.
#[utoipa::path(
    get, path = "/v1/kiosk/roster", tag = "Kios",
    security(("device" = [])),
    responses((status = 200, description = "Daftar siswa ringkas", body = [RosterEntry]))
)]
pub async fn roster(
    State(state): State<AppState>,
    device: AuthDevice,
) -> ApiResult<ApiResponse<Vec<RosterEntry>>> {
    let rows: Vec<RosterEntry> = sqlx::query_as(
        r#"
        SELECT s.id AS student_id, s.full_name, s.nis,
               s.current_classroom_id AS classroom_id, c.name AS classroom_name,
               s.face_enrolled
        FROM students s
        LEFT JOIN classrooms c ON c.id = s.current_classroom_id
        WHERE s.school_id = $1
          AND s.deleted_at IS NULL
          AND s.status = 'aktif'
          -- Tablet di dalam kelas hanya menerima daftar kelasnya sendiri.
          AND ($2::uuid IS NULL OR s.current_classroom_id = $2)
        ORDER BY c.name NULLS LAST, s.full_name
        "#,
    )
    .bind(device.school_id)
    .bind(device.classroom_id)
    .fetch_all(&state.db)
    .await?;

    Ok(ApiResponse::new(rows))
}

/// Konfigurasi runtime (ambang, versi model, dimensi embedding).
#[utoipa::path(
    get, path = "/v1/kiosk/config", tag = "Kios",
    security(("device" = [])),
    responses((status = 200, description = "Konfigurasi perangkat", body = DeviceRuntimeConfig))
)]
pub async fn config(
    State(state): State<AppState>,
    _device: AuthDevice,
) -> ApiResult<ApiResponse<DeviceRuntimeConfig>> {
    Ok(ApiResponse::new(runtime_config(&state)))
}

pub fn runtime_config(state: &AppState) -> DeviceRuntimeConfig {
    DeviceRuntimeConfig {
        embedding_dim: state.cfg.face.embedding_dim,
        model_version: state.cfg.face.model_version.clone(),
        match_threshold: state.cfg.face.match_threshold,
        min_liveness: state.cfg.face.min_liveness,
        scan_cooldown_seconds: state.cfg.face.scan_cooldown.as_secs(),
        heartbeat_interval_seconds: 120,
    }
}
