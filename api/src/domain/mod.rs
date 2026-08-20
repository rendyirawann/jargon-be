//! Data Transfer Object untuk seluruh endpoint.
//!
//! DTO dipisahkan dari baris database: request punya aturan validasi
//! sendiri, dan respons sengaja tidak pernah membocorkan kolom sensitif
//! (hash kata sandi, token perangkat, atau vektor embedding).

pub mod attendance;
pub mod device;
pub mod document;
pub mod face;
pub mod notification;
pub mod panic;
pub mod school;
pub mod student;
pub mod user;
