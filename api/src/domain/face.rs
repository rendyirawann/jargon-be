//! DTO pendaftaran wajah (satu-satunya tempat gambar wajah dikirim & disimpan).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

use crate::face::quality::QualityReport;

pub const FACE_POSES: [&str; 5] = ["frontal", "left", "right", "up", "down"];

/// Permintaan pendaftaran satu sampel wajah.
///
/// Tablet mengirim DUA hal sekaligus:
///   * `image_base64` — gambar wajah yang sudah di-crop, DISIMPAN sebagai
///     arsip agar embedding bisa dihitung ulang bila model di-upgrade.
///   * `embedding`    — vektor hasil ekstraksi di perangkat, agar versi model
///     di tablet dan di server pasti konsisten.
// `device_quality` diterima sebagai informasi, tetapi keputusan diambil dari
// analisis ulang di server — klien tidak dipercaya menilai kualitas fotonya
// sendiri.
#[allow(dead_code)]
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct EnrollFaceRequest {
    /// Gambar wajah ter-crop (JPEG/PNG) dalam base64, tanpa prefix data URI.
    #[validate(length(min = 100, message = "gambar tidak boleh kosong"))]
    pub image_base64: String,
    /// Embedding 512 dimensi hasil model di perangkat.
    #[validate(length(min = 1, message = "embedding wajib dikirim"))]
    pub embedding: Vec<f32>,
    /// Versi model yang menghasilkan embedding. Harus cocok dengan server.
    pub model_version: String,
    /// frontal / left / right / up / down.
    #[serde(default = "default_pose")]
    pub pose: String,
    /// Skor kualitas dari perangkat (0..1). Server tetap menghitung ulang.
    pub device_quality: Option<f32>,
}

fn default_pose() -> String {
    "frontal".to_string()
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EnrollFaceResponse {
    pub enrollment_id: Uuid,
    pub embedding_id: Uuid,
    pub student_id: Uuid,
    /// Total sampel aktif setelah penambahan ini.
    pub sample_count: i16,
    /// Apakah siswa sudah dianggap siap untuk absen dengan wajah.
    pub ready: bool,
    /// Hasil analisis kualitas di sisi server.
    pub quality: QualityReport,
    /// Kemiripan terhadap sampel siswa ini yang sudah ada. Nilai sangat
    /// rendah menandakan kemungkinan foto orang yang berbeda.
    pub self_similarity: Option<f32>,
    pub message: String,
}

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct FaceEnrollmentItem {
    pub id: Uuid,
    pub student_id: Uuid,
    pub student_name: String,
    pub classroom_name: Option<String>,
    pub pose: String,
    pub quality_score: Option<f32>,
    pub status: String,
    pub reject_reason: Option<String>,
    /// URL relatif untuk melihat gambar. Hanya bisa diakses oleh pengguna
    /// yang berwenang atas sekolah siswa tersebut.
    pub image_url: String,
    pub created_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct FaceEnrollmentFilter {
    pub school_id: Option<Uuid>,
    pub classroom_id: Option<Uuid>,
    pub student_id: Option<Uuid>,
    /// pending / approved / rejected / replaced.
    pub status: Option<String>,
}

// Verifikasi manual oleh kepala sekolah: skema sudah final dan tercantum di
// OpenAPI, handler-nya menyusul bersama alur persetujuan berjenjang.
#[allow(dead_code)]
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ReviewEnrollmentRequest {
    /// `approved` atau `rejected`.
    pub status: String,
    #[validate(length(max = 200))]
    pub reject_reason: Option<String>,
}

/// Ringkasan kesiapan biometrik — dipakai kartu dashboard.
#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct FaceCoverage {
    pub total_students: i64,
    pub enrolled: i64,
    pub not_enrolled: i64,
    /// Siswa dengan sampel < 3, yang akurasinya masih rentan.
    pub under_sampled: i64,
    pub pending_review: i64,
}

impl FaceCoverage {
    pub fn percentage(&self) -> f64 {
        if self.total_students == 0 {
            0.0
        } else {
            (self.enrolled as f64 / self.total_students as f64) * 100.0
        }
    }
}

/// Jumlah sampel minimum agar pengenalan cukup andal di berbagai
/// pencahayaan/sudut. Di bawah ini siswa masih bisa absen, tetapi
/// dashboard menandainya agar dilengkapi.
pub const RECOMMENDED_SAMPLES: i16 = 3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persentase_cakupan_aman_saat_nol() {
        let c = FaceCoverage {
            total_students: 0,
            enrolled: 0,
            not_enrolled: 0,
            under_sampled: 0,
            pending_review: 0,
        };
        assert_eq!(c.percentage(), 0.0);
    }

    #[test]
    fn persentase_cakupan_dihitung() {
        let c = FaceCoverage {
            total_students: 400,
            enrolled: 300,
            not_enrolled: 100,
            under_sampled: 50,
            pending_review: 2,
        };
        assert!((c.percentage() - 75.0).abs() < 1e-9);
    }

    #[test]
    fn enroll_menolak_gambar_kosong() {
        let req = EnrollFaceRequest {
            image_base64: "abc".into(),
            embedding: vec![0.1, 0.2],
            model_version: "mobilefacenet-v1".into(),
            pose: "frontal".into(),
            device_quality: None,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn enroll_menolak_embedding_kosong() {
        let req = EnrollFaceRequest {
            image_base64: "x".repeat(200),
            embedding: vec![],
            model_version: "mobilefacenet-v1".into(),
            pose: "frontal".into(),
            device_quality: None,
        };
        assert!(req.validate().is_err());
    }
}
