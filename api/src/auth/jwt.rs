//! Penerbitan & verifikasi JWT akses.
//!
//! Token akses bersifat *self-contained*: membawa peran dan izin milik user
//! sehingga endpoint tidak perlu memanggil database untuk setiap request
//! otorisasi. Konsekuensinya perubahan izin baru berlaku setelah token
//! kedaluwarsa (default 1 jam) atau setelah user refresh — trade-off yang
//! disengaja demi menghindari 3 query tambahan pada setiap request absensi.
//!
//! Pencabutan segera tetap mungkin lewat refresh token yang stateful
//! (tabel `refresh_tokens`) dan flag `users.is_active`/`banned_at`.

use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};

pub const TOKEN_TYPE_ACCESS: &str = "access";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// User id (UUID).
    pub sub: Uuid,
    pub iss: String,
    pub iat: i64,
    pub exp: i64,
    /// Id token, dipakai untuk denylist bila diperlukan.
    pub jti: String,
    /// Selalu "access" untuk token ini.
    pub typ: String,

    pub username: String,
    pub name: String,
    /// NIK (16 digit) atau NISN (10 digit) — identitas login Jargon GO.
    #[serde(default)]
    pub identity: Option<String>,
    /// Sekolah utama. `None` berarti cakupan provinsi (superadmin/dinas).
    pub school_id: Option<Uuid>,
    /// Sekolah tambahan (mis. pengawas yang membina beberapa sekolah).
    #[serde(default)]
    pub scopes: Vec<Uuid>,
    /// Siswa yang boleh dilihat akun ini.
    ///
    /// Kosong = tidak dibatasi pada siswa tertentu (guru, staff, dinas).
    /// Terisi = akun hanya boleh melihat siswa dalam daftar ini:
    ///   * siswa      -> satu id, dirinya sendiri
    ///   * orang tua  -> id anak-anaknya, bisa lintas sekolah
    #[serde(default)]
    pub students: Vec<Uuid>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub perms: Vec<String>,
}

/// Data yang dimuat ke dalam access token.
#[derive(Debug, Clone)]
pub struct AccessTokenInput {
    pub user_id: Uuid,
    pub username: String,
    pub name: String,
    pub identity: Option<String>,
    pub school_id: Option<Uuid>,
    pub scopes: Vec<Uuid>,
    pub students: Vec<Uuid>,
    pub roles: Vec<String>,
    pub perms: Vec<String>,
}

#[derive(Clone)]
pub struct JwtKeys {
    encoding: EncodingKey,
    decoding: DecodingKey,
    issuer: String,
}

impl JwtKeys {
    pub fn new(secret: &str, issuer: &str) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret.as_bytes()),
            decoding: DecodingKey::from_secret(secret.as_bytes()),
            issuer: issuer.to_string(),
        }
    }

    /// Terbitkan access token.
    ///
    /// Parameter dikelompokkan dalam [`AccessTokenInput`] karena jumlahnya
    /// sudah cukup banyak sehingga urutan posisional mudah tertukar —
    /// menukar `scopes` dengan `students` akan menghasilkan kebocoran data
    /// yang tidak terlihat pada waktu kompilasi.
    pub fn issue_access(
        &self,
        input: AccessTokenInput,
        ttl: std::time::Duration,
    ) -> ApiResult<(String, i64)> {
        let now = Utc::now().timestamp();
        let exp = now + ttl.as_secs() as i64;
        let claims = Claims {
            sub: input.user_id,
            iss: self.issuer.clone(),
            iat: now,
            exp,
            jti: Uuid::new_v4().to_string(),
            typ: TOKEN_TYPE_ACCESS.to_string(),
            username: input.username,
            name: input.name,
            identity: input.identity,
            school_id: input.school_id,
            scopes: input.scopes,
            students: input.students,
            roles: input.roles,
            perms: input.perms,
        };
        let token = encode(&Header::new(Algorithm::HS256), &claims, &self.encoding)
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("gagal menerbitkan token: {e}")))?;
        Ok((token, exp))
    }

    pub fn verify(&self, token: &str) -> ApiResult<Claims> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[self.issuer.clone()]);
        validation.set_required_spec_claims(&["exp", "iss", "sub"]);
        // Toleransi 30 detik untuk selisih jam tablet.
        validation.leeway = 30;

        let data = decode::<Claims>(token, &self.decoding, &validation).map_err(|e| {
            use jsonwebtoken::errors::ErrorKind;
            match e.kind() {
                ErrorKind::ExpiredSignature => {
                    ApiError::Unauthorized("token sudah kedaluwarsa".into())
                }
                _ => ApiError::Unauthorized("token tidak valid".into()),
            }
        })?;

        if data.claims.typ != TOKEN_TYPE_ACCESS {
            return Err(ApiError::Unauthorized("jenis token tidak sesuai".into()));
        }
        Ok(data.claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn keys() -> JwtKeys {
        JwtKeys::new("rahasia-uji-yang-cukup-panjang-untuk-hs256-xx", "jargon-api")
    }

    fn input(school: Option<Uuid>) -> AccessTokenInput {
        AccessTokenInput {
            user_id: Uuid::new_v4(),
            username: "budi".into(),
            name: "Budi Guru".into(),
            identity: Some("1275010101900001".into()),
            school_id: school,
            scopes: vec![],
            students: vec![],
            roles: vec!["guru".into()],
            perms: vec!["view_student".into()],
        }
    }

    #[test]
    fn terbitkan_lalu_verifikasi() {
        let k = keys();
        let sid = Uuid::new_v4();
        let payload = input(Some(sid));
        let uid = payload.user_id;

        let (token, exp) = k.issue_access(payload, Duration::from_secs(60)).unwrap();

        assert!(exp > Utc::now().timestamp());
        let c = k.verify(&token).unwrap();
        assert_eq!(c.sub, uid);
        assert_eq!(c.school_id, Some(sid));
        assert_eq!(c.roles, vec!["guru".to_string()]);
        assert!(c.perms.contains(&"view_student".to_string()));
        assert_eq!(c.identity.as_deref(), Some("1275010101900001"));
    }

    #[test]
    fn cakupan_siswa_ikut_terbawa_di_token() {
        // Akun orang tua membawa daftar anaknya. Kalau daftar ini hilang saat
        // token diterbitkan, orang tua akan kehilangan akses ke data anaknya
        // sendiri — atau lebih buruk, dianggap tidak dibatasi sama sekali.
        let k = keys();
        let anak1 = Uuid::new_v4();
        let anak2 = Uuid::new_v4();

        let mut payload = input(None);
        payload.roles = vec!["orang_tua".into()];
        payload.students = vec![anak1, anak2];

        let (token, _) = k.issue_access(payload, Duration::from_secs(60)).unwrap();
        let c = k.verify(&token).unwrap();

        assert_eq!(c.students, vec![anak1, anak2]);
    }

    #[test]
    fn token_dengan_secret_lain_ditolak() {
        let (token, _) = keys()
            .issue_access(input(None), Duration::from_secs(60))
            .unwrap();

        let other = JwtKeys::new("secret-lain-yang-juga-cukup-panjang-sekali", "jargon-api");
        assert!(other.verify(&token).is_err());
    }

    #[test]
    fn issuer_berbeda_ditolak() {
        let (token, _) = keys()
            .issue_access(input(None), Duration::from_secs(60))
            .unwrap();
        let other = JwtKeys::new(
            "rahasia-uji-yang-cukup-panjang-untuk-hs256-xx",
            "issuer-lain",
        );
        assert!(other.verify(&token).is_err());
    }

    #[test]
    fn token_kedaluwarsa_ditolak() {
        let k = keys();
        // TTL 0 + leeway 30s masih lolos, jadi pakai klaim exp di masa lalu.
        let claims = Claims {
            sub: Uuid::new_v4(),
            iss: "jargon-api".into(),
            iat: Utc::now().timestamp() - 7200,
            exp: Utc::now().timestamp() - 3600,
            jti: Uuid::new_v4().to_string(),
            typ: TOKEN_TYPE_ACCESS.into(),
            username: "a".into(),
            name: "A".into(),
            identity: None,
            school_id: None,
            scopes: vec![],
            students: vec![],
            roles: vec![],
            perms: vec![],
        };
        let token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(b"rahasia-uji-yang-cukup-panjang-untuk-hs256-xx"),
        )
        .unwrap();
        let err = k.verify(&token).unwrap_err();
        assert!(matches!(err, ApiError::Unauthorized(_)));
    }
}
