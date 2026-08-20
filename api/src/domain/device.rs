//! DTO perangkat tablet.
//!
//! Alur pemasangan di lapangan:
//!   1. Operator sekolah membuat perangkat di /admin -> dapat **kode pairing**
//!      8 digit yang berlaku 30 menit.
//!   2. Petugas memasukkan kode itu di tablet.
//!   3. Tablet menukar kode dengan **device token** permanen + secret HMAC.
//!      Kode pairing langsung hangus.
//!
//! Dengan begitu token panjang tidak perlu diketik manual, dan kode yang
//! bocor pun tidak berguna setelah 30 menit atau setelah dipakai sekali.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

pub const DEVICE_PLACEMENTS: [&str; 4] = ["gate", "classroom", "office", "mobile"];
pub const DEVICE_MODES: [&str; 4] = ["auto", "check_in", "check_out", "enroll"];

/// Masa berlaku kode pairing.
pub const PAIRING_TTL_MINUTES: i64 = 30;
/// Perangkat dianggap offline bila tidak ada heartbeat selama ini.
///
/// Nilai yang sama tertulis sebagai `INTERVAL '10 minutes'` pada query status
/// perangkat; konstanta ini menjadi rujukan tunggal saat nilainya diubah.
#[allow(dead_code)]
pub const OFFLINE_AFTER_MINUTES: i64 = 10;

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct Device {
    pub id: Uuid,
    pub school_id: Uuid,
    pub school_name: String,
    pub code: String,
    pub name: String,
    /// gate / classroom / office / mobile.
    pub placement: String,
    pub classroom_id: Option<Uuid>,
    pub classroom_name: Option<String>,
    /// auto / check_in / check_out / enroll.
    pub mode: String,
    /// `true` bila perangkat sudah pernah menukar kode pairing.
    pub is_paired: bool,
    pub app_version: Option<String>,
    pub os_version: Option<String>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub last_ip: Option<String>,
    /// Dihitung dari `last_seen_at`.
    pub is_online: bool,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateDeviceRequest {
    pub school_id: Option<Uuid>,
    /// Kode unik perangkat, mis. `MDN-SMAN1-GATE-01`.
    #[validate(length(min = 3, max = 40, message = "kode perangkat 3-40 karakter"))]
    pub code: String,
    #[validate(length(min = 3, max = 120))]
    pub name: String,
    #[serde(default = "default_placement")]
    pub placement: String,
    pub classroom_id: Option<Uuid>,
    #[serde(default = "default_mode")]
    pub mode: String,
}

fn default_placement() -> String {
    "gate".to_string()
}
fn default_mode() -> String {
    "auto".to_string()
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateDeviceRequest {
    #[validate(length(min = 3, max = 120))]
    pub name: Option<String>,
    pub placement: Option<String>,
    pub classroom_id: Option<Uuid>,
    pub mode: Option<String>,
    pub is_active: Option<bool>,
}

/// Respons setelah membuat perangkat atau membuat ulang kode pairing.
#[derive(Debug, Serialize, ToSchema)]
pub struct PairingCodeResponse {
    pub device_id: Uuid,
    pub code: String,
    /// Kode 8 digit yang dimasukkan di tablet.
    pub pairing_code: String,
    pub expires_at: DateTime<Utc>,
}

/// Dikirim oleh tablet, tanpa autentikasi (kode pairing adalah kredensialnya).
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct PairDeviceRequest {
    #[validate(length(min = 8, max = 8, message = "kode pairing harus 8 digit"))]
    pub pairing_code: String,
    #[validate(length(max = 30))]
    pub app_version: Option<String>,
    #[validate(length(max = 60))]
    pub os_version: Option<String>,
    /// Pengenal perangkat dari sistem operasi, untuk jejak audit.
    pub hardware_id: Option<String>,
}

/// Diberikan SEKALI. Token tidak bisa dibaca ulang; bila hilang, buat
/// kode pairing baru.
#[derive(Debug, Serialize, ToSchema)]
pub struct PairDeviceResponse {
    pub device_id: Uuid,
    pub device_code: String,
    pub device_name: String,
    pub school_id: Uuid,
    pub school_name: String,
    pub classroom_id: Option<Uuid>,
    pub classroom_name: Option<String>,
    pub mode: String,
    pub placement: String,
    /// Dipakai sebagai `Authorization: Device <token>`.
    pub device_token: String,
    /// Kunci HMAC (hex) untuk menandatangani payload absensi.
    pub hmac_secret: String,
    /// Konfigurasi yang perlu diketahui tablet agar konsisten dengan server.
    pub config: DeviceRuntimeConfig,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DeviceRuntimeConfig {
    pub embedding_dim: usize,
    pub model_version: String,
    pub match_threshold: f32,
    pub min_liveness: f32,
    /// Jeda minimum (detik) antar scan siswa yang sama.
    pub scan_cooldown_seconds: u64,
    /// Interval heartbeat yang diharapkan (detik).
    pub heartbeat_interval_seconds: u64,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct DeviceHeartbeatRequest {
    #[validate(range(min = 0, max = 100))]
    pub battery_pct: Option<i16>,
    /// Jumlah scan yang masih menunggu terkirim dari penyimpanan lokal.
    #[validate(range(min = 0))]
    pub queued_events: Option<i32>,
    pub app_version: Option<String>,
    pub network: Option<String>,
    pub embedding_model_version: Option<String>,
}

/// Balasan heartbeat: sekaligus kanal perintah ringan dari server ke tablet.
#[derive(Debug, Serialize, ToSchema)]
pub struct DeviceHeartbeatResponse {
    pub server_time: DateTime<Utc>,
    pub config: DeviceRuntimeConfig,
    /// Naik setiap kali data wajah sekolah berubah. Bila berbeda dari nilai
    /// yang dipegang tablet, tablet menyegarkan cache lokalnya.
    pub roster_version: i64,
    /// Perintah opsional: `reload_roster`, `reboot_app`, `revoke`.
    pub commands: Vec<String>,
    /// Jendela absen hari ini, agar tablet bisa menampilkan status walau offline.
    pub today_windows: Option<TodayWindows>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TodayWindows {
    pub is_active_day: bool,
    pub is_holiday: bool,
    pub holiday_name: Option<String>,
    pub check_in_opens_at: Option<String>,
    pub check_in_due_at: Option<String>,
    pub check_in_closes_at: Option<String>,
    pub check_out_opens_at: Option<String>,
    pub check_out_closes_at: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DeviceFilter {
    pub school_id: Option<Uuid>,
    pub placement: Option<String>,
    /// `true` = hanya perangkat yang sedang online.
    pub online: Option<bool>,
    pub is_active: Option<bool>,
}

/// Daftar siswa ringkas untuk cache offline di tablet.
///
/// TIDAK berisi embedding maupun gambar — hanya identitas untuk ditampilkan
/// di layar setelah pencocokan berhasil dilakukan oleh server. Saat mode
/// offline, tablet menyimpan hasil scan lokal dan menyinkronkannya nanti.
#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct RosterEntry {
    pub student_id: Uuid,
    pub full_name: String,
    pub nis: Option<String>,
    pub classroom_id: Option<Uuid>,
    pub classroom_name: Option<String>,
    pub face_enrolled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kode_pairing_harus_delapan_digit() {
        let r = PairDeviceRequest {
            pairing_code: "1234".into(),
            app_version: None,
            os_version: None,
            hardware_id: None,
        };
        assert!(r.validate().is_err());

        let r = PairDeviceRequest {
            pairing_code: "12345678".into(),
            app_version: None,
            os_version: None,
            hardware_id: None,
        };
        assert!(r.validate().is_ok());
    }

    #[test]
    fn kode_perangkat_divalidasi() {
        let r = CreateDeviceRequest {
            school_id: None,
            code: "ab".into(),
            name: "Tablet".into(),
            placement: "gate".into(),
            mode: "auto".into(),
            classroom_id: None,
        };
        assert!(r.validate().is_err());
    }

    #[test]
    fn baterai_di_luar_rentang_ditolak() {
        let r = DeviceHeartbeatRequest {
            battery_pct: Some(150),
            queued_events: None,
            app_version: None,
            network: None,
            embedding_model_version: None,
        };
        assert!(r.validate().is_err());
    }
}
