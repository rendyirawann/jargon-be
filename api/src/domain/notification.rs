//! DTO notifikasi ke wali murid.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

pub const CHANNELS: [&str; 3] = ["whatsapp", "telegram", "email"];
pub const TEMPLATE_KEYS: [&str; 9] = [
    "check_in",
    "check_out",
    "late",
    "absent",
    "sick",
    "permit",
    "daily_recap",
    "weekly_recap",
    "custom",
];

/// Placeholder yang boleh dipakai di badan template.
pub const TEMPLATE_VARIABLES: [&str; 10] = [
    "nama_siswa",
    "nis",
    "kelas",
    "sekolah",
    "tanggal",
    "jam_masuk",
    "jam_pulang",
    "status",
    "menit_terlambat",
    "nama_wali",
];

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct NotificationTemplate {
    pub id: Uuid,
    /// `null` = template bawaan sistem (dipakai bila sekolah belum punya).
    pub school_id: Option<Uuid>,
    pub key: String,
    pub channel: String,
    pub subject: Option<String>,
    pub body: String,
    pub is_active: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpsertTemplateRequest {
    pub school_id: Option<Uuid>,
    pub key: String,
    pub channel: String,
    #[validate(length(max = 200))]
    pub subject: Option<String>,
    #[validate(length(min = 10, max = 4000, message = "isi pesan 10-4000 karakter"))]
    pub body: String,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct OutboxItem {
    pub id: Uuid,
    pub school_id: Uuid,
    pub student_id: Option<Uuid>,
    pub student_name: Option<String>,
    pub channel: String,
    pub template_key: String,
    /// Nomor/alamat tujuan, disamarkan sebagian pada respons.
    pub recipient: String,
    pub subject: Option<String>,
    pub body: String,
    pub status: String,
    pub attempts: i16,
    pub provider: Option<String>,
    pub last_error: Option<String>,
    pub scheduled_at: DateTime<Utc>,
    pub sent_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OutboxFilter {
    pub school_id: Option<Uuid>,
    pub student_id: Option<Uuid>,
    /// queued / sending / sent / failed / cancelled.
    pub status: Option<String>,
    pub channel: Option<String>,
}

/// Kirim pesan bebas ke wali murid dari halaman monitoring.
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct SendMessageRequest {
    #[validate(length(min = 1, max = 500, message = "pilih 1 sampai 500 siswa"))]
    pub student_ids: Vec<Uuid>,
    /// Kosongkan untuk memakai kanal pilihan masing-masing wali.
    pub channel: Option<String>,
    #[validate(length(max = 200))]
    pub subject: Option<String>,
    #[validate(length(min = 5, max = 4000, message = "isi pesan 5-4000 karakter"))]
    pub body: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SendMessageResponse {
    pub queued: usize,
    /// Siswa yang dilewati beserta alasannya (mis. wali belum punya nomor).
    pub skipped: Vec<SkippedRecipient>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SkippedRecipient {
    pub student_id: Uuid,
    pub student_name: String,
    pub reason: String,
}

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct NotificationPolicy {
    pub school_id: Uuid,
    pub notify_on_check_in: bool,
    pub notify_on_check_out: bool,
    pub notify_on_late: bool,
    pub notify_on_absent: bool,
    pub absent_notify_after: chrono::NaiveTime,
    pub quiet_hours_start: Option<chrono::NaiveTime>,
    pub quiet_hours_end: Option<chrono::NaiveTime>,
    pub daily_recap_at: Option<chrono::NaiveTime>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdatePolicyRequest {
    pub notify_on_check_in: Option<bool>,
    pub notify_on_check_out: Option<bool>,
    pub notify_on_late: Option<bool>,
    pub notify_on_absent: Option<bool>,
    pub absent_notify_after: Option<chrono::NaiveTime>,
    pub quiet_hours_start: Option<chrono::NaiveTime>,
    pub quiet_hours_end: Option<chrono::NaiveTime>,
    pub daily_recap_at: Option<chrono::NaiveTime>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NotificationStats {
    pub queued: i64,
    pub sent_today: i64,
    pub failed_today: i64,
    pub by_channel: Vec<ChannelStat>,
}

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct ChannelStat {
    pub channel: String,
    pub sent: i64,
    pub failed: i64,
    pub queued: i64,
}

/// Samarkan tujuan pada respons API supaya daftar log tidak menjadi
/// sumber kebocoran nomor telepon orang tua.
pub fn mask_recipient(recipient: &str) -> String {
    if let Some((local, domain)) = recipient.split_once('@') {
        // Email: b***@domain.com
        let head: String = local.chars().take(1).collect();
        return format!("{head}***@{domain}");
    }
    let digits: Vec<char> = recipient.chars().collect();
    if digits.len() <= 6 {
        return "*".repeat(digits.len());
    }
    let head: String = digits.iter().take(4).collect();
    let tail: String = digits.iter().skip(digits.len() - 3).collect();
    format!("{head}{}{tail}", "*".repeat(digits.len() - 7))
}

/// Isi placeholder `{{nama}}` pada template.
///
/// Placeholder yang tidak dikenal DIBIARKAN utuh, bukan dihapus: pesan yang
/// masih menampilkan `{{kelas}}` langsung terlihat salah oleh operator,
/// sementara pesan yang kehilangan potongan kata sulit disadari.
pub fn render_template(body: &str, vars: &[(&str, String)]) -> String {
    let mut out = body.to_string();
    for (key, value) in vars {
        out = out.replace(&format!("{{{{{key}}}}}"), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nomor_wa_disamarkan() {
        assert_eq!(mask_recipient("628123456789"), "6281*****789");
        assert_eq!(mask_recipient("12345"), "*****");
    }

    #[test]
    fn email_disamarkan() {
        assert_eq!(mask_recipient("budi@gmail.com"), "b***@gmail.com");
    }

    #[test]
    fn render_mengganti_placeholder() {
        let body = "Ananda {{nama_siswa}} kelas {{kelas}} hadir pukul {{jam_masuk}}.";
        let out = render_template(
            body,
            &[
                ("nama_siswa", "Budi Santoso".to_string()),
                ("kelas", "X IPA 1".to_string()),
                ("jam_masuk", "06:52".to_string()),
            ],
        );
        assert_eq!(out, "Ananda Budi Santoso kelas X IPA 1 hadir pukul 06:52.");
    }

    #[test]
    fn placeholder_tak_dikenal_dibiarkan() {
        let out = render_template("Hai {{tidak_ada}}", &[("nama_siswa", "Budi".into())]);
        assert_eq!(out, "Hai {{tidak_ada}}");
    }

    #[test]
    fn render_mengganti_semua_kemunculan() {
        let out = render_template(
            "{{nama_siswa}} dan {{nama_siswa}}",
            &[("nama_siswa", "Budi".into())],
        );
        assert_eq!(out, "Budi dan Budi");
    }

    #[test]
    fn pesan_terlalu_panjang_ditolak() {
        let r = SendMessageRequest {
            student_ids: vec![Uuid::new_v4()],
            channel: None,
            subject: None,
            body: "x".repeat(5000),
        };
        assert!(r.validate().is_err());
    }

    #[test]
    fn daftar_siswa_kosong_ditolak() {
        let r = SendMessageRequest {
            student_ids: vec![],
            channel: None,
            subject: None,
            body: "Halo Bapak/Ibu".into(),
        };
        assert!(r.validate().is_err());
    }
}
