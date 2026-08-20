//! Aturan jam absensi: penentuan hadir/terlambat, jendela waktu, hari libur.
//!
//! Logika di sini murni (tanpa I/O) supaya bisa diuji menyeluruh — ini bagian
//! yang paling mudah salah dan paling terasa akibatnya: satu kesalahan
//! pembulatan menit membuat ribuan siswa tercatat terlambat.

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::attendance::AttendanceStatus;
use crate::error::ApiResult;
use crate::util;

// Beberapa field belum dipakai jalur pengenalan wajah (`id`, `school_id`,
// `classroom_id`, `check_in_start_at`, `require_check_out`) tetapi merupakan
// bagian utuh dari aturan yang dibaca dari database dan ditampilkan pada
// endpoint /v1/attendance-rules serta heartbeat tablet.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct EffectiveRule {
    pub id: Option<Uuid>,
    pub school_id: Uuid,
    pub classroom_id: Option<Uuid>,
    pub check_in_opens_at: NaiveTime,
    pub check_in_start_at: NaiveTime,
    pub check_in_due_at: NaiveTime,
    pub check_in_closes_at: NaiveTime,
    pub check_out_opens_at: NaiveTime,
    pub check_out_closes_at: NaiveTime,
    pub late_grace_minutes: i16,
    pub active_weekdays: i16,
    pub require_check_out: bool,
}

impl EffectiveRule {
    /// Aturan bawaan bila sekolah belum mengatur apa pun.
    /// Mengikuti jam sekolah umum di Sumatera Utara: masuk 07:15, Senin-Jumat.
    pub fn default_for(school_id: Uuid) -> Self {
        let t = |h, m| NaiveTime::from_hms_opt(h, m, 0).expect("jam valid");
        Self {
            id: None,
            school_id,
            classroom_id: None,
            check_in_opens_at: t(5, 30),
            check_in_start_at: t(6, 30),
            check_in_due_at: t(7, 15),
            check_in_closes_at: t(9, 0),
            check_out_opens_at: t(12, 0),
            check_out_closes_at: t(18, 0),
            late_grace_minutes: 0,
            active_weekdays: 31, // Senin-Jumat
            require_check_out: true,
        }
    }

    /// Batas akhir dianggap tepat waktu, termasuk toleransi.
    pub fn on_time_limit(&self) -> NaiveTime {
        self.check_in_due_at
            + chrono::Duration::minutes(self.late_grace_minutes as i64)
    }

    pub fn is_active_day(&self, date: NaiveDate) -> bool {
        util::is_weekday_active(self.active_weekdays, date)
    }

    /// Apakah jam lokal masih dalam jendela absen masuk.
    pub fn in_check_in_window(&self, local: NaiveTime) -> bool {
        local >= self.check_in_opens_at && local <= self.check_in_closes_at
    }

    pub fn in_check_out_window(&self, local: NaiveTime) -> bool {
        local >= self.check_out_opens_at && local <= self.check_out_closes_at
    }

    /// Tentukan status & keterlambatan dari jam kedatangan (waktu lokal WIB).
    ///
    /// * sebelum batas (termasuk toleransi) -> `hadir`, 0 menit
    /// * setelah batas, masih sebelum tutup -> `terlambat`, selisih menit
    /// * setelah tutup                      -> `alfa`
    pub fn classify_check_in(&self, local: NaiveTime) -> (AttendanceStatus, i32) {
        let limit = self.on_time_limit();
        if local <= limit {
            (AttendanceStatus::Hadir, 0)
        } else if local <= self.check_in_closes_at {
            // Selisih dihitung dari batas resmi (`check_in_due_at`), BUKAN dari
            // batas+toleransi: toleransi menentukan apakah dianggap terlambat,
            // sedangkan angka menit yang dilaporkan ke orang tua harus jujur
            // terhadap jam masuk sebenarnya.
            let minutes = (local - self.check_in_due_at).num_minutes().max(0) as i32;
            (AttendanceStatus::Terlambat, minutes)
        } else {
            (AttendanceStatus::Alfa, 0)
        }
    }

    /// Arah scan otomatis.
    ///
    /// Prioritas: bila siswa belum absen masuk dan masih dalam jendela masuk,
    /// scan dianggap masuk. Selain itu, bila sudah masuk dan sudah masuk
    /// jendela pulang, dianggap pulang.
    pub fn auto_direction(
        &self,
        local: NaiveTime,
        already_checked_in: bool,
    ) -> Option<AutoDirection> {
        if !already_checked_in && self.in_check_in_window(local) {
            return Some(AutoDirection::CheckIn);
        }
        if already_checked_in && self.in_check_out_window(local) {
            return Some(AutoDirection::CheckOut);
        }
        if !already_checked_in && local > self.check_in_closes_at && self.in_check_out_window(local) {
            // Siswa datang sangat terlambat setelah gerbang absen masuk tutup.
            // Tetap catat sebagai masuk (status alfa/terlambat) daripada
            // kehilangan datanya sama sekali.
            return Some(AutoDirection::CheckIn);
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoDirection {
    CheckIn,
    CheckOut,
}

/// Ambil aturan yang berlaku: prioritas aturan kelas, lalu aturan sekolah,
/// lalu bawaan sistem.
pub async fn resolve_rule(
    pool: &PgPool,
    school_id: Uuid,
    classroom_id: Option<Uuid>,
    date: NaiveDate,
) -> ApiResult<EffectiveRule> {
    type Row = (
        Uuid,
        Uuid,
        Option<Uuid>,
        NaiveTime,
        NaiveTime,
        NaiveTime,
        NaiveTime,
        NaiveTime,
        NaiveTime,
        i16,
        i16,
        bool,
    );

    let row: Option<Row> = sqlx::query_as(
        r#"
        SELECT id, school_id, classroom_id,
               check_in_opens_at, check_in_start_at, check_in_due_at, check_in_closes_at,
               check_out_opens_at, check_out_closes_at,
               late_grace_minutes, active_weekdays, require_check_out
        FROM attendance_rules
        WHERE school_id = $1
          AND is_active
          AND effective_from <= $3
          AND (effective_to IS NULL OR effective_to >= $3)
          AND (classroom_id = $2 OR classroom_id IS NULL)
        -- Aturan spesifik kelas menang atas aturan seluruh sekolah.
        ORDER BY (classroom_id IS NOT NULL) DESC, effective_from DESC
        LIMIT 1
        "#,
    )
    .bind(school_id)
    .bind(classroom_id)
    .bind(date)
    .fetch_optional(pool)
    .await?;

    Ok(match row {
        Some((
            id,
            school_id,
            classroom_id,
            check_in_opens_at,
            check_in_start_at,
            check_in_due_at,
            check_in_closes_at,
            check_out_opens_at,
            check_out_closes_at,
            late_grace_minutes,
            active_weekdays,
            require_check_out,
        )) => EffectiveRule {
            id: Some(id),
            school_id,
            classroom_id,
            check_in_opens_at,
            check_in_start_at,
            check_in_due_at,
            check_in_closes_at,
            check_out_opens_at,
            check_out_closes_at,
            late_grace_minutes,
            active_weekdays,
            require_check_out,
        },
        None => EffectiveRule::default_for(school_id),
    })
}

/// Nama hari libur bila tanggal tersebut libur untuk sekolah ini.
pub async fn holiday_name(
    pool: &PgPool,
    school_id: Uuid,
    date: NaiveDate,
) -> ApiResult<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT name FROM holidays
        WHERE holiday_date = $1
          AND (school_id = $2 OR school_id IS NULL)
        ORDER BY (school_id IS NOT NULL) DESC
        LIMIT 1
        "#,
    )
    .bind(date)
    .bind(school_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}

/// Jam lokal (WIB) dari sebuah instant.
pub fn local_time(ts: DateTime<Utc>) -> NaiveTime {
    util::to_wib(ts).time()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    fn rule() -> EffectiveRule {
        EffectiveRule::default_for(Uuid::new_v4())
    }

    #[test]
    fn datang_sebelum_batas_dianggap_hadir() {
        let r = rule();
        let (s, late) = r.classify_check_in(t(6, 52));
        assert_eq!(s, AttendanceStatus::Hadir);
        assert_eq!(late, 0);
    }

    #[test]
    fn tepat_di_batas_masih_hadir() {
        let r = rule();
        let (s, late) = r.classify_check_in(t(7, 15));
        assert_eq!(s, AttendanceStatus::Hadir, "07:15 tepat batas harus tetap hadir");
        assert_eq!(late, 0);
    }

    #[test]
    fn satu_menit_lewat_batas_terlambat() {
        let r = rule();
        let (s, late) = r.classify_check_in(t(7, 16));
        assert_eq!(s, AttendanceStatus::Terlambat);
        assert_eq!(late, 1);
    }

    #[test]
    fn toleransi_menunda_status_terlambat_tapi_menit_tetap_jujur() {
        let mut r = rule();
        r.late_grace_minutes = 10;

        // Dalam toleransi -> masih hadir.
        let (s, late) = r.classify_check_in(t(7, 20));
        assert_eq!(s, AttendanceStatus::Hadir);
        assert_eq!(late, 0);

        // Lewat toleransi -> terlambat, dihitung dari 07:15 bukan 07:25.
        let (s, late) = r.classify_check_in(t(7, 30));
        assert_eq!(s, AttendanceStatus::Terlambat);
        assert_eq!(late, 15);
    }

    #[test]
    fn setelah_gerbang_tutup_dianggap_alfa() {
        let r = rule();
        let (s, late) = r.classify_check_in(t(9, 30));
        assert_eq!(s, AttendanceStatus::Alfa);
        assert_eq!(late, 0);
    }

    #[test]
    fn jendela_absen_masuk() {
        let r = rule();
        assert!(!r.in_check_in_window(t(5, 0)));
        assert!(r.in_check_in_window(t(5, 30)));
        assert!(r.in_check_in_window(t(9, 0)));
        assert!(!r.in_check_in_window(t(9, 1)));
    }

    #[test]
    fn arah_otomatis_pagi_adalah_masuk() {
        let r = rule();
        assert_eq!(r.auto_direction(t(6, 45), false), Some(AutoDirection::CheckIn));
    }

    #[test]
    fn arah_otomatis_siang_setelah_masuk_adalah_pulang() {
        let r = rule();
        assert_eq!(r.auto_direction(t(13, 10), true), Some(AutoDirection::CheckOut));
    }

    #[test]
    fn scan_kedua_di_pagi_hari_tidak_menghasilkan_arah() {
        let r = rule();
        // Sudah absen masuk, tapi belum jam pulang -> tidak ada aksi.
        assert_eq!(r.auto_direction(t(7, 30), true), None);
    }

    #[test]
    fn siswa_sangat_terlambat_tetap_dicatat_masuk() {
        let r = rule();
        // 12:30, gerbang masuk sudah tutup (09:00) dan belum absen.
        assert_eq!(r.auto_direction(t(12, 30), false), Some(AutoDirection::CheckIn));
    }

    #[test]
    fn di_luar_semua_jendela_tidak_ada_arah() {
        let r = rule();
        assert_eq!(r.auto_direction(t(4, 0), false), None);
        assert_eq!(r.auto_direction(t(22, 0), true), None);
    }

    #[test]
    fn hari_aktif_default_senin_jumat() {
        let r = rule();
        let senin = NaiveDate::from_ymd_opt(2026, 8, 17).unwrap();
        let sabtu = NaiveDate::from_ymd_opt(2026, 8, 22).unwrap();
        let minggu = NaiveDate::from_ymd_opt(2026, 8, 23).unwrap();
        assert!(r.is_active_day(senin));
        assert!(!r.is_active_day(sabtu));
        assert!(!r.is_active_day(minggu));
    }

    #[test]
    fn sekolah_dengan_hari_sabtu_aktif() {
        let mut r = rule();
        r.active_weekdays = 63; // Senin-Sabtu
        let sabtu = NaiveDate::from_ymd_opt(2026, 8, 22).unwrap();
        assert!(r.is_active_day(sabtu));
    }

    #[test]
    fn jam_lokal_dikonversi_dari_utc() {
        // 15 Agustus 2026 00:00 UTC = 07:00 WIB.
        let utc = "2026-08-15T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
        assert_eq!(local_time(utc), t(7, 0));
    }
}
