//! Modul biometrik wajah.
//!
//! Pembagian tanggung jawab antara tablet dan server:
//!
//! ```text
//!   TABLET (Flutter)                        SERVER (Rust)
//!   ─────────────────                       ──────────────
//!   deteksi wajah (ML Kit)
//!   cek liveness (blink/gerak)
//!   crop + align 112x112
//!   ekstraksi embedding (TFLite)
//!   L2-normalize
//!        │
//!        ├── PENDAFTARAN ──> kirim GAMBAR + embedding ──> simpan keduanya
//!        │                                                (gambar di storage,
//!        │                                                 vektor di pgvector)
//!        │
//!        └── ABSEN HARIAN ─> kirim HANYA embedding ─────> cocokkan, catat
//!                                                        absensi, BUANG
//!                                                        embedding-nya
//! ```
//!
//! Alasan ekstraksi dilakukan di tablet: satu request absen hanya membawa
//! 2 KB (512 x f32) bukan ~200 KB gambar. Untuk ribuan sekolah yang banyak
//! di antaranya berjaringan lambat, ini perbedaan antara "instan" dan
//! "tidak bisa dipakai". Konsekuensinya versi model di tablet harus sama
//! dengan yang tercatat pada embedding tersimpan — dijaga oleh
//! `model_version` dan divalidasi pada setiap request.

pub mod index;
pub mod quality;
pub mod vector;

pub use index::{FaceIndex, SchoolSlice};
