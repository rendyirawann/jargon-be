//! Logging & tracing.
//!
//! Di lokal keluaran berwarna dan mudah dibaca manusia; di produksi berupa
//! JSON satu baris per event agar bisa dikirim ke Loki/Elasticsearch tanpa
//! parser khusus.

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

pub fn init(app_env: &str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // Default waras: info untuk kode kita, warn untuk pustaka yang berisik.
        EnvFilter::new(
            "jargon_api=info,tower_http=info,axum=info,sqlx=warn,hyper=warn,lettre=warn",
        )
    });

    let is_production = matches!(app_env, "production" | "prod" | "staging");

    if is_production {
        tracing_subscriber::registry()
            .with(filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_current_span(true)
                    .with_span_list(false)
                    .with_target(true)
                    .flatten_event(true),
            )
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_target(false)
                    .with_ansi(true)
                    .compact(),
            )
            .init();
    }
}
