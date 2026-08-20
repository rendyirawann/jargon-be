//! DTO sekolah, wilayah, tahun ajaran, dan rombel (kelas).

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

// =====================================================================
// Sekolah
// =====================================================================

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct School {
    pub id: Uuid,
    /// Nomor Pokok Sekolah Nasional (8 digit).
    pub npsn: String,
    pub name: String,
    pub slug: String,
    /// SD / SMP / SMA / SMK / SLB / TK / PAUD.
    pub jenjang: String,
    /// negeri / swasta.
    pub status: String,
    pub region_id: Option<Uuid>,
    pub region_name: Option<String>,
    pub address: Option<String>,
    pub village: Option<String>,
    pub district: Option<String>,
    pub postal_code: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub geofence_radius_m: i32,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub principal_name: Option<String>,
    pub logo_path: Option<String>,
    pub timezone: String,
    /// Ambang kemiripan wajah khusus sekolah; `null` = pakai default global.
    pub face_match_threshold: Option<f32>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Baris ringkas untuk tabel daftar sekolah di dashboard Superadmin.
#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct SchoolListItem {
    pub id: Uuid,
    pub npsn: String,
    pub name: String,
    pub jenjang: String,
    pub status: String,
    pub region_name: Option<String>,
    pub is_active: bool,
    pub student_count: i64,
    pub enrolled_face_count: i64,
    pub device_count: i64,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateSchoolRequest {
    #[validate(length(min = 6, max = 12, message = "NPSN harus 6-12 karakter"))]
    pub npsn: String,
    #[validate(length(min = 3, max = 200, message = "nama sekolah minimal 3 karakter"))]
    pub name: String,
    /// Salah satu: PAUD, TK, SD, SMP, SMA, SMK, SLB.
    pub jenjang: String,
    /// negeri atau swasta.
    #[serde(default = "default_status")]
    pub status: String,
    pub region_id: Option<Uuid>,
    pub address: Option<String>,
    pub village: Option<String>,
    pub district: Option<String>,
    #[validate(length(max = 10))]
    pub postal_code: Option<String>,
    #[validate(range(min = -11.0, max = 6.0, message = "latitude di luar wilayah Indonesia"))]
    pub latitude: Option<f64>,
    #[validate(range(min = 95.0, max = 141.0, message = "longitude di luar wilayah Indonesia"))]
    pub longitude: Option<f64>,
    #[validate(range(min = 20, max = 5000))]
    pub geofence_radius_m: Option<i32>,
    pub phone: Option<String>,
    #[validate(email(message = "format email tidak valid"))]
    pub email: Option<String>,
    pub principal_name: Option<String>,
    #[validate(range(min = 0.3, max = 0.99, message = "ambang kemiripan harus antara 0,3 dan 0,99"))]
    pub face_match_threshold: Option<f32>,
}

fn default_status() -> String {
    "negeri".to_string()
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateSchoolRequest {
    #[validate(length(min = 3, max = 200))]
    pub name: Option<String>,
    pub jenjang: Option<String>,
    pub status: Option<String>,
    pub region_id: Option<Uuid>,
    pub address: Option<String>,
    pub village: Option<String>,
    pub district: Option<String>,
    pub postal_code: Option<String>,
    #[validate(range(min = -11.0, max = 6.0))]
    pub latitude: Option<f64>,
    #[validate(range(min = 95.0, max = 141.0))]
    pub longitude: Option<f64>,
    #[validate(range(min = 20, max = 5000))]
    pub geofence_radius_m: Option<i32>,
    pub phone: Option<String>,
    #[validate(email)]
    pub email: Option<String>,
    pub principal_name: Option<String>,
    #[validate(range(min = 0.3, max = 0.99))]
    pub face_match_threshold: Option<f32>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SchoolFilter {
    /// Filter jenjang, mis. `SMA`.
    pub jenjang: Option<String>,
    pub status: Option<String>,
    pub region_id: Option<Uuid>,
    pub is_active: Option<bool>,
}

pub const SCHOOL_JENJANG: [&str; 7] = ["PAUD", "TK", "SD", "SMP", "SMA", "SMK", "SLB"];
pub const SCHOOL_STATUS: [&str; 2] = ["negeri", "swasta"];

// =====================================================================
// Wilayah
// =====================================================================

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct Region {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub kind: String,
    pub school_count: i64,
}

// =====================================================================
// Tahun ajaran
// =====================================================================

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct AcademicYear {
    pub id: Uuid,
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub is_active: bool,
}

// =====================================================================
// Rombel / kelas
// =====================================================================

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct Classroom {
    pub id: Uuid,
    pub school_id: Uuid,
    pub school_name: String,
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub name: String,
    pub grade_level: i16,
    pub major: Option<String>,
    pub homeroom_teacher_id: Option<Uuid>,
    pub homeroom_teacher_name: Option<String>,
    pub capacity: i16,
    pub student_count: i64,
    pub is_active: bool,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateClassroomRequest {
    /// Wajib untuk Superadmin; diabaikan (dipaksa ke sekolah sendiri) untuk
    /// pengguna tingkat sekolah.
    pub school_id: Option<Uuid>,
    /// Default: tahun ajaran yang sedang aktif.
    pub academic_year_id: Option<Uuid>,
    #[validate(length(min = 1, max = 60, message = "nama kelas wajib diisi"))]
    pub name: String,
    #[validate(range(min = 1, max = 13, message = "tingkat kelas harus 1-13"))]
    pub grade_level: i16,
    #[validate(length(max = 60))]
    pub major: Option<String>,
    pub homeroom_teacher_id: Option<Uuid>,
    #[validate(range(min = 1, max = 100))]
    pub capacity: Option<i16>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateClassroomRequest {
    #[validate(length(min = 1, max = 60))]
    pub name: Option<String>,
    #[validate(range(min = 1, max = 13))]
    pub grade_level: Option<i16>,
    pub major: Option<String>,
    pub homeroom_teacher_id: Option<Uuid>,
    #[validate(range(min = 1, max = 100))]
    pub capacity: Option<i16>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ClassroomFilter {
    pub school_id: Option<Uuid>,
    pub academic_year_id: Option<Uuid>,
    pub grade_level: Option<i16>,
    /// Hanya kelas yang menjadi tanggung jawab saya sebagai wali kelas.
    pub mine: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validasi_npsn_dan_nama() {
        let req = CreateSchoolRequest {
            npsn: "123".into(),
            name: "AB".into(),
            jenjang: "SMA".into(),
            status: "negeri".into(),
            region_id: None,
            address: None,
            village: None,
            district: None,
            postal_code: None,
            latitude: None,
            longitude: None,
            geofence_radius_m: None,
            phone: None,
            email: None,
            principal_name: None,
            face_match_threshold: None,
        };
        let err = req.validate().unwrap_err();
        let fields: Vec<String> = err.field_errors().keys().map(|k| k.to_string()).collect();
        assert!(fields.contains(&"npsn".to_string()));
        assert!(fields.contains(&"name".to_string()));
    }

    #[test]
    fn koordinat_di_luar_indonesia_ditolak() {
        let mut req = CreateSchoolRequest {
            npsn: "10259876".into(),
            name: "SMA Negeri 1 Medan".into(),
            jenjang: "SMA".into(),
            status: "negeri".into(),
            region_id: None,
            address: None,
            village: None,
            district: None,
            postal_code: None,
            latitude: Some(51.5),  // London
            longitude: Some(-0.12),
            geofence_radius_m: None,
            phone: None,
            email: None,
            principal_name: None,
            face_match_threshold: None,
        };
        assert!(req.validate().is_err());

        req.latitude = Some(3.5952);
        req.longitude = Some(98.6722);
        assert!(req.validate().is_ok());
    }

    #[test]
    fn ambang_kemiripan_dibatasi() {
        let base = |t: Option<f32>| CreateSchoolRequest {
            npsn: "10259876".into(),
            name: "SMA Negeri 1 Medan".into(),
            jenjang: "SMA".into(),
            status: "negeri".into(),
            region_id: None,
            address: None,
            village: None,
            district: None,
            postal_code: None,
            latitude: None,
            longitude: None,
            geofence_radius_m: None,
            phone: None,
            email: None,
            principal_name: None,
            face_match_threshold: t,
        };
        assert!(base(Some(0.1)).validate().is_err(), "ambang terlalu rendah berbahaya");
        assert!(base(Some(1.5)).validate().is_err());
        assert!(base(Some(0.62)).validate().is_ok());
    }

    #[test]
    fn tingkat_kelas_divalidasi() {
        let req = CreateClassroomRequest {
            school_id: None,
            academic_year_id: None,
            name: "X IPA 1".into(),
            grade_level: 99,
            major: None,
            homeroom_teacher_id: None,
            capacity: None,
        };
        assert!(req.validate().is_err());
    }
}
