//! Proses latar yang berjalan di dalam binary API.
//!
//! Sengaja tidak memakai antrean/scheduler eksternal: satu binary yang
//! di-deploy jauh lebih mudah dioperasikan oleh tim yang mengelola ribuan
//! sekolah. Semua worker aman dijalankan pada beberapa replika sekaligus
//! (klaim pekerjaan memakai `FOR UPDATE SKIP LOCKED` dan operasi bersifat
//! idempotent), sehingga penskalaan horizontal tidak butuh koordinasi.
//!
//! Setel `WORKERS_ENABLED=false` bila ingin memisahkan replika "web" dan
//! "worker".

pub mod maintenance;
pub mod outbox;

use crate::state::AppState;

/// Jalankan semua worker. Mengembalikan handle untuk shutdown yang rapi.
pub fn spawn_all(
    state: AppState,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Vec<tokio::task::JoinHandle<()>> {
    vec![
        tokio::spawn(outbox::run(state.clone(), shutdown.clone())),
        tokio::spawn(maintenance::run(state, shutdown)),
    ]
}
