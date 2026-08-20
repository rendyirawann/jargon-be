//! Pengiriman notifikasi ke wali murid.
//!
//! Dua tahap yang sengaja dipisah:
//!
//! 1. **Enqueue** ([`enqueue_attendance`]) — dijalankan di dalam transaksi
//!    yang sama dengan pencatatan absensi. Kalau transaksi batal, pesan pun
//!    tidak pernah ada. Kalau transaksi sukses, pesan PASTI tercatat.
//! 2. **Dispatch** ([`dispatch`]) — dijalankan worker terpisah. Provider WA
//!    yang lambat atau down hanya memperlambat notifikasi, tidak pernah
//!    membuat siswa gagal absen.

pub mod email;
pub mod telegram;
pub mod whatsapp;

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::domain::attendance::AttendanceStatus;
use crate::domain::notification::render_template;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::util;

/// Konteks yang dibutuhkan untuk merangkai pesan absensi.
#[derive(Debug, Clone)]
pub struct AttendanceNotifyContext {
    pub school_id: Uuid,
    pub school_name: String,
    pub student_id: Uuid,
    pub student_name: String,
    pub student_nis: Option<String>,
    pub classroom_name: Option<String>,
    pub attendance_id: Uuid,
    pub attendance_date: NaiveDate,
    pub status: AttendanceStatus,
    pub check_in_at: Option<DateTime<Utc>>,
    pub check_out_at: Option<DateTime<Utc>>,
    pub late_minutes: i32,
}

/// Kejadian yang memicu notifikasi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyEvent {
    CheckIn,
    CheckOut,
    Absent,
    ManualCorrection,
}

impl NotifyEvent {
    fn template_key(self, status: AttendanceStatus) -> &'static str {
        match self {
            NotifyEvent::CheckIn | NotifyEvent::ManualCorrection => status.notification_key(),
            NotifyEvent::CheckOut => "check_out",
            NotifyEvent::Absent => "absent",
        }
    }
}

#[derive(Debug, Default, sqlx::FromRow)]
struct PolicyRow {
    notify_on_check_in: bool,
    notify_on_check_out: bool,
    notify_on_late: bool,
    notify_on_absent: bool,
}

impl PolicyRow {
    fn permissive() -> Self {
        Self {
            notify_on_check_in: true,
            notify_on_check_out: false,
            notify_on_late: true,
            notify_on_absent: true,
        }
    }

    fn allows(&self, event: NotifyEvent, status: AttendanceStatus) -> bool {
        match event {
            NotifyEvent::CheckIn => match status {
                AttendanceStatus::Terlambat => self.notify_on_late,
                _ => self.notify_on_check_in,
            },
            NotifyEvent::CheckOut => self.notify_on_check_out,
            NotifyEvent::Absent => self.notify_on_absent,
            // Koreksi manual selalu dikirim bila operator memintanya.
            NotifyEvent::ManualCorrection => true,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct GuardianTarget {
    id: Uuid,
    full_name: String,
    whatsapp: Option<String>,
    phone: Option<String>,
    email: Option<String>,
    telegram_chat_id: Option<String>,
    preferred_channel: String,
}

impl GuardianTarget {
    /// Alamat tujuan untuk kanal pilihannya. `None` berarti data kontak
    /// belum lengkap — dilaporkan sebagai "skipped", bukan error.
    fn recipient(&self) -> Option<(&'static str, String)> {
        match self.preferred_channel.as_str() {
            "whatsapp" => self
                .whatsapp
                .as_deref()
                .or(self.phone.as_deref())
                .and_then(crate::domain::student::normalize_phone)
                .map(|v| ("whatsapp", v)),
            "telegram" => self
                .telegram_chat_id
                .as_deref()
                .filter(|v| !v.trim().is_empty())
                .map(|v| ("telegram", v.to_string())),
            "email" => self
                .email
                .as_deref()
                .filter(|v| v.contains('@'))
                .map(|v| ("email", v.to_string())),
            _ => None,
        }
    }
}

/// Masukkan notifikasi absensi ke outbox. Mengembalikan jumlah pesan.
///
/// Aman dipanggil di dalam transaksi absensi — hanya melakukan SELECT
/// referensi dan INSERT ke outbox.
pub async fn enqueue_attendance(
    tx: &mut Transaction<'_, Postgres>,
    ctx: &AttendanceNotifyContext,
    event: NotifyEvent,
) -> ApiResult<usize> {
    let policy: Option<PolicyRow> = sqlx::query_as(
        r#"
        SELECT notify_on_check_in, notify_on_check_out, notify_on_late, notify_on_absent
        FROM notification_policies WHERE school_id = $1
        "#,
    )
    .bind(ctx.school_id)
    .fetch_optional(&mut **tx)
    .await?;

    let policy = policy.unwrap_or_else(PolicyRow::permissive);
    if !policy.allows(event, ctx.status) {
        return Ok(0);
    }

    let template_key = event.template_key(ctx.status);

    let guardians: Vec<GuardianTarget> = sqlx::query_as(
        r#"
        SELECT id, full_name, whatsapp, phone, email, telegram_chat_id, preferred_channel
        FROM student_guardians
        WHERE student_id = $1
          AND notify_enabled
          AND preferred_channel <> 'none'
        ORDER BY is_primary DESC, created_at
        "#,
    )
    .bind(ctx.student_id)
    .fetch_all(&mut **tx)
    .await?;

    if guardians.is_empty() {
        return Ok(0);
    }

    let vars = build_variables(ctx);
    let mut queued = 0usize;

    for guardian in &guardians {
        let Some((channel, recipient)) = guardian.recipient() else {
            continue;
        };

        let Some((subject, body)) =
            load_template(tx, ctx.school_id, template_key, channel).await?
        else {
            tracing::warn!(
                school_id = %ctx.school_id, template_key, channel,
                "template notifikasi tidak ditemukan, pesan dilewati"
            );
            continue;
        };

        let mut all_vars = vars.clone();
        all_vars.push(("nama_wali", guardian.full_name.clone()));

        let rendered_body = render_template(&body, &all_vars);
        let rendered_subject = subject.map(|s| render_template(&s, &all_vars));

        sqlx::query(
            r#"
            INSERT INTO notification_outbox
                (school_id, student_id, guardian_id, attendance_id, channel,
                 template_key, recipient, subject, body, variables, status, scheduled_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'queued', NOW())
            "#,
        )
        .bind(ctx.school_id)
        .bind(ctx.student_id)
        .bind(guardian.id)
        .bind(ctx.attendance_id)
        .bind(channel)
        .bind(template_key)
        .bind(&recipient)
        .bind(rendered_subject.as_deref())
        .bind(&rendered_body)
        .bind(serde_json::json!(
            all_vars.iter().cloned().collect::<std::collections::BTreeMap<_, _>>()
        ))
        .execute(&mut **tx)
        .await?;

        queued += 1;
    }

    if queued > 0 {
        sqlx::query(
            r#"
            UPDATE attendances
               SET notification_status = 'queued', notified_at = NOW()
             WHERE attendance_date = $1 AND id = $2
            "#,
        )
        .bind(ctx.attendance_date)
        .bind(ctx.attendance_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(queued)
}

fn build_variables(ctx: &AttendanceNotifyContext) -> Vec<(&'static str, String)> {
    vec![
        ("nama_siswa", ctx.student_name.clone()),
        ("nis", ctx.student_nis.clone().unwrap_or_default()),
        ("kelas", ctx.classroom_name.clone().unwrap_or_else(|| "-".into())),
        ("sekolah", ctx.school_name.clone()),
        ("tanggal", util::format_date_id(ctx.attendance_date)),
        (
            "jam_masuk",
            util::format_time_wib(ctx.check_in_at).unwrap_or_else(|| "-".into()),
        ),
        (
            "jam_pulang",
            util::format_time_wib(ctx.check_out_at).unwrap_or_else(|| "-".into()),
        ),
        ("status", status_label(ctx.status).to_string()),
        ("menit_terlambat", ctx.late_minutes.to_string()),
    ]
}

pub fn status_label(status: AttendanceStatus) -> &'static str {
    match status {
        AttendanceStatus::Hadir => "Hadir",
        AttendanceStatus::Terlambat => "Terlambat",
        AttendanceStatus::Izin => "Izin",
        AttendanceStatus::Sakit => "Sakit",
        AttendanceStatus::Alfa => "Tanpa Keterangan",
        AttendanceStatus::Dispensasi => "Dispensasi",
    }
}

/// Ambil template milik sekolah; jatuh ke template bawaan bila belum ada.
async fn load_template(
    tx: &mut Transaction<'_, Postgres>,
    school_id: Uuid,
    key: &str,
    channel: &str,
) -> ApiResult<Option<(Option<String>, String)>> {
    let row: Option<(Option<String>, String)> = sqlx::query_as(
        r#"
        SELECT subject, body
        FROM notification_templates
        WHERE key = $2 AND channel = $3 AND is_active
          AND (school_id = $1 OR school_id IS NULL)
        ORDER BY (school_id IS NOT NULL) DESC
        LIMIT 1
        "#,
    )
    .bind(school_id)
    .bind(key)
    .bind(channel)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row)
}

// =====================================================================
// Pengiriman
// =====================================================================

/// Satu baris outbox yang siap dikirim.
///
/// `school_id` ikut dibawa untuk pemilihan kredensial provider per sekolah
/// (tabel `notification_channels`) dan untuk atribusi pada log.
#[allow(dead_code)]
#[derive(Debug, sqlx::FromRow)]
pub struct PendingMessage {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub school_id: Uuid,
    pub channel: String,
    pub recipient: String,
    pub subject: Option<String>,
    pub body: String,
    pub attempts: i16,
    pub max_attempts: i16,
}

#[derive(Debug)]
pub struct DispatchResult {
    pub provider: String,
    pub message_id: Option<String>,
}

/// Kirim satu pesan lewat kanalnya.
pub async fn dispatch(state: &AppState, msg: &PendingMessage) -> ApiResult<DispatchResult> {
    match msg.channel.as_str() {
        "whatsapp" => whatsapp::send(state, &msg.recipient, &msg.body).await,
        "telegram" => telegram::send(state, &msg.recipient, &msg.body).await,
        "email" => {
            email::send(
                state,
                &msg.recipient,
                msg.subject.as_deref().unwrap_or("Notifikasi Absensi"),
                &msg.body,
            )
            .await
        }
        other => Err(ApiError::BadRequest(format!(
            "kanal notifikasi `{other}` tidak dikenal"
        ))),
    }
}

/// Jeda sebelum percobaan ulang: 1, 2, 4, 8, 16 menit (backoff eksponensial).
pub fn retry_delay_seconds(attempts: i16) -> i64 {
    let exp = attempts.clamp(0, 6) as u32;
    60i64 * 2i64.pow(exp)
}

/// Buat baris kebijakan default bila sekolah belum punya.
pub async fn ensure_policy(pool: &PgPool, school_id: Uuid) -> ApiResult<()> {
    sqlx::query(
        r#"
        INSERT INTO notification_policies (school_id) VALUES ($1)
        ON CONFLICT (school_id) DO NOTHING
        "#,
    )
    .bind(school_id)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kebijakan_default_mengizinkan_masuk_dan_alfa() {
        let p = PolicyRow::permissive();
        assert!(p.allows(NotifyEvent::CheckIn, AttendanceStatus::Hadir));
        assert!(p.allows(NotifyEvent::CheckIn, AttendanceStatus::Terlambat));
        assert!(p.allows(NotifyEvent::Absent, AttendanceStatus::Alfa));
        // Notifikasi pulang default nonaktif agar orang tua tidak dibanjiri.
        assert!(!p.allows(NotifyEvent::CheckOut, AttendanceStatus::Hadir));
    }

    #[test]
    fn kebijakan_terlambat_bisa_dimatikan_terpisah() {
        let p = PolicyRow {
            notify_on_check_in: true,
            notify_on_check_out: false,
            notify_on_late: false,
            notify_on_absent: true,
        };
        assert!(p.allows(NotifyEvent::CheckIn, AttendanceStatus::Hadir));
        assert!(!p.allows(NotifyEvent::CheckIn, AttendanceStatus::Terlambat));
    }

    #[test]
    fn koreksi_manual_selalu_diizinkan() {
        let p = PolicyRow {
            notify_on_check_in: false,
            notify_on_check_out: false,
            notify_on_late: false,
            notify_on_absent: false,
        };
        assert!(p.allows(NotifyEvent::ManualCorrection, AttendanceStatus::Izin));
    }

    #[test]
    fn kunci_template_mengikuti_status() {
        assert_eq!(
            NotifyEvent::CheckIn.template_key(AttendanceStatus::Terlambat),
            "late"
        );
        assert_eq!(
            NotifyEvent::CheckIn.template_key(AttendanceStatus::Hadir),
            "check_in"
        );
        assert_eq!(
            NotifyEvent::CheckOut.template_key(AttendanceStatus::Hadir),
            "check_out"
        );
        assert_eq!(
            NotifyEvent::Absent.template_key(AttendanceStatus::Alfa),
            "absent"
        );
    }

    fn guardian(channel: &str, wa: Option<&str>, email: Option<&str>, tg: Option<&str>) -> GuardianTarget {
        GuardianTarget {
            id: Uuid::new_v4(),
            full_name: "Ibu Sri".into(),
            whatsapp: wa.map(String::from),
            phone: None,
            email: email.map(String::from),
            telegram_chat_id: tg.map(String::from),
            preferred_channel: channel.into(),
        }
    }

    #[test]
    fn tujuan_wa_dinormalisasi() {
        let g = guardian("whatsapp", Some("0812-3456-789"), None, None);
        assert_eq!(g.recipient(), Some(("whatsapp", "628123456789".to_string())));
    }

    #[test]
    fn wali_tanpa_kontak_dilewati() {
        assert!(guardian("whatsapp", None, None, None).recipient().is_none());
        assert!(guardian("email", None, None, None).recipient().is_none());
        assert!(guardian("telegram", None, None, Some("  ")).recipient().is_none());
        assert!(guardian("none", Some("08123456789"), None, None).recipient().is_none());
    }

    #[test]
    fn email_tanpa_at_ditolak() {
        assert!(guardian("email", None, Some("bukan-email"), None).recipient().is_none());
        assert!(guardian("email", None, Some("a@b.id"), None).recipient().is_some());
    }

    #[test]
    fn backoff_naik_eksponensial() {
        assert_eq!(retry_delay_seconds(0), 60);
        assert_eq!(retry_delay_seconds(1), 120);
        assert_eq!(retry_delay_seconds(2), 240);
        assert_eq!(retry_delay_seconds(3), 480);
        // Dibatasi agar tidak meledak.
        assert_eq!(retry_delay_seconds(50), 60 * 64);
    }

    #[test]
    fn variabel_template_lengkap() {
        let ctx = AttendanceNotifyContext {
            school_id: Uuid::new_v4(),
            school_name: "SMA Negeri 1 Medan".into(),
            student_id: Uuid::new_v4(),
            student_name: "Budi Santoso".into(),
            student_nis: Some("12345".into()),
            classroom_name: Some("X IPA 1".into()),
            attendance_id: Uuid::new_v4(),
            attendance_date: NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
            status: AttendanceStatus::Terlambat,
            check_in_at: Some("2026-08-14T00:20:00Z".parse().unwrap()),
            check_out_at: None,
            late_minutes: 5,
        };
        let vars = build_variables(&ctx);
        let map: std::collections::HashMap<_, _> = vars.into_iter().collect();

        assert_eq!(map["nama_siswa"], "Budi Santoso");
        assert_eq!(map["kelas"], "X IPA 1");
        assert_eq!(map["tanggal"], "14 Agustus 2026");
        // 00:20 UTC = 07:20 WIB.
        assert_eq!(map["jam_masuk"], "07:20");
        assert_eq!(map["jam_pulang"], "-");
        assert_eq!(map["status"], "Terlambat");
        assert_eq!(map["menit_terlambat"], "5");
    }
}
