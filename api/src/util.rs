//! Tipe & helper lintas-modul: envelope respons, paginasi, waktu WIB.

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::error::{ApiError, ApiResult};

// =====================================================================
// Waktu
// =====================================================================

/// Seluruh Sumatera Utara berada di WIB (UTC+7) dan Indonesia tidak
/// menerapkan DST, sehingga offset tetap sudah pasti benar — tidak perlu
/// database timezone.
pub const WIB_OFFSET_SECONDS: i32 = 7 * 3600;

pub fn wib() -> FixedOffset {
    FixedOffset::east_opt(WIB_OFFSET_SECONDS).expect("offset WIB valid")
}

pub fn now_wib() -> DateTime<FixedOffset> {
    Utc::now().with_timezone(&wib())
}

/// Tanggal absensi "hari ini" menurut waktu setempat sekolah.
///
/// Penting: absensi pukul 06:30 WIB tanggal 15 harus tercatat pada tanggal
/// 15, bukan 14 — padahal dalam UTC waktu itu masih tanggal 14 pukul 23:30.
pub fn today_wib() -> NaiveDate {
    now_wib().date_naive()
}

pub fn to_wib(ts: DateTime<Utc>) -> DateTime<FixedOffset> {
    ts.with_timezone(&wib())
}

/// Gabungkan tanggal + jam lokal menjadi instant absolut.
pub fn wib_datetime(date: NaiveDate, time: NaiveTime) -> DateTime<Utc> {
    let naive = date.and_time(time);
    wib()
        .from_local_datetime(&naive)
        .single()
        // Tidak ada DST di Indonesia, jadi cabang ini praktis tak terjangkau.
        .unwrap_or_else(|| wib().from_utc_datetime(&naive))
        .with_timezone(&Utc)
}

/// Nomor hari dalam pekan: 0 = Senin .. 6 = Minggu (cocok dengan bitmask
/// `attendance_rules.active_weekdays`).
pub fn weekday_index(date: NaiveDate) -> u32 {
    use chrono::Datelike;
    date.weekday().num_days_from_monday()
}

pub fn is_weekday_active(mask: i16, date: NaiveDate) -> bool {
    let bit = 1i16 << weekday_index(date);
    mask & bit != 0
}

pub fn format_time_wib(ts: Option<DateTime<Utc>>) -> Option<String> {
    ts.map(|t| to_wib(t).format("%H:%M").to_string())
}

pub fn format_date_id(date: NaiveDate) -> String {
    const BULAN: [&str; 12] = [
        "Januari", "Februari", "Maret", "April", "Mei", "Juni", "Juli",
        "Agustus", "September", "Oktober", "November", "Desember",
    ];
    use chrono::Datelike;
    format!(
        "{} {} {}",
        date.day(),
        BULAN[(date.month() - 1) as usize],
        date.year()
    )
}

// =====================================================================
// Envelope respons
// =====================================================================

/// Semua respons sukses memakai bentuk ini agar klien punya satu parser.
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiResponse<T> {
    /// Selalu `true` pada respons sukses.
    pub success: bool,
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn new(data: T) -> Self {
        Self { success: true, data, message: None }
    }

    pub fn with_message(data: T, message: impl Into<String>) -> Self {
        Self { success: true, data, message: Some(message.into()) }
    }
}

impl<T: Serialize> axum::response::IntoResponse for ApiResponse<T> {
    fn into_response(self) -> axum::response::Response {
        axum::Json(self).into_response()
    }
}

// =====================================================================
// Paginasi
// =====================================================================

pub const DEFAULT_PER_PAGE: i64 = 25;
pub const MAX_PER_PAGE: i64 = 200;

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PageQuery {
    /// Halaman, dimulai dari 1.
    #[param(minimum = 1, example = 1)]
    pub page: Option<i64>,
    /// Jumlah baris per halaman (maksimum 200).
    #[param(minimum = 1, maximum = 200, example = 25)]
    pub per_page: Option<i64>,
    /// Kata kunci pencarian bebas.
    pub q: Option<String>,
    /// Kolom pengurutan, mis. `full_name`. Divalidasi oleh whitelist tiap endpoint.
    pub sort_by: Option<String>,
    /// `asc` atau `desc`.
    pub sort_dir: Option<String>,
}

impl Default for PageQuery {
    fn default() -> Self {
        Self { page: None, per_page: None, q: None, sort_by: None, sort_dir: None }
    }
}

impl PageQuery {
    pub fn page(&self) -> i64 {
        self.page.unwrap_or(1).max(1)
    }

    pub fn per_page(&self) -> i64 {
        self.per_page.unwrap_or(DEFAULT_PER_PAGE).clamp(1, MAX_PER_PAGE)
    }

    pub fn offset(&self) -> i64 {
        (self.page() - 1) * self.per_page()
    }

    /// Pola pencarian untuk ILIKE. `None` bila kata kunci kosong/terlalu pendek.
    pub fn search_pattern(&self) -> Option<String> {
        self.q
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| s.len() >= 2)
            .map(|s| format!("%{}%", s.replace('%', "\\%").replace('_', "\\_")))
    }

    /// Bangun klausa ORDER BY dari whitelist.
    ///
    /// Nama kolom TIDAK BOLEH berasal langsung dari input pengguna — ini
    /// satu-satunya tempat string SQL dibentuk dari parameter, dan hanya
    /// nilai yang lolos whitelist yang dipakai.
    pub fn order_by(&self, allowed: &[&str], fallback: &str) -> String {
        let col = self
            .sort_by
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| allowed.contains(s))
            .unwrap_or(fallback);
        let dir = match self.sort_dir.as_deref().map(|s| s.to_ascii_lowercase()) {
            Some(d) if d == "asc" => "ASC",
            _ => "DESC",
        };
        format!("{col} {dir}")
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Paginated<T> {
    pub items: Vec<T>,
    pub meta: PageMeta,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PageMeta {
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
    pub total_pages: i64,
}

impl<T> Paginated<T> {
    pub fn new(items: Vec<T>, page: i64, per_page: i64, total: i64) -> Self {
        let total_pages = if per_page > 0 {
            (total + per_page - 1) / per_page
        } else {
            0
        };
        Self { items, meta: PageMeta { page, per_page, total, total_pages } }
    }
}

impl<T: Serialize> axum::response::IntoResponse for Paginated<T> {
    fn into_response(self) -> axum::response::Response {
        axum::Json(serde_json::json!({
            "success": true,
            "data": self.items,
            "meta": self.meta,
        }))
        .into_response()
    }
}

// =====================================================================
// Rentang tanggal
// =====================================================================

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DateRangeQuery {
    /// Tanggal awal (YYYY-MM-DD). Default: hari ini.
    pub from: Option<NaiveDate>,
    /// Tanggal akhir (YYYY-MM-DD). Default: sama dengan `from`.
    pub to: Option<NaiveDate>,
}

impl DateRangeQuery {
    /// Rentang yang sudah dinormalisasi & dibatasi.
    ///
    /// Batas 366 hari mencegah satu request laporan memindai seluruh
    /// riwayat 160 juta baris.
    pub fn resolve(&self) -> ApiResult<(NaiveDate, NaiveDate)> {
        let from = self.from.unwrap_or_else(today_wib);
        let to = self.to.unwrap_or(from);
        if to < from {
            return Err(ApiError::field(
                "to",
                "tanggal akhir tidak boleh sebelum tanggal awal",
            ));
        }
        if (to - from).num_days() > 366 {
            return Err(ApiError::field(
                "to",
                "rentang tanggal maksimum 366 hari, gunakan ekspor laporan untuk rentang lebih panjang",
            ));
        }
        Ok((from, to))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tanggal_wib_bukan_utc() {
        // 15 Agustus 2026 00:30 WIB = 14 Agustus 17:30 UTC.
        let utc = Utc.with_ymd_and_hms(2026, 8, 14, 17, 30, 0).unwrap();
        assert_eq!(to_wib(utc).date_naive(), NaiveDate::from_ymd_opt(2026, 8, 15).unwrap());
    }

    #[test]
    fn wib_datetime_bolak_balik() {
        let d = NaiveDate::from_ymd_opt(2026, 8, 15).unwrap();
        let t = NaiveTime::from_hms_opt(7, 15, 0).unwrap();
        let utc = wib_datetime(d, t);
        assert_eq!(utc.hour_minute_wib(), (7, 15));
    }

    trait HourMinuteWib {
        fn hour_minute_wib(&self) -> (u32, u32);
    }
    impl HourMinuteWib for DateTime<Utc> {
        fn hour_minute_wib(&self) -> (u32, u32) {
            use chrono::Timelike;
            let l = to_wib(*self);
            (l.hour(), l.minute())
        }
    }

    #[test]
    fn indeks_hari_senin_nol() {
        // 17 Agustus 2026 adalah hari Senin.
        let senin = NaiveDate::from_ymd_opt(2026, 8, 17).unwrap();
        assert_eq!(weekday_index(senin), 0);
        let minggu = NaiveDate::from_ymd_opt(2026, 8, 16).unwrap();
        assert_eq!(weekday_index(minggu), 6);
    }

    #[test]
    fn bitmask_hari_aktif_senin_jumat() {
        let mask = 31i16; // 0b0011111
        let senin = NaiveDate::from_ymd_opt(2026, 8, 17).unwrap();
        let jumat = NaiveDate::from_ymd_opt(2026, 8, 21).unwrap();
        let sabtu = NaiveDate::from_ymd_opt(2026, 8, 22).unwrap();
        assert!(is_weekday_active(mask, senin));
        assert!(is_weekday_active(mask, jumat));
        assert!(!is_weekday_active(mask, sabtu));
    }

    #[test]
    fn paginasi_menghitung_total_halaman() {
        let p = Paginated::new(vec![1, 2, 3], 1, 25, 51);
        assert_eq!(p.meta.total_pages, 3);
        let p = Paginated::new(Vec::<i32>::new(), 1, 25, 0);
        assert_eq!(p.meta.total_pages, 0);
    }

    #[test]
    fn per_page_dibatasi() {
        let q = PageQuery { page: Some(0), per_page: Some(10_000), ..Default::default() };
        assert_eq!(q.page(), 1);
        assert_eq!(q.per_page(), MAX_PER_PAGE);
        assert_eq!(q.offset(), 0);
    }

    #[test]
    fn order_by_hanya_menerima_whitelist() {
        let allowed = ["full_name", "created_at"];
        let q = PageQuery {
            sort_by: Some("full_name; DROP TABLE students".into()),
            sort_dir: Some("asc".into()),
            ..Default::default()
        };
        // Input berbahaya diabaikan, jatuh ke fallback.
        assert_eq!(q.order_by(&allowed, "created_at"), "created_at ASC");

        let q = PageQuery {
            sort_by: Some("full_name".into()),
            sort_dir: Some("asc".into()),
            ..Default::default()
        };
        assert_eq!(q.order_by(&allowed, "created_at"), "full_name ASC");
    }

    #[test]
    fn pola_pencarian_meng_escape_wildcard() {
        let q = PageQuery { q: Some("100%_budi".into()), ..Default::default() };
        assert_eq!(q.search_pattern().unwrap(), "%100\\%\\_budi%");

        let q = PageQuery { q: Some("a".into()), ..Default::default() };
        assert!(q.search_pattern().is_none(), "kata kunci 1 karakter diabaikan");
    }

    #[test]
    fn rentang_tanggal_divalidasi() {
        let r = DateRangeQuery {
            from: Some(NaiveDate::from_ymd_opt(2026, 8, 10).unwrap()),
            to: Some(NaiveDate::from_ymd_opt(2026, 8, 1).unwrap()),
        };
        assert!(r.resolve().is_err(), "to < from harus ditolak");

        let r = DateRangeQuery {
            from: Some(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()),
            to: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        };
        assert!(r.resolve().is_err(), "rentang terlalu panjang harus ditolak");

        let d = NaiveDate::from_ymd_opt(2026, 8, 14).unwrap();
        let r = DateRangeQuery { from: Some(d), to: None };
        assert_eq!(r.resolve().unwrap(), (d, d));
    }

    #[test]
    fn format_tanggal_indonesia() {
        let d = NaiveDate::from_ymd_opt(2026, 8, 14).unwrap();
        assert_eq!(format_date_id(d), "14 Agustus 2026");
    }
}
