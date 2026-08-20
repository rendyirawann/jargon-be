//! Autentikasi & otorisasi.
//!
//! Ada TIGA jenis pemanggil API, masing-masing dengan extractor sendiri:
//!
//! | Pemanggil            | Header                          | Extractor    |
//! |----------------------|----------------------------------|--------------|
//! | Manusia (guru/kepsek/superadmin) | `Authorization: Bearer <jwt>` | [`AuthUser`]   |
//! | Tablet kios          | `Authorization: Device <token>`  | [`AuthDevice`] |
//! | Service (dashboard)  | `X-Api-Key` + `X-Api-Secret`     | [`AuthClient`] |
//!
//! Selain izin (RBAC), setiap akses juga melewati **penjaga tenant**:
//! seorang guru di SMA N 2 Binjai tidak akan pernah bisa membaca data
//! sekolah lain, walaupun ia menebak UUID-nya. Aturan itu terpusat di
//! [`AuthUser::resolve_school`] sehingga tidak bisa lupa diterapkan di
//! salah satu handler.

pub mod jwt;
pub mod password;

use std::collections::HashSet;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// Peran dengan cakupan seluruh provinsi.
pub const PROVINCE_ROLES: [&str; 2] = ["superadmin", "admin_dinas"];
pub const ROLE_SUPERADMIN: &str = "superadmin";

// =====================================================================
// Pengguna manusia
// =====================================================================

/// Peran yang cakupannya dibatasi pada siswa tertentu, bukan pada sekolah.
pub const STUDENT_SCOPED_ROLES: [&str; 2] = ["siswa", "orang_tua"];

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub username: String,
    pub name: String,
    /// NIK atau NISN — identitas login Jargon GO.
    pub identity: Option<String>,
    /// Sekolah utama; `None` untuk peran tingkat provinsi.
    pub school_id: Option<Uuid>,
    /// Sekolah tambahan yang boleh diakses.
    pub extra_schools: Vec<Uuid>,
    /// Siswa yang boleh dilihat akun ini.
    ///
    /// Kosong berarti TIDAK dibatasi pada siswa tertentu (guru, staff, dinas).
    /// Terisi berarti akun hanya boleh melihat siswa dalam daftar ini.
    pub students: Vec<Uuid>,
    pub roles: Vec<String>,
    pub permissions: HashSet<String>,
}

impl AuthUser {
    pub fn is_superadmin(&self) -> bool {
        self.roles.iter().any(|r| r == ROLE_SUPERADMIN)
    }

    /// Benar bila user boleh melihat lintas sekolah (Disdik provinsi).
    pub fn is_province_scope(&self) -> bool {
        self.roles.iter().any(|r| PROVINCE_ROLES.contains(&r.as_str()))
    }

    pub fn has_permission(&self, perm: &str) -> bool {
        self.is_superadmin() || self.permissions.contains(perm)
    }

    /// Gagalkan request bila izin tidak dimiliki.
    pub fn require(&self, perm: &str) -> ApiResult<()> {
        if self.has_permission(perm) {
            Ok(())
        } else {
            Err(ApiError::Forbidden(format!(
                "Anda tidak memiliki izin `{perm}`"
            )))
        }
    }

    pub fn require_any(&self, perms: &[&str]) -> ApiResult<()> {
        if perms.iter().any(|p| self.has_permission(p)) {
            Ok(())
        } else {
            Err(ApiError::Forbidden(format!(
                "Anda tidak memiliki salah satu izin: {}",
                perms.join(", ")
            )))
        }
    }

    /// Daftar sekolah yang boleh diakses. `None` = semua sekolah.
    pub fn accessible_schools(&self) -> Option<Vec<Uuid>> {
        if self.is_province_scope() {
            return None;
        }
        let mut out = Vec::with_capacity(1 + self.extra_schools.len());
        if let Some(s) = self.school_id {
            out.push(s);
        }
        out.extend(self.extra_schools.iter().copied());
        Some(out)
    }

    /// Inti penjaga tenant.
    ///
    /// * `requested = Some(x)` — pastikan user memang berhak atas sekolah `x`.
    /// * `requested = None`    — pakai sekolah milik user; untuk peran
    ///   provinsi `None` diteruskan yang berarti "semua sekolah".
    pub fn resolve_school(&self, requested: Option<Uuid>) -> ApiResult<Option<Uuid>> {
        match self.accessible_schools() {
            // Cakupan provinsi: bebas memilih, termasuk tidak memfilter.
            None => Ok(requested),
            Some(allowed) => {
                if allowed.is_empty() {
                    return Err(ApiError::Forbidden(
                        "Akun Anda belum ditautkan ke sekolah mana pun. Hubungi administrator.".into(),
                    ));
                }
                match requested {
                    None => Ok(Some(allowed[0])),
                    Some(want) if allowed.contains(&want) => Ok(Some(want)),
                    Some(_) => Err(ApiError::Forbidden(
                        "Anda hanya dapat mengakses data sekolah Anda sendiri".into(),
                    )),
                }
            }
        }
    }

    /// Varian yang mewajibkan satu sekolah konkret (untuk operasi tulis).
    pub fn require_school(&self, requested: Option<Uuid>) -> ApiResult<Uuid> {
        self.resolve_school(requested)?.ok_or_else(|| {
            ApiError::BadRequest(
                "Parameter `school_id` wajib diisi untuk operasi ini".into(),
            )
        })
    }

    // =================================================================
    // Cakupan siswa (siswa & orang tua)
    // =================================================================

    /// Benar bila akun ini hanya boleh melihat siswa tertentu.
    ///
    /// Ini dimensi cakupan KEDUA, terpisah dari cakupan sekolah. Seorang
    /// orang tua bisa punya anak di dua sekolah berbeda, sehingga
    /// membatasinya lewat `school_id` saja justru akan memberinya akses ke
    /// seluruh siswa di kedua sekolah itu.
    pub fn is_student_scoped(&self) -> bool {
        self.roles
            .iter()
            .any(|r| STUDENT_SCOPED_ROLES.contains(&r.as_str()))
    }

    /// Daftar siswa yang boleh dilihat, atau `None` bila tidak dibatasi.
    pub fn accessible_students(&self) -> Option<&[Uuid]> {
        self.is_student_scoped().then_some(self.students.as_slice())
    }

    /// Gagalkan permintaan bila siswa tersebut di luar cakupan akun.
    ///
    /// Dipakai setiap kali sebuah endpoint menerima `student_id` dari klien.
    pub fn require_student(&self, student_id: Uuid) -> ApiResult<()> {
        match self.accessible_students() {
            None => Ok(()),
            Some(allowed) if allowed.contains(&student_id) => Ok(()),
            Some(allowed) if allowed.is_empty() => Err(ApiError::Forbidden(
                "Akun Anda belum ditautkan ke data siswa mana pun. Hubungi \
                 operator sekolah."
                    .into(),
            )),
            Some(_) => Err(ApiError::Forbidden(
                "Anda hanya dapat melihat data siswa yang terkait dengan akun Anda".into(),
            )),
        }
    }

    /// Daftar siswa yang harus difilter untuk sebuah query daftar.
    ///
    /// * `None`      -> tanpa batasan siswa (guru/staff/dinas)
    /// * `Some(ids)` -> query WAJIB menambahkan `student_id = ANY(ids)`
    ///
    /// Bila klien menyebut satu `student_id`, nilai itu diperiksa dulu
    /// terhadap cakupan sehingga tidak mungkin lolos lewat parameter.
    pub fn resolve_students(&self, requested: Option<Uuid>) -> ApiResult<Option<Vec<Uuid>>> {
        match (self.accessible_students(), requested) {
            (None, _) => Ok(None),
            (Some(_), Some(id)) => {
                self.require_student(id)?;
                Ok(Some(vec![id]))
            }
            (Some(allowed), None) => {
                if allowed.is_empty() {
                    return Err(ApiError::Forbidden(
                        "Akun Anda belum ditautkan ke data siswa mana pun. Hubungi \
                         operator sekolah."
                            .into(),
                    ));
                }
                Ok(Some(allowed.to_vec()))
            }
        }
    }

    /// Label peran untuk ditampilkan di aplikasi.
    pub fn role_label(&self) -> &'static str {
        match self.roles.first().map(String::as_str) {
            Some("superadmin") => "Superadmin",
            Some("admin_dinas") => "Admin Dinas",
            Some("petugas_pengaduan") => "Petugas Pengaduan",
            Some("kepala_sekolah") => "Kepala Sekolah",
            Some("guru") => "Guru",
            Some("staff_tu") => "Staff TU",
            Some("siswa") => "Siswa",
            Some("orang_tua") => "Orang Tua",
            _ => "Pengguna",
        }
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer_token(parts, "Bearer")
            .ok_or_else(|| ApiError::Unauthorized("header Authorization tidak ada".into()))?;

        let claims = state.jwt.verify(&token)?;

        Ok(AuthUser {
            id: claims.sub,
            username: claims.username,
            name: claims.name,
            identity: claims.identity,
            school_id: claims.school_id,
            extra_schools: claims.scopes,
            students: claims.students,
            roles: claims.roles,
            permissions: claims.perms.into_iter().collect(),
        })
    }
}

// =====================================================================
// Tablet kios
// =====================================================================

// Beberapa field belum dibaca oleh handler mana pun, tetapi ikut dimuat
// sekali dari database dan dipakai untuk logging/diagnosis di lapangan
// (`code`, `name`) serta verifikasi tanda tangan HMAC (`hmac_secret`).
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AuthDevice {
    pub id: Uuid,
    pub school_id: Uuid,
    pub code: String,
    pub name: String,
    pub mode: String,
    pub placement: String,
    pub classroom_id: Option<Uuid>,
    pub hmac_secret: Option<Vec<u8>>,
}

impl AuthDevice {
    pub fn can_enroll(&self) -> bool {
        self.mode == "enroll"
    }
}

impl FromRequestParts<AppState> for AuthDevice {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer_token(parts, "Device")
            .ok_or_else(|| ApiError::Unauthorized("token perangkat tidak ada".into()))?;

        state.lookup_device(&token).await
    }
}

// =====================================================================
// Klien server-to-server
// =====================================================================

// Jalur server-to-server: dashboard Laravel memanggil API dengan kredensial
// layanan. Extractor-nya sudah lengkap dan teruji, namun belum dipasang pada
// route mana pun karena dashboard saat ini membaca PostgreSQL langsung untuk
// tampilan daftar. Disiapkan di sini supaya endpoint internal berikutnya
// tinggal menambahkan `client: AuthClient` pada signature handler.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AuthClient {
    pub id: Uuid,
    pub name: String,
    pub school_id: Option<Uuid>,
    pub scopes: Vec<String>,
}

#[allow(dead_code)]
impl AuthClient {
    pub fn require_scope(&self, scope: &str) -> ApiResult<()> {
        if self.scopes.iter().any(|s| s == scope || s == "*") {
            Ok(())
        } else {
            Err(ApiError::Forbidden(format!("scope `{scope}` diperlukan")))
        }
    }
}

impl FromRequestParts<AppState> for AuthClient {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let key_id = parts
            .headers
            .get("x-api-key")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ApiError::Unauthorized("header X-Api-Key tidak ada".into()))?
            .to_string();
        let secret = parts
            .headers
            .get("x-api-secret")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ApiError::Unauthorized("header X-Api-Secret tidak ada".into()))?
            .to_string();

        state.lookup_api_client(&key_id, &secret).await
    }
}

// =====================================================================
// Helper
// =====================================================================

/// Ambil kredensial dari header `Authorization: <scheme> <value>`.
/// Perbandingan skema case-insensitive sesuai RFC 7235.
fn bearer_token(parts: &Parts, scheme: &str) -> Option<String> {
    let raw = parts.headers.get(axum::http::header::AUTHORIZATION)?.to_str().ok()?;
    let (got_scheme, value) = raw.split_once(' ')?;
    if got_scheme.eq_ignore_ascii_case(scheme) {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(roles: &[&str], school: Option<Uuid>, perms: &[&str]) -> AuthUser {
        AuthUser {
            id: Uuid::new_v4(),
            username: "uji".into(),
            name: "Uji".into(),
            identity: None,
            school_id: school,
            extra_schools: vec![],
            students: vec![],
            roles: roles.iter().map(|s| s.to_string()).collect(),
            permissions: perms.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn superadmin_punya_semua_izin() {
        let u = user(&["superadmin"], None, &[]);
        assert!(u.has_permission("apa_saja_yang_belum_ada"));
        assert!(u.require("delete_school").is_ok());
    }

    #[test]
    fn guru_hanya_izin_yang_diberikan() {
        let u = user(&["guru"], Some(Uuid::new_v4()), &["view_student"]);
        assert!(u.require("view_student").is_ok());
        assert!(u.require("delete_student").is_err());
    }

    #[test]
    fn guru_tidak_bisa_menembus_sekolah_lain() {
        let own = Uuid::new_v4();
        let other = Uuid::new_v4();
        let u = user(&["guru"], Some(own), &["view_student"]);

        // Tanpa parameter -> otomatis sekolah sendiri.
        assert_eq!(u.resolve_school(None).unwrap(), Some(own));
        // Sekolah sendiri -> boleh.
        assert_eq!(u.resolve_school(Some(own)).unwrap(), Some(own));
        // Sekolah lain -> ditolak.
        assert!(matches!(
            u.resolve_school(Some(other)),
            Err(ApiError::Forbidden(_))
        ));
    }

    #[test]
    fn dinas_boleh_lintas_sekolah_dan_tanpa_filter() {
        let u = user(&["admin_dinas"], None, &["view_student"]);
        let any = Uuid::new_v4();
        assert_eq!(u.resolve_school(None).unwrap(), None);
        assert_eq!(u.resolve_school(Some(any)).unwrap(), Some(any));
    }

    #[test]
    fn pengawas_dengan_beberapa_sekolah() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        let mut u = user(&["guru"], Some(a), &[]);
        u.extra_schools = vec![b];

        assert_eq!(u.resolve_school(Some(b)).unwrap(), Some(b));
        assert!(u.resolve_school(Some(c)).is_err());
    }

    #[test]
    fn akun_tanpa_sekolah_dan_bukan_dinas_ditolak() {
        let u = user(&["guru"], None, &["view_student"]);
        let err = u.resolve_school(None).unwrap_err();
        assert!(matches!(err, ApiError::Forbidden(_)));
    }

    #[test]
    fn require_school_menolak_permintaan_tanpa_sekolah_untuk_dinas() {
        let u = user(&["superadmin"], None, &[]);
        assert!(u.require_school(None).is_err());
        let s = Uuid::new_v4();
        assert_eq!(u.require_school(Some(s)).unwrap(), s);
    }

    // -----------------------------------------------------------------
    // Cakupan siswa — inti privasi akun siswa & orang tua
    // -----------------------------------------------------------------

    #[test]
    fn guru_tidak_dibatasi_pada_siswa_tertentu() {
        let u = user(&["guru"], Some(Uuid::new_v4()), &["view_attendance"]);
        assert!(!u.is_student_scoped());
        assert!(u.accessible_students().is_none());
        // Boleh melihat siswa mana pun di sekolahnya.
        assert!(u.require_student(Uuid::new_v4()).is_ok());
        assert_eq!(u.resolve_students(None).unwrap(), None);
    }

    #[test]
    fn siswa_hanya_melihat_dirinya_sendiri() {
        let diri = Uuid::new_v4();
        let orang_lain = Uuid::new_v4();

        let mut u = user(&["siswa"], Some(Uuid::new_v4()), &["view_own_attendance"]);
        u.students = vec![diri];

        assert!(u.is_student_scoped());
        assert!(u.require_student(diri).is_ok());
        assert!(matches!(
            u.require_student(orang_lain),
            Err(ApiError::Forbidden(_))
        ));
        assert_eq!(u.resolve_students(None).unwrap(), Some(vec![diri]));
    }

    #[test]
    fn orang_tua_melihat_semua_anaknya_lintas_sekolah() {
        // Anak di sekolah berbeda: inilah alasan cakupan siswa tidak bisa
        // diturunkan dari school_id.
        let anak1 = Uuid::new_v4();
        let anak2 = Uuid::new_v4();

        let mut u = user(&["orang_tua"], None, &["view_children_attendance"]);
        u.students = vec![anak1, anak2];

        let resolved = u.resolve_students(None).unwrap().unwrap();
        assert!(resolved.contains(&anak1));
        assert!(resolved.contains(&anak2));

        // Menyebut anak sendiri: dipersempit ke anak itu saja.
        assert_eq!(u.resolve_students(Some(anak2)).unwrap(), Some(vec![anak2]));
    }

    #[test]
    fn orang_tua_tidak_bisa_menembus_lewat_parameter_student_id() {
        let anak = Uuid::new_v4();
        let anak_orang_lain = Uuid::new_v4();

        let mut u = user(&["orang_tua"], None, &["view_children_attendance"]);
        u.students = vec![anak];

        assert!(matches!(
            u.resolve_students(Some(anak_orang_lain)),
            Err(ApiError::Forbidden(_))
        ));
    }

    #[test]
    fn akun_siswa_tanpa_tautan_gagal_aman() {
        // Akun yang salah konfigurasi harus melihat NOL data, bukan semua.
        let u = user(&["siswa"], Some(Uuid::new_v4()), &["view_own_attendance"]);
        assert!(u.students.is_empty());
        assert!(matches!(
            u.resolve_students(None),
            Err(ApiError::Forbidden(_))
        ));
        assert!(matches!(
            u.require_student(Uuid::new_v4()),
            Err(ApiError::Forbidden(_))
        ));
    }

    #[test]
    fn label_peran_untuk_aplikasi() {
        assert_eq!(user(&["siswa"], None, &[]).role_label(), "Siswa");
        assert_eq!(user(&["orang_tua"], None, &[]).role_label(), "Orang Tua");
        assert_eq!(
            user(&["petugas_pengaduan"], None, &[]).role_label(),
            "Petugas Pengaduan"
        );
        assert_eq!(user(&[], None, &[]).role_label(), "Pengguna");
    }

    #[test]
    fn parsing_header_authorization() {
        use axum::http::{HeaderValue, Request};
        let req = Request::builder()
            .header("authorization", HeaderValue::from_static("device  abc123 "))
            .body(())
            .unwrap();
        let (parts, _) = req.into_parts();
        assert_eq!(bearer_token(&parts, "Device").as_deref(), Some("abc123"));
        assert_eq!(bearer_token(&parts, "Bearer"), None);
    }
}
