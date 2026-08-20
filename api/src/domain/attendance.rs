//! DTO absensi & pengenalan wajah harian.

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

// =====================================================================
// Status
// =====================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum AttendanceStatus {
    Hadir,
    Terlambat,
    Izin,
    Sakit,
    Alfa,
    Dispensasi,
}

impl AttendanceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hadir => "hadir",
            Self::Terlambat => "terlambat",
            Self::Izin => "izin",
            Self::Sakit => "sakit",
            Self::Alfa => "alfa",
            Self::Dispensasi => "dispensasi",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "hadir" => Self::Hadir,
            "terlambat" => Self::Terlambat,
            "izin" => Self::Izin,
            "sakit" => Self::Sakit,
            "alfa" => Self::Alfa,
            "dispensasi" => Self::Dispensasi,
            _ => return None,
        })
    }

    /// Status yang dihitung sebagai "masuk sekolah".
    pub fn is_present(self) -> bool {
        matches!(self, Self::Hadir | Self::Terlambat | Self::Dispensasi)
    }

    /// Kunci template notifikasi yang cocok untuk status ini.
    pub fn notification_key(self) -> &'static str {
        match self {
            Self::Terlambat => "late",
            Self::Alfa => "absent",
            Self::Sakit => "sick",
            Self::Izin => "permit",
            _ => "check_in",
        }
    }
}

/// Arah scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScanDirection {
    CheckIn,
    CheckOut,
    /// Tentukan otomatis dari jam & status absensi hari ini.
    Auto,
}

impl ScanDirection {
    /// Dipakai saat arah dikirim sebagai string bebas (mis. dari antrean
    /// offline tablet yang menyimpan payload apa adanya).
    #[allow(dead_code)]
    pub fn parse(s: &str) -> Self {
        match s {
            "check_in" => Self::CheckIn,
            "check_out" => Self::CheckOut,
            _ => Self::Auto,
        }
    }
}

// =====================================================================
// Pengenalan wajah (dipanggil tablet)
// =====================================================================

/// Payload absensi dari tablet.
///
/// PERHATIKAN: tidak ada field gambar. Tablet hanya mengirim vektor, dan
/// server tidak menyimpan vektor itu — hanya memakainya untuk mencocokkan.
// `classroom_id` diterima dari tablet sebagai konteks dan tercatat pada log
// scan; arah absen sendiri ditentukan dari konfigurasi perangkat.
#[allow(dead_code)]
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct RecognizeRequest {
    /// Embedding 512 dimensi (L2-normalized) dari wajah yang tertangkap.
    #[validate(length(min = 1, message = "embedding wajib dikirim"))]
    pub embedding: Vec<f32>,
    /// Versi model di perangkat; harus sama dengan versi embedding tersimpan.
    pub model_version: String,
    /// Skor liveness dari perangkat (0..1) — hasil cek kedip/gerak kepala.
    #[validate(range(min = 0.0, max = 1.0))]
    pub liveness_score: f32,
    /// Waktu perangkat saat menangkap wajah. Dipakai untuk deteksi replay.
    pub client_time: DateTime<Utc>,
    /// Nilai acak sekali pakai per scan. Mencegah pengiriman ulang payload.
    #[validate(length(min = 8, max = 100, message = "nonce minimal 8 karakter"))]
    pub nonce: String,
    /// Paksa arah scan. Kosongkan agar server menentukan sendiri.
    pub direction: Option<ScanDirection>,
    /// Kelas tempat tablet berada (untuk tablet per kelas).
    pub classroom_id: Option<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RecognizeResponse {
    /// `true` bila wajah dikenali dan absensi tercatat/diperbarui.
    pub matched: bool,
    /// Tindakan yang terjadi.
    pub action: RecognizeAction,
    /// Pesan siap-tampil untuk layar tablet (bahasa Indonesia).
    pub message: String,
    pub student: Option<RecognizedStudent>,
    pub attendance: Option<AttendanceRecord>,
    /// Skor kemiripan kandidat terbaik.
    pub similarity: Option<f32>,
    /// Selisih dengan siswa lain terdekat. Kecil = ambigu.
    pub margin: Option<f32>,
    /// Jumlah embedding yang diperiksa — berguna untuk diagnosis lapangan.
    pub candidates_scanned: usize,
    pub processing_ms: u64,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecognizeAction {
    /// Absen masuk berhasil dicatat.
    CheckedIn,
    /// Absen pulang berhasil dicatat.
    CheckedOut,
    /// Siswa dikenali tapi sudah absen — tidak ada perubahan.
    AlreadyRecorded,
    /// Wajah tidak dikenali di sekolah ini.
    NoMatch,
    /// Dikenali tapi skor di bawah ambang / margin terlalu kecil.
    LowConfidence,
    /// Ditolak oleh aturan (di luar jam, hari libur, liveness gagal, replay).
    Rejected,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RecognizedStudent {
    pub id: Uuid,
    pub full_name: String,
    pub nis: Option<String>,
    pub nisn: Option<String>,
    pub classroom_id: Option<Uuid>,
    pub classroom_name: Option<String>,
    pub school_id: Uuid,
    pub school_name: String,
    /// Pas foto administratif untuk konfirmasi visual di layar tablet.
    pub photo_url: Option<String>,
}

// =====================================================================
// Baris absensi
// =====================================================================

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
pub struct AttendanceRecord {
    pub id: Uuid,
    pub attendance_date: NaiveDate,
    pub school_id: Uuid,
    pub school_name: String,
    pub student_id: Uuid,
    pub student_name: String,
    pub student_nis: Option<String>,
    pub classroom_id: Option<Uuid>,
    pub classroom_name: Option<String>,
    pub check_in_at: Option<DateTime<Utc>>,
    pub check_out_at: Option<DateTime<Utc>>,
    pub status: String,
    pub late_minutes: i32,
    pub duration_minutes: Option<i32>,
    pub check_in_method: Option<String>,
    pub check_out_method: Option<String>,
    pub notes: Option<String>,
    pub notification_status: String,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AttendanceFilter {
    pub school_id: Option<Uuid>,
    pub classroom_id: Option<Uuid>,
    pub student_id: Option<Uuid>,
    /// hadir / terlambat / izin / sakit / alfa / dispensasi.
    pub status: Option<String>,
    /// Hanya yang belum absen pulang.
    pub missing_check_out: Option<bool>,
}

/// Koreksi manual oleh guru/staff (mis. siswa lupa absen, izin, sakit).
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ManualAttendanceRequest {
    pub student_id: Uuid,
    /// Default: hari ini (WIB).
    pub attendance_date: Option<NaiveDate>,
    pub status: AttendanceStatus,
    /// Jam masuk (WIB, format HH:MM). Wajib bila status hadir/terlambat.
    pub check_in_time: Option<NaiveTime>,
    pub check_out_time: Option<NaiveTime>,
    #[validate(length(min = 3, max = 300, message = "alasan koreksi wajib diisi minimal 3 karakter"))]
    pub notes: String,
    /// Kirim notifikasi ke wali murid untuk perubahan ini.
    #[serde(default)]
    pub notify_guardian: bool,
}

/// Penandaan massal, mis. satu kelas ikut lomba (dispensasi).
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct BulkAttendanceRequest {
    #[validate(length(min = 1, max = 500, message = "pilih antara 1 sampai 500 siswa"))]
    pub student_ids: Vec<Uuid>,
    pub attendance_date: Option<NaiveDate>,
    pub status: AttendanceStatus,
    #[validate(length(min = 3, max = 300))]
    pub notes: String,
    #[serde(default)]
    pub notify_guardian: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BulkAttendanceResponse {
    pub updated: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

// =====================================================================
// Ringkasan & laporan
// =====================================================================

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct AttendanceSummary {
    pub summary_date: NaiveDate,
    pub total_students: i64,
    pub hadir: i64,
    pub terlambat: i64,
    pub izin: i64,
    pub sakit: i64,
    pub alfa: i64,
    pub dispensasi: i64,
    pub belum_absen: i64,
}

impl AttendanceSummary {
    pub fn attendance_rate(&self) -> f64 {
        if self.total_students == 0 {
            return 0.0;
        }
        let present = self.hadir + self.terlambat + self.dispensasi;
        (present as f64 / self.total_students as f64) * 100.0
    }
}

/// Ringkasan per kelas untuk halaman monitoring guru/kepala sekolah.
#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct ClassroomSummary {
    pub classroom_id: Uuid,
    pub classroom_name: String,
    pub grade_level: i16,
    pub homeroom_teacher_name: Option<String>,
    pub total_students: i64,
    pub hadir: i64,
    pub terlambat: i64,
    pub izin: i64,
    pub sakit: i64,
    pub alfa: i64,
    pub belum_absen: i64,
}

/// Rekap per siswa untuk rentang tanggal (rapor kehadiran).
#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct StudentAttendanceRecap {
    pub student_id: Uuid,
    pub student_name: String,
    pub nis: Option<String>,
    pub classroom_name: Option<String>,
    pub hadir: i64,
    pub terlambat: i64,
    pub izin: i64,
    pub sakit: i64,
    pub alfa: i64,
    pub total_late_minutes: i64,
    pub effective_days: i64,
}

impl StudentAttendanceRecap {
    /// Persentase kehadiran untuk cetak rapor. Dipakai oleh dashboard lewat
    /// perhitungan yang sama, dan diuji di sini agar rumusnya tidak berbeda.
    #[allow(dead_code)]
    pub fn presence_percentage(&self) -> f64 {
        if self.effective_days == 0 {
            return 0.0;
        }
        ((self.hadir + self.terlambat) as f64 / self.effective_days as f64) * 100.0
    }
}

/// Statistik tingkat provinsi untuk dashboard Superadmin.
#[derive(Debug, Serialize, ToSchema)]
pub struct ProvinceOverview {
    pub summary_date: NaiveDate,
    pub total_schools: i64,
    pub active_schools: i64,
    /// Sekolah yang sudah ada minimal satu scan hari ini.
    pub reporting_schools: i64,
    pub total_students: i64,
    pub enrolled_students: i64,
    pub total_devices: i64,
    pub online_devices: i64,
    pub attendance: AttendanceSummary,
    pub top_schools_by_rate: Vec<SchoolRate>,
    pub lowest_schools_by_rate: Vec<SchoolRate>,
}

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
pub struct SchoolRate {
    pub school_id: Uuid,
    pub school_name: String,
    pub jenjang: String,
    pub total_students: i64,
    pub present: i64,
    pub rate: f64,
}

// =====================================================================
// Aturan jam absensi
// =====================================================================

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct AttendanceRule {
    pub id: Uuid,
    pub school_id: Uuid,
    pub classroom_id: Option<Uuid>,
    pub name: String,
    pub check_in_opens_at: NaiveTime,
    pub check_in_start_at: NaiveTime,
    pub check_in_due_at: NaiveTime,
    pub check_in_closes_at: NaiveTime,
    pub check_out_opens_at: NaiveTime,
    pub check_out_closes_at: NaiveTime,
    pub late_grace_minutes: i16,
    pub active_weekdays: i16,
    pub require_check_out: bool,
    pub is_active: bool,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpsertAttendanceRuleRequest {
    pub school_id: Option<Uuid>,
    /// Kosongkan untuk aturan seluruh sekolah.
    pub classroom_id: Option<Uuid>,
    #[validate(length(min = 2, max = 80))]
    pub name: Option<String>,
    pub check_in_opens_at: NaiveTime,
    pub check_in_start_at: NaiveTime,
    pub check_in_due_at: NaiveTime,
    pub check_in_closes_at: NaiveTime,
    pub check_out_opens_at: NaiveTime,
    pub check_out_closes_at: NaiveTime,
    #[validate(range(min = 0, max = 120))]
    pub late_grace_minutes: Option<i16>,
    /// Bitmask hari aktif: bit0=Senin .. bit6=Minggu. 31 = Senin-Jumat.
    #[validate(range(min = 1, max = 127))]
    pub active_weekdays: Option<i16>,
    pub require_check_out: Option<bool>,
}

impl UpsertAttendanceRuleRequest {
    /// Konsistensi antar-jam tidak bisa dinyatakan lewat atribut `validate`,
    /// jadi diperiksa manual di sini.
    pub fn validate_windows(&self) -> Result<(), crate::error::ApiError> {
        use crate::error::{ApiError, FieldError};
        let mut errs = Vec::new();
        if self.check_in_start_at < self.check_in_opens_at {
            errs.push(FieldError::new(
                "check_in_start_at",
                "jam mulai hadir tidak boleh sebelum gerbang absen dibuka",
            ));
        }
        if self.check_in_due_at < self.check_in_start_at {
            errs.push(FieldError::new(
                "check_in_due_at",
                "batas terlambat harus setelah jam mulai hadir",
            ));
        }
        if self.check_in_closes_at < self.check_in_due_at {
            errs.push(FieldError::new(
                "check_in_closes_at",
                "jam tutup absen masuk harus setelah batas terlambat",
            ));
        }
        if self.check_out_closes_at <= self.check_out_opens_at {
            errs.push(FieldError::new(
                "check_out_closes_at",
                "jam tutup absen pulang harus setelah jam buka",
            ));
        }
        if errs.is_empty() {
            Ok(())
        } else {
            Err(ApiError::validation(errs))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_bolak_balik_ke_string() {
        for s in ["hadir", "terlambat", "izin", "sakit", "alfa", "dispensasi"] {
            assert_eq!(AttendanceStatus::parse(s).unwrap().as_str(), s);
        }
        assert!(AttendanceStatus::parse("tidak_ada").is_none());
    }

    #[test]
    fn status_hadir_dihitung_masuk() {
        assert!(AttendanceStatus::Hadir.is_present());
        assert!(AttendanceStatus::Terlambat.is_present());
        assert!(AttendanceStatus::Dispensasi.is_present());
        assert!(!AttendanceStatus::Alfa.is_present());
        assert!(!AttendanceStatus::Izin.is_present());
    }

    #[test]
    fn template_notifikasi_sesuai_status() {
        assert_eq!(AttendanceStatus::Terlambat.notification_key(), "late");
        assert_eq!(AttendanceStatus::Alfa.notification_key(), "absent");
        assert_eq!(AttendanceStatus::Hadir.notification_key(), "check_in");
    }

    #[test]
    fn tingkat_kehadiran_dihitung() {
        let s = AttendanceSummary {
            summary_date: NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
            total_students: 200,
            hadir: 150,
            terlambat: 20,
            izin: 5,
            sakit: 5,
            alfa: 20,
            dispensasi: 0,
            belum_absen: 0,
        };
        assert!((s.attendance_rate() - 85.0).abs() < 1e-9);

        let kosong = AttendanceSummary { total_students: 0, ..s };
        assert_eq!(kosong.attendance_rate(), 0.0);
    }

    #[test]
    fn persentase_rekap_siswa_aman_saat_nol_hari() {
        let r = StudentAttendanceRecap {
            student_id: Uuid::new_v4(),
            student_name: "Budi".into(),
            nis: None,
            classroom_name: None,
            hadir: 0,
            terlambat: 0,
            izin: 0,
            sakit: 0,
            alfa: 0,
            total_late_minutes: 0,
            effective_days: 0,
        };
        assert_eq!(r.presence_percentage(), 0.0);
    }

    fn rule(
        opens: (u32, u32),
        start: (u32, u32),
        due: (u32, u32),
        closes: (u32, u32),
    ) -> UpsertAttendanceRuleRequest {
        let t = |h: u32, m: u32| NaiveTime::from_hms_opt(h, m, 0).unwrap();
        UpsertAttendanceRuleRequest {
            school_id: None,
            classroom_id: None,
            name: None,
            check_in_opens_at: t(opens.0, opens.1),
            check_in_start_at: t(start.0, start.1),
            check_in_due_at: t(due.0, due.1),
            check_in_closes_at: t(closes.0, closes.1),
            check_out_opens_at: t(12, 0),
            check_out_closes_at: t(18, 0),
            late_grace_minutes: None,
            active_weekdays: None,
            require_check_out: None,
        }
    }

    #[test]
    fn jendela_jam_yang_konsisten_diterima() {
        assert!(rule((5, 30), (6, 30), (7, 15), (9, 0)).validate_windows().is_ok());
    }

    #[test]
    fn jendela_jam_terbalik_ditolak() {
        // Batas terlambat lebih awal daripada jam mulai hadir.
        let r = rule((5, 30), (7, 0), (6, 30), (9, 0));
        assert!(r.validate_windows().is_err());
    }

    #[test]
    fn jam_pulang_terbalik_ditolak() {
        let mut r = rule((5, 30), (6, 30), (7, 15), (9, 0));
        r.check_out_opens_at = NaiveTime::from_hms_opt(18, 0, 0).unwrap();
        r.check_out_closes_at = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
        assert!(r.validate_windows().is_err());
    }

    #[test]
    fn arah_scan_diparse() {
        assert_eq!(ScanDirection::parse("check_in"), ScanDirection::CheckIn);
        assert_eq!(ScanDirection::parse("check_out"), ScanDirection::CheckOut);
        assert_eq!(ScanDirection::parse("apa pun"), ScanDirection::Auto);
    }
}
