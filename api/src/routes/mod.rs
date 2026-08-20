//! Perakitan router.
//!
//! Struktur URL:
//! ```text
//!   /health, /health/live, /health/ready   -> tanpa autentikasi
//!   /docs                                  -> Swagger UI
//!   /api-docs/openapi.json                 -> spesifikasi OpenAPI 3.1
//!   /files/*                               -> berkas, wajib Bearer
//!   /v1/auth/*                             -> login & token (NIK/NISN)
//!   /v1/devices/pair                       -> tanpa Bearer (kode pairing)
//!   /v1/kiosk/*                            -> Authorization: Device <token>
//!   /v1/*                                  -> Authorization: Bearer <jwt>
//!
//!   Jargon GO (aplikasi mobile):
//!   /v1/me/*                               -> data milik pengguna sendiri
//!   /v1/panic/*                            -> Panic Button (pengaduan anonim)
//!   /v1/documents/*                        -> Pemberkasan kepegawaian
//! ```

pub mod attendance;
pub mod auth;
pub mod classrooms;
pub mod dashboard;
pub mod devices;
pub mod documents;
pub mod files;
pub mod health;
pub mod kiosk;
pub mod me;
pub mod notifications;
pub mod panic;
pub mod schools;
pub mod students;
pub mod users;

use axum::Router;

use crate::state::AppState;

/// Seluruh endpoint versi 1.
pub fn v1() -> Router<AppState> {
    Router::new()
        .merge(auth::router())
        .merge(schools::router())
        .merge(classrooms::router())
        .merge(students::router())
        .merge(attendance::router())
        .merge(devices::router())
        .merge(devices::public_router())
        .merge(kiosk::router())
        .merge(notifications::router())
        .merge(dashboard::router())
        .merge(users::router())
        // --- Jargon GO (Super Apps) ---
        .merge(me::router())
        .merge(panic::router())
        .merge(documents::router())
}

/// Endpoint di luar prefix `/v1`.
pub fn root() -> Router<AppState> {
    Router::new().merge(health::router()).merge(files::router())
}
