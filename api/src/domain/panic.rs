//! DTO Panic Button — kanal pengaduan anonim.
//!
//! ATURAN YANG TIDAK BOLEH DILANGGAR
//!   Tidak ada satu pun struct di berkas ini yang memuat `author_user_id`,
//!   kecuali [`UnmaskedAuthor`] yang hanya dikembalikan endpoint pembukaan
//!   identitas. Bila suatu saat perlu menambah field baru pada respons feed,
//!   pastikan field itu tidak dapat dipakai mempersempit siapa pelapornya.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

pub const SEVERITIES: [&str; 4] = ["rendah", "sedang", "tinggi", "darurat"];
pub const STATUSES: [&str; 5] = [
    "baru",
    "diverifikasi",
    "ditindaklanjuti",
    "selesai",
    "ditolak",
];
pub const MODERATION_STATUSES: [&str; 3] = ["pending", "approved", "rejected"];
pub const VISIBILITIES: [&str; 2] = ["publik", "terbatas"];

/// Tingkat keparahan yang otomatis dianggap mendesak dan diteruskan ke Dinas
/// tanpa menunggu moderasi.
pub const URGENT_SEVERITIES: [&str; 2] = ["tinggi", "darurat"];

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct PanicCategory {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub default_severity: String,
}

/// Satu kartu di feed. Sengaja TIDAK memuat identitas pelapor.
#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct PanicFeedItem {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub category_code: String,
    pub category_name: String,
    pub category_icon: Option<String>,
    /// Nama tampilan anonim, mis. `Siswa#7K4M`.
    pub anonymous_handle: String,
    /// Peran pelapor (`siswa`, `guru`, ...) — berguna untuk penanganan,
    /// tidak mempersempit identitas di sekolah berpopulasi ratusan.
    pub author_role: String,
    /// Nama sekolah. Untuk feed publik nilainya sudah disamarkan.
    pub school_label: String,
    /// Id sekolah — hanya diisi untuk peran yang menangani, agar dashboard
    /// bisa menautkan laporan ke halaman sekolahnya. Untuk warga biasa
    /// nilainya `null`: mengembalikannya akan membatalkan penyamaran nama
    /// sekolah di baris sebelumnya.
    pub school_id: Option<Uuid>,
    pub title: String,
    pub body: String,
    pub severity: String,
    pub status: String,
    pub support_count: i16,
    pub comment_count: i16,
    /// `true` bila pengguna yang meminta adalah pelapornya.
    pub is_mine: bool,
    /// `true` bila pengguna yang meminta sudah menekan "saya juga mengalami".
    pub is_supported: bool,
    pub media: Vec<String>,
    pub handled_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
}

/// Detail laporan beserta lini masa penanganan dan komentar.
#[derive(Debug, Serialize, ToSchema)]
pub struct PanicReportDetail {
    #[serde(flatten)]
    pub report: PanicFeedItem,
    pub timeline: Vec<PanicTimelineEntry>,
    pub comments: Vec<PanicComment>,
    /// Nama sekolah sebenarnya. Hanya diisi untuk peran yang menangani.
    pub school_name: Option<String>,
    pub resolution: Option<String>,
    /// Status moderasi — hanya relevan bagi pelapor dan moderator.
    pub moderation_status: String,
    pub moderation_note: Option<String>,
}

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct PanicTimelineEntry {
    pub status: String,
    pub note: Option<String>,
    /// Nama petugas bila tindakan dilakukan secara resmi.
    pub actor_label: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct PanicComment {
    pub id: Uuid,
    /// Handle anonim, atau `null` bila komentar resmi.
    pub anonymous_handle: Option<String>,
    pub is_official: bool,
    /// Nama & jabatan petugas — hanya untuk komentar resmi.
    pub official_name: Option<String>,
    pub official_title: Option<String>,
    pub body: String,
    pub is_mine: bool,
    pub created_at: DateTime<Utc>,
}

// =====================================================================
// Request
// =====================================================================

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateReportRequest {
    pub category_id: Uuid,
    #[validate(length(min = 10, max = 150, message = "judul 10-150 karakter"))]
    pub title: String,
    #[validate(length(min = 20, max = 5000, message = "isi laporan minimal 20 karakter"))]
    pub body: String,
    /// `rendah` / `sedang` / `tinggi` / `darurat`.
    /// Kosongkan untuk memakai tingkat bawaan kategori.
    pub severity: Option<String>,
    /// `publik` (tampil di feed setelah dimoderasi) atau `terbatas`
    /// (hanya terlihat Dinas, tidak pernah masuk feed).
    #[serde(default = "default_visibility")]
    pub visibility: String,
    /// Foto bukti dalam base64, maksimum 4 berkas.
    ///
    /// Metadata EXIF dibuang server sebelum disimpan — koordinat GPS pada
    /// foto akan membocorkan lokasi pelapor.
    #[serde(default)]
    #[validate(length(max = 4, message = "maksimum 4 lampiran"))]
    pub media_base64: Vec<String>,
}

fn default_visibility() -> String {
    "publik".to_string()
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateReportResponse {
    pub id: Uuid,
    pub anonymous_handle: String,
    pub severity: String,
    pub moderation_status: String,
    /// Penjelasan untuk pelapor tentang apa yang terjadi selanjutnya.
    pub message: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateCommentRequest {
    #[validate(length(min = 2, max = 2000, message = "komentar 2-2000 karakter"))]
    pub body: String,
    /// Kirim sebagai komentar resmi (menampilkan nama & jabatan).
    /// Hanya dihormati bila pengguna punya izin menangani pengaduan.
    #[serde(default)]
    pub as_official: bool,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ModerateReportRequest {
    /// `approved` atau `rejected`.
    pub moderation_status: String,
    #[validate(length(max = 300))]
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateReportStatusRequest {
    /// `diverifikasi` / `ditindaklanjuti` / `selesai` / `ditolak`.
    pub status: String,
    #[validate(length(min = 3, max = 500, message = "catatan tindak lanjut wajib diisi"))]
    pub note: String,
    /// Hasil akhir penanganan, wajib bila status `selesai`.
    #[validate(length(max = 2000))]
    pub resolution: Option<String>,
    /// Tampilkan catatan ini pada lini masa yang dilihat pelapor.
    #[serde(default = "default_true")]
    pub visible_to_reporter: bool,
}

fn default_true() -> bool {
    true
}

/// Permintaan membuka identitas pelapor.
///
/// Alasan WAJIB dan disimpan permanen. Tanpa itu, izin
/// `unmask_panic_report` hanyalah janji yang tidak bisa diaudit.
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UnmaskRequest {
    #[validate(length(min = 20, max = 500,
        message = "alasan pembukaan identitas wajib diisi minimal 20 karakter"))]
    pub reason: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UnmaskedAuthor {
    pub report_id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub identity_number: Option<String>,
    pub role: String,
    pub school_name: Option<String>,
    /// Peringatan yang ditampilkan kembali ke petugas.
    pub notice: String,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct FeedFilter {
    /// Batasi ke satu sekolah (hanya untuk peran yang menangani).
    pub school_id: Option<Uuid>,
    pub category_code: Option<String>,
    pub severity: Option<String>,
    pub status: Option<String>,
    /// `true` = hanya laporan milik saya sendiri.
    pub mine: Option<bool>,
    /// `true` = hanya yang menunggu moderasi (untuk petugas).
    pub pending_moderation: Option<bool>,
}

/// Ringkasan untuk dashboard penanganan.
#[derive(Debug, Serialize, ToSchema)]
pub struct PanicStats {
    pub total: i64,
    pub baru: i64,
    pub ditindaklanjuti: i64,
    pub selesai: i64,
    pub menunggu_moderasi: i64,
    pub darurat_belum_ditangani: i64,
    pub per_kategori: Vec<CategoryCount>,
}

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct CategoryCount {
    pub category_name: String,
    pub total: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> CreateReportRequest {
        CreateReportRequest {
            category_id: Uuid::new_v4(),
            title: "Ada pungutan liar di kelas".into(),
            body: "Wali kelas meminta uang seragam di luar ketentuan sekolah.".into(),
            severity: None,
            visibility: "publik".into(),
            media_base64: vec![],
        }
    }

    #[test]
    fn laporan_valid_diterima() {
        assert!(base().validate().is_ok());
    }

    #[test]
    fn judul_terlalu_pendek_ditolak() {
        // Judul satu kata membuat laporan tidak bisa ditriase petugas.
        let mut r = base();
        r.title = "Pungli".into();
        assert!(r.validate().is_err());
    }

    #[test]
    fn isi_terlalu_pendek_ditolak() {
        let mut r = base();
        r.body = "tidak enak".into();
        assert!(r.validate().is_err());
    }

    #[test]
    fn lampiran_dibatasi_empat() {
        let mut r = base();
        r.media_base64 = vec!["x".repeat(200); 5];
        assert!(r.validate().is_err());
    }

    #[test]
    fn alasan_unmask_wajib_panjang() {
        // Alasan sependek "audit" tidak dapat dipertanggungjawabkan bila
        // suatu saat pembukaan identitas dipersoalkan.
        let short = UnmaskRequest { reason: "audit".into() };
        assert!(short.validate().is_err());

        let ok = UnmaskRequest {
            reason: "Permintaan penyidik Polrestabes Medan nomor B/123/VIII/2026".into(),
        };
        assert!(ok.validate().is_ok());
    }

    #[test]
    fn catatan_tindak_lanjut_wajib() {
        let r = UpdateReportStatusRequest {
            status: "ditindaklanjuti".into(),
            note: "ok".into(),
            resolution: None,
            visible_to_reporter: true,
        };
        assert!(r.validate().is_err(), "catatan terlalu pendek harus ditolak");
    }

    #[test]
    fn tingkat_darurat_termasuk_mendesak() {
        assert!(URGENT_SEVERITIES.contains(&"darurat"));
        assert!(URGENT_SEVERITIES.contains(&"tinggi"));
        assert!(!URGENT_SEVERITIES.contains(&"rendah"));
    }
}
