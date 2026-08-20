//! Absensi Face Recognition API — Dinas Pendidikan Provinsi Sumatera Utara.
//!
//! Satu binary yang melayani:
//!   * REST API untuk tablet kios & aplikasi mobile,
//!   * REST API untuk dashboard `/admin` (Laravel Octane),
//!   * Swagger UI di `/docs`,
//!   * worker latar (pengiriman notifikasi & pemeliharaan harian).
//!
//! Jalankan `cargo run` setelah menyiapkan `.env` (lihat `.env.example`).
//!
//! Subcommand:
//!   * `jargon-api`           jalankan server (migrasi ikut diterapkan)
//!   * `jargon-api migrate`   HANYA terapkan migrasi, lalu keluar
//!   * `jargon-api version`   cetak versi
//!
//! `migrate` ada karena tidak setiap keadaan boleh menyalakan server untuk
//! membuat tabel: pemasangan awal di server, pipeline CI/CD yang memisahkan
//! langkah migrasi dari langkah rilis, dan pemeriksaan apakah skema sudah
//! mutakhir. Migrasinya sama persis dengan yang dijalankan saat start —
//! disematkan ke biner oleh `sqlx::migrate!`, jadi tidak mungkin berbeda.

mod auth;
mod config;
mod domain;
mod error;
mod extract;
mod face;
mod openapi;
mod routes;
mod services;
mod state;
mod telemetry;
mod util;
mod workers;

use std::time::Duration;

use axum::http::{header, HeaderName, HeaderValue, Method};
use axum::Router;
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::Config;
use crate::openapi::ApiDoc;
use crate::state::AppState;

const REQUEST_ID_HEADER: &str = "x-request-id";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // .env bersifat opsional: di produksi variabel datang dari orchestrator.
    let _ = dotenvy::dotenv();

    // Subcommand dibaca SEBELUM konfigurasi penuh: `version` tidak butuh
    // database, dan operator yang salah mengetik subcommand sebaiknya tahu
    // segera, bukan setelah menunggu koneksi database gagal.
    let command = std::env::args().nth(1).unwrap_or_default();

    if command == "version" || command == "--version" || command == "-V" {
        println!("jargon-api {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if !command.is_empty() && command != "migrate" && command != "serve" {
        eprintln!("Subcommand `{command}` tidak dikenal.");
        eprintln!("Pilihan: serve (bawaan), migrate, version");
        std::process::exit(2);
    }

    let cfg = Config::from_env().map_err(|e| anyhow::anyhow!("{e}"))?;
    telemetry::init(&cfg.app_env);

    tracing::info!(
        app = %cfg.app_name,
        env = %cfg.app_env,
        version = env!("CARGO_PKG_VERSION"),
        "menyalakan layanan"
    );

    let bind_addr = cfg.bind_addr.clone();
    let enable_swagger = cfg.enable_swagger;
    let workers_enabled = cfg.workers_enabled;
    let max_upload = cfg.max_upload_bytes;
    let cors_origins = cfg.cors_allowed_origins.clone();
    let is_production = cfg.is_production();

    let state = AppState::bootstrap(cfg)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    // Migrasi dijalankan saat start. Aman untuk beberapa replika: sqlx memakai
    // advisory lock sehingga hanya satu yang menerapkan, sisanya menunggu.
    match sqlx::migrate!("./migrations").run(&state.db).await {
        Ok(()) => tracing::info!("migrasi database mutakhir"),
        Err(e) => {
            tracing::error!(error = %e, "migrasi database gagal");
            return Err(anyhow::anyhow!("migrasi gagal: {e}"));
        }
    }

    // `migrate`: selesai. Kolam koneksi ditutup rapi supaya advisory lock
    // sqlx dilepas segera — bukan menunggu batas waktu — agar langkah
    // berikutnya di pipeline tidak tertahan menunggu lock yang sudah tidak
    // dipakai siapa pun.
    if command == "migrate" {
        state.db.close().await;
        tracing::info!("migrasi selesai, keluar");
        return Ok(());
    }

    warn_on_risky_config(&state, is_production);

    // -----------------------------------------------------------------
    // Router
    // -----------------------------------------------------------------
    let mut app = Router::new()
        .merge(routes::root())
        .nest("/v1", routes::v1());

    if enable_swagger {
        app = app.merge(
            SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()),
        );
        tracing::info!("Swagger UI aktif di /docs");
    }

    let cors = build_cors(&cors_origins);

    let app = app
        .layer(
            ServiceBuilder::new()
                // Request id dipasang paling luar agar semua log & respons
                // error membawa id yang sama untuk satu permintaan.
                .layer(SetRequestIdLayer::new(
                    HeaderName::from_static(REQUEST_ID_HEADER),
                    MakeRequestUuid,
                ))
                .layer(PropagateRequestIdLayer::new(HeaderName::from_static(
                    REQUEST_ID_HEADER,
                )))
                .layer(TraceLayer::new_for_http())
                // Batas waktu per request: mencegah koneksi menggantung saat
                // provider notifikasi atau database melambat. 504 dipilih
                // eksplisit supaya klien bisa membedakannya dari error aplikasi.
                .layer(TimeoutLayer::with_status_code(
                    axum::http::StatusCode::GATEWAY_TIMEOUT,
                    Duration::from_secs(30),
                ))
                .layer(RequestBodyLimitLayer::new(max_upload + 1024 * 64))
                .layer(CompressionLayer::new())
                .layer(cors)
                .layer(SetResponseHeaderLayer::if_not_present(
                    header::X_CONTENT_TYPE_OPTIONS,
                    HeaderValue::from_static("nosniff"),
                ))
                .layer(SetResponseHeaderLayer::if_not_present(
                    header::X_FRAME_OPTIONS,
                    HeaderValue::from_static("DENY"),
                ))
                .layer(SetResponseHeaderLayer::if_not_present(
                    header::REFERRER_POLICY,
                    HeaderValue::from_static("no-referrer"),
                )),
        )
        .with_state(state.clone());

    // -----------------------------------------------------------------
    // Worker + shutdown
    // -----------------------------------------------------------------
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let worker_handles = if workers_enabled {
        workers::spawn_all(state.clone(), shutdown_rx)
    } else {
        tracing::info!("worker latar dimatikan (WORKERS_ENABLED=false)");
        Vec::new()
    };

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .map_err(|e| anyhow::anyhow!("gagal bind ke {bind_addr}: {e}"))?;

    tracing::info!(addr = %bind_addr, "siap menerima permintaan");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Beri worker kesempatan menyelesaikan pekerjaan yang sedang berjalan,
    // supaya tidak ada notifikasi yang tertinggal di status 'sending'.
    tracing::info!("menghentikan worker...");
    let _ = shutdown_tx.send(true);
    for handle in worker_handles {
        let _ = tokio::time::timeout(Duration::from_secs(10), handle).await;
    }

    state.db.close().await;
    tracing::info!("layanan berhenti dengan rapi");
    Ok(())
}

/// CORS: `*` untuk pengembangan, daftar origin eksplisit untuk produksi.
fn build_cors(origins: &[String]) -> CorsLayer {
    let base = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
            HeaderName::from_static("x-api-key"),
            HeaderName::from_static("x-api-secret"),
            HeaderName::from_static("idempotency-key"),
        ])
        .max_age(Duration::from_secs(3600));

    if origins.iter().any(|o| o == "*") {
        // `allow_credentials` sengaja tidak dinyalakan bersama wildcard —
        // kombinasi itu ditolak browser dan juga oleh tower-http.
        base.allow_origin(AllowOrigin::any())
    } else {
        let parsed: Vec<HeaderValue> = origins
            .iter()
            .filter_map(|o| HeaderValue::from_str(o).ok())
            .collect();
        base.allow_origin(parsed).allow_credentials(true)
    }
}

/// Peringatkan konfigurasi berbahaya alih-alih gagal diam-diam.
fn warn_on_risky_config(state: &AppState, is_production: bool) {
    if !is_production {
        return;
    }
    if state.cfg.cors_allowed_origins.iter().any(|o| o == "*") {
        tracing::warn!(
            "CORS_ALLOWED_ORIGINS masih `*` di produksi — batasi ke domain dashboard"
        );
    }
    if state.cfg.enable_swagger {
        tracing::warn!(
            "Swagger UI terbuka di produksi — set ENABLE_SWAGGER=false bila tidak diperlukan"
        );
    }
    if state.redis.is_none() {
        tracing::warn!(
            "Redis tidak aktif: rate limit dan proteksi replay nonce tidak berjalan"
        );
    }
    if state.cfg.secrets_key.is_none() {
        tracing::warn!(
            "SECRETS_KEY_HEX belum diisi: kredensial provider notifikasi tersimpan tanpa enkripsi"
        );
    }
    if !state.cfg.notify.enabled {
        tracing::warn!("NOTIFY_ENABLED=false: orang tua tidak akan menerima notifikasi");
    }
}

/// Tunggu Ctrl+C atau SIGTERM (dikirim Docker/Kubernetes saat rolling update).
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("gagal memasang handler Ctrl+C");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("gagal memasang handler SIGTERM")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("Ctrl+C diterima"),
        _ = terminate => tracing::info!("SIGTERM diterima"),
    }
}
