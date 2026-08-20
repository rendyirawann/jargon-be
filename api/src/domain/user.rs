//! DTO autentikasi & manajemen pengguna (guru, staff, kepala sekolah, dinas).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

pub const ASSIGNABLE_ROLES: [&str; 8] = [
    "superadmin",
    "admin_dinas",
    "petugas_pengaduan",
    "kepala_sekolah",
    "guru",
    "staff_tu",
    "siswa",
    "orang_tua",
];

/// Peran yang WAJIB terikat pada satu sekolah.
pub const SCHOOL_BOUND_ROLES: [&str; 4] = ["kepala_sekolah", "guru", "staff_tu", "siswa"];

/// Peran yang cakupannya ditentukan oleh tautan ke siswa, bukan oleh sekolah.
///
/// `orang_tua` sengaja TIDAK terikat sekolah: anak-anaknya bisa berada di
/// sekolah yang berbeda, dan mengikat akun ke satu sekolah justru akan
/// memberinya akses ke seluruh siswa sekolah itu.
pub const STUDENT_LINKED_ROLES: [&str; 2] = ["siswa", "orang_tua"];

/// Peran yang login memakai NISN. Sisanya memakai NIK.
pub const NISN_ROLES: [&str; 1] = ["siswa"];

// =====================================================================
// Login
// =====================================================================

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct LoginRequest {
    /// Identitas login.
    ///
    /// Di aplikasi Jargon GO: **NISN** (10 digit) untuk siswa, **NIK**
    /// (16 digit) untuk guru, staff, kepala sekolah, orang tua, dan dinas.
    /// Dashboard `/admin` juga menerima username atau email.
    #[validate(length(min = 3, max = 150, message = "NIK/NISN atau username wajib diisi"))]
    pub identifier: String,
    #[validate(length(min = 6, message = "kata sandi minimal 6 karakter"))]
    pub password: String,
    /// Nama perangkat untuk daftar sesi (mis. "Redmi Note 12 - Pak Budi").
    pub device_name: Option<String>,
}

impl LoginRequest {
    /// Bentuk identitas yang dikirim, untuk pesan galat yang lebih membantu.
    ///
    /// Angka 10 digit hampir pasti NISN dan 16 digit hampir pasti NIK;
    /// mengetahui ini membuat pesan "NISN tidak terdaftar" jauh lebih berguna
    /// bagi siswa daripada "kredensial salah".
    pub fn identity_kind(&self) -> IdentityKind {
        let value = self.identifier.trim();
        if value.chars().all(|c| c.is_ascii_digit()) {
            match value.len() {
                10 => IdentityKind::Nisn,
                16 => IdentityKind::Nik,
                _ => IdentityKind::Other,
            }
        } else {
            IdentityKind::Other
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityKind {
    Nik,
    Nisn,
    /// Username atau email (jalur dashboard).
    Other,
}

impl IdentityKind {
    pub fn label(self) -> &'static str {
        match self {
            IdentityKind::Nik => "NIK",
            IdentityKind::Nisn => "NISN",
            IdentityKind::Other => "Akun",
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    /// Unix timestamp kedaluwarsa access token.
    pub expires_at: i64,
    pub user: UserProfile,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct RefreshRequest {
    #[validate(length(min = 20, message = "refresh token tidak valid"))]
    pub refresh_token: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserProfile {
    pub id: Uuid,
    pub name: String,
    pub username: String,
    pub email: String,
    /// NIK atau NISN.
    pub identity_number: Option<String>,
    /// `nik` atau `nisn`.
    pub identity_type: Option<String>,
    pub avatar_url: Option<String>,
    pub phone: Option<String>,
    pub position: Option<String>,
    pub employee_no: Option<String>,
    /// `null` untuk Superadmin / Admin Dinas / orang tua.
    pub school_id: Option<Uuid>,
    pub school_name: Option<String>,
    pub roles: Vec<String>,
    pub role_label: String,
    pub permissions: Vec<String>,
    /// Kelas yang menjadi tanggung jawabnya sebagai wali kelas.
    pub homeroom_classrooms: Vec<HomeroomRef>,
    /// Siswa yang terkait dengan akun ini.
    ///
    /// Untuk akun `siswa` berisi satu entri (dirinya). Untuk `orang_tua`
    /// berisi anak-anaknya, bisa lintas sekolah. Kosong untuk peran lain.
    pub students: Vec<LinkedStudent>,
    pub must_change_password: bool,
    pub last_login: Option<DateTime<Utc>>,
}

/// Siswa yang tertaut ke sebuah akun aplikasi.
#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct LinkedStudent {
    pub id: Uuid,
    pub full_name: String,
    pub nisn: Option<String>,
    pub nis: Option<String>,
    pub school_id: Uuid,
    pub school_name: String,
    pub classroom_name: Option<String>,
    /// Hubungan pemilik akun dengan siswa ini: `diri_sendiri`, `ayah`,
    /// `ibu`, atau `wali`.
    pub relation: String,
}

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct HomeroomRef {
    pub id: Uuid,
    pub name: String,
    pub grade_level: i16,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    #[validate(length(min = 8, message = "kata sandi baru minimal 8 karakter"))]
    pub new_password: String,
    #[validate(must_match(other = "new_password", message = "konfirmasi kata sandi tidak sama"))]
    pub new_password_confirmation: String,
}

// =====================================================================
// Manajemen pengguna
// =====================================================================

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct UserListItem {
    pub id: Uuid,
    pub name: String,
    pub username: String,
    pub email: String,
    pub phone: Option<String>,
    pub position: Option<String>,
    pub school_id: Option<Uuid>,
    pub school_name: Option<String>,
    pub roles: Vec<String>,
    pub is_active: bool,
    pub is_banned: bool,
    pub last_login: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateUserRequest {
    #[validate(length(min = 3, max = 150))]
    pub name: String,
    #[validate(length(min = 3, max = 50, message = "username 3-50 karakter"))]
    pub username: String,
    #[validate(email(message = "format email tidak valid"))]
    pub email: String,
    #[validate(length(min = 8, message = "kata sandi minimal 8 karakter"))]
    pub password: String,
    /// Identitas login Jargon GO: NIK 16 digit, atau NISN 10 digit untuk siswa.
    pub identity_number: Option<String>,
    /// Siswa yang ditautkan.
    ///
    /// Wajib untuk peran `siswa`. Untuk `orang_tua`, isi dengan id anak-anaknya.
    #[serde(default)]
    pub student_ids: Vec<Uuid>,
    /// Hubungan wali dengan siswa (`ayah`/`ibu`/`wali`), khusus `orang_tua`.
    pub guardian_relation: Option<String>,
    /// Satu dari: superadmin, admin_dinas, petugas_pengaduan, kepala_sekolah,
    /// guru, staff_tu, siswa, orang_tua.
    pub role: String,
    /// Wajib untuk peran tingkat sekolah; harus kosong untuk peran provinsi.
    pub school_id: Option<Uuid>,
    #[validate(length(max = 30))]
    pub employee_no: Option<String>,
    #[validate(length(max = 100))]
    pub position: Option<String>,
    #[validate(length(max = 15))]
    pub phone: Option<String>,
    pub telegram_chat_id: Option<String>,
    /// Sekolah tambahan (untuk pengawas). Hanya dihormati bila diisi oleh
    /// Superadmin.
    #[serde(default)]
    pub extra_school_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateUserRequest {
    #[validate(length(min = 3, max = 150))]
    pub name: Option<String>,
    #[validate(email)]
    pub email: Option<String>,
    pub role: Option<String>,
    pub school_id: Option<Uuid>,
    pub employee_no: Option<String>,
    pub position: Option<String>,
    pub phone: Option<String>,
    pub telegram_chat_id: Option<String>,
    pub is_active: Option<bool>,
    /// Bila diisi, kata sandi di-reset dan pengguna wajib menggantinya
    /// saat login berikutnya.
    #[validate(length(min = 8))]
    pub new_password: Option<String>,
    pub extra_school_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct UserFilter {
    pub school_id: Option<Uuid>,
    pub role: Option<String>,
    pub is_active: Option<bool>,
}

impl CreateUserRequest {
    /// Aturan silang peran vs sekolah.
    ///
    /// Kesalahan konfigurasi di sini berbahaya: guru tanpa `school_id` akan
    /// gagal melihat data apa pun, sedangkan `superadmin` yang terikat satu
    /// sekolah kehilangan kemampuan mengawasi provinsi.
    pub fn validate_role_scope(&self) -> Result<(), crate::error::ApiError> {
        use crate::error::{ApiError, FieldError};

        if !ASSIGNABLE_ROLES.contains(&self.role.as_str()) {
            return Err(ApiError::field(
                "role",
                &format!("peran tidak dikenal, pilih salah satu: {}", ASSIGNABLE_ROLES.join(", ")),
            ));
        }

        let role = self.role.as_str();

        // --- Tautan ke siswa ---------------------------------------
        if STUDENT_LINKED_ROLES.contains(&role) && self.student_ids.is_empty() {
            return Err(ApiError::validation(vec![FieldError::new(
                "student_ids",
                match role {
                    "siswa" => "akun siswa wajib ditautkan ke data siswanya".to_string(),
                    _ => "akun orang tua wajib ditautkan ke minimal satu anak".to_string(),
                },
            )]));
        }
        if role == "siswa" && self.student_ids.len() > 1 {
            return Err(ApiError::field(
                "student_ids",
                "satu akun siswa hanya boleh ditautkan ke satu data siswa",
            ));
        }
        if !STUDENT_LINKED_ROLES.contains(&role) && !self.student_ids.is_empty() {
            return Err(ApiError::field(
                "student_ids",
                "tautan siswa hanya berlaku untuk peran siswa dan orang tua",
            ));
        }

        // --- Identitas login ---------------------------------------
        if let Some(identity) = self.identity_number.as_deref().map(str::trim) {
            if !identity.is_empty() {
                let expected = if NISN_ROLES.contains(&role) { 10 } else { 16 };
                let label = if expected == 10 { "NISN" } else { "NIK" };

                if !identity.chars().all(|c| c.is_ascii_digit())
                    || identity.len() != expected
                {
                    return Err(ApiError::field(
                        "identity_number",
                        &format!("peran `{role}` login memakai {label} ({expected} digit angka)"),
                    ));
                }
            }
        }

        // --- Cakupan sekolah ---------------------------------------
        //
        // `orang_tua` sengaja dikecualikan dari kedua aturan: anaknya bisa
        // berada di sekolah berbeda, jadi akun itu tidak terikat sekolah
        // mana pun — cakupannya sepenuhnya berasal dari student_ids.
        if role == "orang_tua" {
            return Ok(());
        }

        let school_bound = SCHOOL_BOUND_ROLES.contains(&role);
        match (school_bound, self.school_id) {
            (true, None) => Err(ApiError::validation(vec![FieldError::new(
                "school_id",
                format!("peran `{role}` wajib ditautkan ke satu sekolah"),
            )])),
            (false, Some(_)) => Err(ApiError::validation(vec![FieldError::new(
                "school_id",
                format!("peran `{role}` bercakupan provinsi, kosongkan school_id"),
            )])),
            _ => Ok(()),
        }
    }
}

/// Penautan akun orang tua ke seorang siswa.
///
/// Dipisahkan dari `UpdateUserRequest` karena tautan wali bukan atribut akun,
/// melainkan hubungan yang bisa bertambah kapan saja: anak kedua masuk sekolah
/// tahun berikutnya, atau seorang wali menggantikan orang tua yang meninggal.
/// Memaksakannya lewat "perbarui akun" berarti setiap penambahan anak harus
/// mengirim ulang seluruh daftar — dan sekali daftar itu terkirim tidak
/// lengkap, tautan anak yang lain ikut terhapus.
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct LinkChildRequest {
    pub student_id: Uuid,
    /// `ayah`, `ibu`, atau `wali`. Default `wali`.
    pub relation: Option<String>,
}

/// Pembuatan akun siswa secara massal dari data siswa yang sudah ada.
///
/// Untuk 700.000 siswa, membuat akun satu per satu tidak mungkin. Endpoint
/// ini membuat akun untuk seluruh siswa aktif di satu kelas/sekolah dan
/// mengembalikan kata sandi awal SEKALI — tidak pernah bisa dilihat lagi.
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct BulkStudentAccountRequest {
    pub school_id: Option<Uuid>,
    /// Batasi ke satu kelas. Kosongkan untuk seluruh sekolah.
    pub classroom_id: Option<Uuid>,
    /// Lewati siswa yang akunnya sudah ada (default `true`).
    #[serde(default = "default_true_bool")]
    pub skip_existing: bool,
    /// Batas jumlah akun per permintaan.
    #[validate(range(min = 1, max = 1000))]
    pub limit: Option<i64>,
}

fn default_true_bool() -> bool {
    true
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BulkStudentAccountResponse {
    pub created: usize,
    pub skipped: usize,
    /// Kredensial awal, hanya dikembalikan pada respons ini.
    ///
    /// Kata sandi dibuat acak, bukan diturunkan dari NISN atau tanggal lahir:
    /// keduanya tercetak pada dokumen sekolah dan mudah ditebak teman sekelas.
    pub credentials: Vec<InitialCredential>,
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InitialCredential {
    pub student_id: Uuid,
    pub full_name: String,
    pub classroom_name: Option<String>,
    pub nisn: String,
    pub initial_password: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(role: &str, school: Option<Uuid>) -> CreateUserRequest {
        CreateUserRequest {
            name: "Budi Guru".into(),
            username: "budi".into(),
            email: "budi@sekolah.id".into(),
            password: "RahasiaKuat1".into(),
            identity_number: None,
            student_ids: vec![],
            guardian_relation: None,
            role: role.into(),
            school_id: school,
            employee_no: None,
            position: None,
            phone: None,
            telegram_chat_id: None,
            extra_school_ids: vec![],
        }
    }

    #[test]
    fn guru_wajib_punya_sekolah() {
        assert!(req("guru", None).validate_role_scope().is_err());
        assert!(req("guru", Some(Uuid::new_v4())).validate_role_scope().is_ok());
    }

    #[test]
    fn superadmin_tidak_boleh_terikat_sekolah() {
        assert!(req("superadmin", Some(Uuid::new_v4())).validate_role_scope().is_err());
        assert!(req("superadmin", None).validate_role_scope().is_ok());
    }

    #[test]
    fn kepala_sekolah_wajib_punya_sekolah() {
        assert!(req("kepala_sekolah", None).validate_role_scope().is_err());
        assert!(req("kepala_sekolah", Some(Uuid::new_v4())).validate_role_scope().is_ok());
    }

    #[test]
    fn peran_asing_ditolak() {
        assert!(req("dukun", Some(Uuid::new_v4())).validate_role_scope().is_err());
    }

    // -----------------------------------------------------------------
    // Peran baru Jargon GO
    // -----------------------------------------------------------------

    #[test]
    fn akun_siswa_wajib_ditautkan_ke_satu_siswa() {
        let mut r = req("siswa", Some(Uuid::new_v4()));
        assert!(r.validate_role_scope().is_err(), "tanpa tautan harus ditolak");

        r.student_ids = vec![Uuid::new_v4()];
        assert!(r.validate_role_scope().is_ok());

        // Satu akun tidak boleh mewakili dua siswa.
        r.student_ids.push(Uuid::new_v4());
        assert!(r.validate_role_scope().is_err());
    }

    #[test]
    fn akun_orang_tua_tidak_terikat_sekolah_tapi_wajib_punya_anak() {
        let mut r = req("orang_tua", None);
        assert!(r.validate_role_scope().is_err(), "tanpa anak harus ditolak");

        r.student_ids = vec![Uuid::new_v4(), Uuid::new_v4()];
        assert!(
            r.validate_role_scope().is_ok(),
            "orang tua boleh punya anak di sekolah berbeda, jadi school_id kosong itu benar"
        );

        // Bahkan bila school_id diisi, itu tidak dianggap galat — nilai itu
        // sekadar diabaikan karena cakupan berasal dari daftar anak.
        r.school_id = Some(Uuid::new_v4());
        assert!(r.validate_role_scope().is_ok());
    }

    #[test]
    fn peran_selain_siswa_dan_ortu_tidak_boleh_menautkan_siswa() {
        let mut r = req("guru", Some(Uuid::new_v4()));
        r.student_ids = vec![Uuid::new_v4()];
        assert!(r.validate_role_scope().is_err());
    }

    #[test]
    fn siswa_login_pakai_nisn_sepuluh_digit() {
        let mut r = req("siswa", Some(Uuid::new_v4()));
        r.student_ids = vec![Uuid::new_v4()];

        r.identity_number = Some("1275010101900001".into()); // NIK 16 digit
        assert!(r.validate_role_scope().is_err(), "siswa harus memakai NISN");

        r.identity_number = Some("0061234567".into()); // NISN 10 digit
        assert!(r.validate_role_scope().is_ok());
    }

    #[test]
    fn guru_login_pakai_nik_enam_belas_digit() {
        let mut r = req("guru", Some(Uuid::new_v4()));

        r.identity_number = Some("0061234567".into());
        assert!(r.validate_role_scope().is_err(), "guru harus memakai NIK");

        r.identity_number = Some("1275010101900001".into());
        assert!(r.validate_role_scope().is_ok());
    }

    #[test]
    fn identitas_dengan_huruf_ditolak() {
        let mut r = req("guru", Some(Uuid::new_v4()));
        r.identity_number = Some("12750101ABCD0001".into());
        assert!(r.validate_role_scope().is_err());
    }

    #[test]
    fn jenis_identitas_dikenali_dari_panjangnya() {
        let mk = |s: &str| LoginRequest {
            identifier: s.into(),
            password: "rahasia123".into(),
            device_name: None,
        };
        assert_eq!(mk("0061234567").identity_kind(), IdentityKind::Nisn);
        assert_eq!(mk("1275010101900001").identity_kind(), IdentityKind::Nik);
        assert_eq!(mk("superadmin").identity_kind(), IdentityKind::Other);
        assert_eq!(mk("12345").identity_kind(), IdentityKind::Other);
    }

    #[test]
    fn konfirmasi_kata_sandi_harus_sama() {
        let r = ChangePasswordRequest {
            current_password: "lama123".into(),
            new_password: "BaruKuat123".into(),
            new_password_confirmation: "BedaLagi123".into(),
        };
        assert!(r.validate().is_err());

        let r = ChangePasswordRequest {
            current_password: "lama123".into(),
            new_password: "BaruKuat123".into(),
            new_password_confirmation: "BaruKuat123".into(),
        };
        assert!(r.validate().is_ok());
    }

    #[test]
    fn login_menolak_input_kosong() {
        let r = LoginRequest {
            identifier: "ab".into(),
            password: "123".into(),
            device_name: None,
        };
        let err = r.validate().unwrap_err();
        let fields: Vec<String> = err.field_errors().keys().map(|k| k.to_string()).collect();
        assert!(fields.contains(&"identifier".to_string()));
        assert!(fields.contains(&"password".to_string()));
    }
}
