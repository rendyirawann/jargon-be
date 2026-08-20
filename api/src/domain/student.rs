//! DTO siswa & wali murid.
//!
//! Siswa tidak memiliki akun. Tidak ada field kredensial di sini —
//! identifikasi siswa hanya melalui NISN/NIS (administratif) dan wajah
//! (operasional).

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

pub const STUDENT_STATUS: [&str; 5] = ["aktif", "lulus", "pindah", "keluar", "cuti"];
pub const GUARDIAN_RELATIONS: [&str; 3] = ["ayah", "ibu", "wali"];
pub const NOTIFY_CHANNELS: [&str; 4] = ["whatsapp", "telegram", "email", "none"];

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct Student {
    pub id: Uuid,
    pub school_id: Uuid,
    pub school_name: String,
    pub current_classroom_id: Option<Uuid>,
    pub classroom_name: Option<String>,
    pub grade_level: Option<i16>,
    pub nisn: Option<String>,
    pub nis: Option<String>,
    pub full_name: String,
    /// L atau P.
    pub gender: Option<String>,
    pub birth_place: Option<String>,
    pub birth_date: Option<NaiveDate>,
    pub religion: Option<String>,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub photo_path: Option<String>,
    pub father_name: Option<String>,
    pub mother_name: Option<String>,
    pub status: String,
    pub entry_year: Option<i16>,
    /// Apakah wajah siswa sudah didaftarkan.
    pub face_enrolled: bool,
    pub face_enrolled_at: Option<DateTime<Utc>>,
    /// Jumlah sampel wajah aktif (idealnya 3-5 dari sudut berbeda).
    pub face_sample_count: i16,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct StudentListItem {
    pub id: Uuid,
    pub school_id: Uuid,
    pub nisn: Option<String>,
    pub nis: Option<String>,
    pub full_name: String,
    pub gender: Option<String>,
    pub classroom_name: Option<String>,
    pub status: String,
    pub face_enrolled: bool,
    pub face_sample_count: i16,
    /// Status absensi hari ini, bila diminta lewat `with_today=true`.
    pub today_status: Option<String>,
    pub today_check_in: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateStudentRequest {
    /// Wajib bagi Superadmin/Dinas. Untuk pengguna sekolah otomatis diisi.
    pub school_id: Option<Uuid>,
    pub current_classroom_id: Option<Uuid>,
    #[validate(length(min = 10, max = 10, message = "NISN harus 10 digit"))]
    pub nisn: Option<String>,
    #[validate(length(min = 1, max = 20))]
    pub nis: Option<String>,
    #[validate(length(min = 2, max = 150, message = "nama lengkap minimal 2 karakter"))]
    pub full_name: String,
    /// L atau P.
    pub gender: Option<String>,
    pub birth_place: Option<String>,
    pub birth_date: Option<NaiveDate>,
    pub religion: Option<String>,
    pub address: Option<String>,
    #[validate(length(max = 20))]
    pub phone: Option<String>,
    pub father_name: Option<String>,
    pub mother_name: Option<String>,
    #[validate(range(min = 1990, max = 2100))]
    pub entry_year: Option<i16>,
    /// Wali murid yang dibuat sekaligus. Minimal satu sangat dianjurkan,
    /// karena tanpa ini notifikasi absensi tidak punya tujuan.
    #[serde(default)]
    #[validate(nested)]
    pub guardians: Vec<CreateGuardianRequest>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateStudentRequest {
    pub current_classroom_id: Option<Uuid>,
    #[validate(length(min = 10, max = 10))]
    pub nisn: Option<String>,
    #[validate(length(min = 1, max = 20))]
    pub nis: Option<String>,
    #[validate(length(min = 2, max = 150))]
    pub full_name: Option<String>,
    pub gender: Option<String>,
    pub birth_place: Option<String>,
    pub birth_date: Option<NaiveDate>,
    pub religion: Option<String>,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub father_name: Option<String>,
    pub mother_name: Option<String>,
    /// aktif / lulus / pindah / keluar / cuti.
    pub status: Option<String>,
    #[validate(range(min = 1990, max = 2100))]
    pub entry_year: Option<i16>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct StudentFilter {
    pub school_id: Option<Uuid>,
    pub classroom_id: Option<Uuid>,
    pub grade_level: Option<i16>,
    /// Default `aktif`. Kirim `all` untuk semua status.
    pub status: Option<String>,
    /// `true` = hanya yang sudah punya data wajah, `false` = yang belum.
    pub face_enrolled: Option<bool>,
    /// Sertakan status absensi hari ini pada setiap baris.
    pub with_today: Option<bool>,
}

// =====================================================================
// Wali murid
// =====================================================================

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct Guardian {
    pub id: Uuid,
    pub student_id: Uuid,
    pub relation: String,
    pub full_name: String,
    pub phone: Option<String>,
    pub whatsapp: Option<String>,
    pub email: Option<String>,
    pub telegram_chat_id: Option<String>,
    pub preferred_channel: String,
    pub is_primary: bool,
    pub notify_enabled: bool,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateGuardianRequest {
    /// ayah / ibu / wali.
    #[serde(default = "default_relation")]
    pub relation: String,
    #[validate(length(min = 2, max = 150, message = "nama wali minimal 2 karakter"))]
    pub full_name: String,
    #[validate(length(max = 20))]
    pub phone: Option<String>,
    /// Nomor WhatsApp. Format apa pun diterima lalu dinormalisasi ke 62xxx.
    #[validate(length(max = 20))]
    pub whatsapp: Option<String>,
    #[validate(email(message = "format email wali tidak valid"))]
    pub email: Option<String>,
    pub telegram_chat_id: Option<String>,
    #[serde(default = "default_channel")]
    pub preferred_channel: String,
    #[serde(default)]
    pub is_primary: bool,
    #[serde(default = "default_true")]
    pub notify_enabled: bool,
}

fn default_relation() -> String {
    "wali".to_string()
}
fn default_channel() -> String {
    "whatsapp".to_string()
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateGuardianRequest {
    pub relation: Option<String>,
    #[validate(length(min = 2, max = 150))]
    pub full_name: Option<String>,
    pub phone: Option<String>,
    pub whatsapp: Option<String>,
    #[validate(email)]
    pub email: Option<String>,
    pub telegram_chat_id: Option<String>,
    pub preferred_channel: Option<String>,
    pub is_primary: Option<bool>,
    pub notify_enabled: Option<bool>,
}

/// Normalisasi nomor telepon Indonesia ke format E.164 tanpa `+` (62xxx).
///
/// Operator sekolah memasukkan nomor dalam berbagai bentuk: `08123456789`,
/// `+62 812-3456-789`, `62812 3456 789`. Provider WhatsApp menolak semuanya
/// kecuali satu bentuk, jadi normalisasi dilakukan sekali di sini.
pub fn normalize_phone(input: &str) -> Option<String> {
    let digits: String = input.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let normalized = if let Some(rest) = digits.strip_prefix("62") {
        format!("62{rest}")
    } else if let Some(rest) = digits.strip_prefix('0') {
        format!("62{rest}")
    } else if digits.starts_with('8') {
        format!("62{digits}")
    } else {
        digits
    };
    // Nomor seluler Indonesia: 62 + 9..12 digit.
    (normalized.len() >= 11 && normalized.len() <= 15).then_some(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalisasi_nomor_wa_berbagai_format() {
        assert_eq!(normalize_phone("08123456789").as_deref(), Some("628123456789"));
        assert_eq!(normalize_phone("+62 812-3456-789").as_deref(), Some("628123456789"));
        assert_eq!(normalize_phone("62812 3456 789").as_deref(), Some("628123456789"));
        assert_eq!(normalize_phone("8123456789").as_deref(), Some("628123456789"));
    }

    #[test]
    fn nomor_tidak_masuk_akal_ditolak() {
        assert_eq!(normalize_phone(""), None);
        assert_eq!(normalize_phone("123"), None);
        assert_eq!(normalize_phone("abcdefgh"), None);
        assert_eq!(normalize_phone("0812345678901234567"), None);
    }

    #[test]
    fn nisn_harus_sepuluh_digit() {
        let mk = |nisn: &str| CreateStudentRequest {
            school_id: None,
            current_classroom_id: None,
            nisn: Some(nisn.to_string()),
            nis: None,
            full_name: "Budi Santoso".into(),
            gender: Some("L".into()),
            birth_place: None,
            birth_date: None,
            religion: None,
            address: None,
            phone: None,
            father_name: None,
            mother_name: None,
            entry_year: None,
            guardians: vec![],
        };
        assert!(mk("123").validate().is_err());
        assert!(mk("0061234567").validate().is_ok());
    }

    #[test]
    fn validasi_wali_bersarang_ikut_diperiksa() {
        let req = CreateStudentRequest {
            school_id: None,
            current_classroom_id: None,
            nisn: None,
            nis: None,
            full_name: "Budi Santoso".into(),
            gender: None,
            birth_place: None,
            birth_date: None,
            religion: None,
            address: None,
            phone: None,
            father_name: None,
            mother_name: None,
            entry_year: None,
            guardians: vec![CreateGuardianRequest {
                relation: "ayah".into(),
                full_name: "A".into(), // terlalu pendek
                phone: None,
                whatsapp: None,
                email: Some("bukan-email".into()),
                telegram_chat_id: None,
                preferred_channel: "whatsapp".into(),
                is_primary: true,
                notify_enabled: true,
            }],
        };
        assert!(req.validate().is_err(), "wali tidak valid harus menggagalkan request");
    }
}
