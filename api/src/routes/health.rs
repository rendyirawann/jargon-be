//! Endpoint kesehatan untuk load balancer, Kubernetes, dan monitoring.

use axum::extract::State;
use axum::routing::get;
use axum::Router;
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::ApiResult;
use crate::state::AppState;
use crate::util::ApiResponse;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct HealthStatus {
    pub status: &'static str,
    pub app: String,
    pub version: &'static str,
    pub environment: String,
    pub uptime_seconds: u64,
    pub database: ComponentStatus,
    pub redis: ComponentStatus,
    pub face_index: FaceIndexStatus,
    pub server_time: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ComponentStatus {
    pub available: bool,
    pub detail: String,
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FaceIndexStatus {
    pub cached_schools: usize,
    pub cached_samples: usize,
    pub embedding_dim: usize,
    pub model_version: String,
}

/// Liveness: proses hidup. Sengaja tidak menyentuh dependensi apa pun —
/// database yang lambat tidak boleh membuat orchestrator me-restart pod.
#[utoipa::path(
    get, path = "/health/live", tag = "Sistem",
    responses((status = 200, description = "Proses hidup"))
)]
pub async fn live() -> &'static str {
    "ok"
}

/// Readiness: siap menerima trafik. Ini memeriksa database, karena tanpa
/// database endpoint absensi pasti gagal.
#[utoipa::path(
    get, path = "/health/ready", tag = "Sistem",
    responses(
        (status = 200, description = "Siap menerima trafik"),
        (status = 503, description = "Belum siap")
    )
)]
pub async fn ready(State(state): State<AppState>) -> Result<&'static str, axum::http::StatusCode> {
    match sqlx::query("SELECT 1").execute(&state.db).await {
        Ok(_) => Ok("ready"),
        Err(e) => {
            tracing::error!(error = %e, "readiness gagal: database tidak terjangkau");
            Err(axum::http::StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

/// Status rinci untuk dashboard monitoring.
#[utoipa::path(
    get, path = "/health", tag = "Sistem",
    responses((status = 200, description = "Status komponen", body = HealthStatus))
)]
pub async fn health(State(state): State<AppState>) -> ApiResult<ApiResponse<HealthStatus>> {
    let db_start = std::time::Instant::now();
    let database = match sqlx::query("SELECT 1").execute(&state.db).await {
        Ok(_) => ComponentStatus {
            available: true,
            detail: format!(
                "terhubung ({} koneksi idle dari {})",
                state.db.num_idle(),
                state.db.size()
            ),
            latency_ms: Some(db_start.elapsed().as_millis() as u64),
        },
        Err(e) => ComponentStatus {
            available: false,
            detail: e.to_string(),
            latency_ms: None,
        },
    };

    let redis = match state.redis.clone() {
        None => ComponentStatus {
            available: false,
            detail: "tidak dikonfigurasi (opsional)".into(),
            latency_ms: None,
        },
        Some(mut conn) => {
            let start = std::time::Instant::now();
            match redis::cmd("PING").query_async::<String>(&mut conn).await {
                Ok(_) => ComponentStatus {
                    available: true,
                    detail: "terhubung".into(),
                    latency_ms: Some(start.elapsed().as_millis() as u64),
                },
                Err(e) => ComponentStatus {
                    available: false,
                    detail: e.to_string(),
                    latency_ms: None,
                },
            }
        }
    };

    Ok(ApiResponse::new(HealthStatus {
        status: if database.available { "ok" } else { "degraded" },
        app: state.cfg.app_name.clone(),
        version: env!("CARGO_PKG_VERSION"),
        environment: state.cfg.app_env.clone(),
        uptime_seconds: state.uptime().as_secs(),
        database,
        redis,
        face_index: FaceIndexStatus {
            cached_schools: state.face_index.cached_schools(),
            cached_samples: state.face_index.cached_samples(),
            embedding_dim: state.cfg.face.embedding_dim,
            model_version: state.cfg.face.model_version.clone(),
        },
        server_time: chrono::Utc::now(),
    }))
}
