//! Logika bisnis. Handler HTTP di `routes/` sengaja dibuat tipis dan hanya
//! mengurus parsing/otorisasi, sedangkan aturan domain tinggal di sini agar
//! bisa diuji tanpa server dan dipakai ulang oleh worker.

pub mod anonymity;
pub mod audit;
pub mod enrollment;
pub mod notify;
pub mod recognition;
pub mod reports;
pub mod rules;
pub mod storage;
