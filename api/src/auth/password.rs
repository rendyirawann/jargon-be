//! Hashing kata sandi & token.
//!
//! Kata sandi memakai **bcrypt**, bukan Argon2, dengan alasan yang sangat
//! konkret: tabel `users` dipakai bersama oleh API Rust dan dashboard
//! Laravel. Hash driver default Laravel adalah bcrypt, sehingga memilih
//! bcrypt di sini membuat `Hash::check()` di PHP dan `verify()` di Rust
//! saling kompatibel — seorang user bisa login di kedua sisi dengan satu
//! kata sandi, dan reset password dari salah satu sisi tetap berlaku.
//!
//! Token perangkat/refresh TIDAK di-hash dengan bcrypt: token itu sudah
//! acak 256-bit, jadi SHA-256 sudah cukup dan jauh lebih murah (dipanggil
//! pada setiap request absensi).

use base64::Engine as _;
use rand::RngCore;

use crate::error::{ApiError, ApiResult};

/// Cost 12 = default Laravel (config/hashing.php `bcrypt.rounds`).
const BCRYPT_COST: u32 = 12;

pub fn hash_password(plain: &str) -> ApiResult<String> {
    bcrypt::hash(plain, BCRYPT_COST)
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("gagal hash password: {e}")))
}

/// Verifikasi kata sandi. Hash yang rusak/format asing diperlakukan sebagai
/// gagal, bukan error, supaya tidak membocorkan informasi lewat perbedaan
/// respons.
pub fn verify_password(plain: &str, hashed: &str) -> bool {
    // Laravel menulis prefix `$2y$`; crate bcrypt menerimanya.
    bcrypt::verify(plain, hashed).unwrap_or(false)
}

/// Token acak URL-safe. 32 byte entropi = 256 bit.
pub fn generate_token() -> String {
    let mut buf = [0u8; 32];
    rand::rng().fill_bytes(&mut buf);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

/// Rahasia HMAC untuk penandatanganan request oleh tablet.
pub fn generate_secret() -> Vec<u8> {
    let mut buf = vec![0u8; 32];
    rand::rng().fill_bytes(&mut buf);
    buf
}

/// Kata sandi awal untuk akun yang dibuat massal (mis. akun siswa).
///
/// Dibuat ACAK, bukan diturunkan dari NISN atau tanggal lahir. Keduanya
/// tercetak pada kartu pelajar dan diketahui teman sekelas — memakainya
/// sebagai kata sandi awal berarti setiap siswa dapat masuk ke akun temannya
/// pada hari pertama pemakaian.
///
/// Alfabet tanpa huruf/angka yang mudah tertukar (I, l, 1, O, 0), karena
/// kata sandi ini akan dibagikan di atas kertas dan diketik ulang siswa.
pub fn generate_initial_password() -> String {
    const CHARS: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
    let mut buf = [0u8; 10];
    rand::rng().fill_bytes(&mut buf);
    buf.iter()
        .map(|b| CHARS[(*b as usize) % CHARS.len()] as char)
        .collect()
}

/// Kode pairing 8 digit yang mudah dibacakan lewat telepon.
pub fn generate_pairing_code() -> String {
    let mut buf = [0u8; 4];
    rand::rng().fill_bytes(&mut buf);
    let n = u32::from_le_bytes(buf) % 100_000_000;
    format!("{n:08}")
}

/// Perbandingan waktu-konstan untuk membandingkan hash token.
///
/// Dipakai oleh verifikasi kredensial layanan (`AppState::lookup_api_client`).
#[allow(dead_code)]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_lalu_verifikasi_berhasil() {
        let h = hash_password("Rahasia#2026").unwrap();
        assert!(verify_password("Rahasia#2026", &h));
        assert!(!verify_password("Rahasia#2027", &h));
    }

    #[test]
    fn hash_laravel_2y_dapat_diverifikasi() {
        // Hash ini dihasilkan PHP: password_hash('Superadmin#2026', PASSWORD_BCRYPT, ['cost'=>12])
        let laravel_hash = "$2y$12$RllO.hebA59eQO9X5OjvAOcJ02/tqDIYNH8VxPskGLD1ahjfqz.4.";
        assert!(
            verify_password("Superadmin#2026", laravel_hash),
            "hash bcrypt dari Laravel harus bisa diverifikasi oleh Rust"
        );
        assert!(!verify_password("salah", laravel_hash));
    }

    #[test]
    fn hash_rusak_tidak_panik() {
        assert!(!verify_password("apa saja", "bukan-hash"));
        assert!(!verify_password("apa saja", ""));
    }

    #[test]
    fn token_acak_dan_cukup_panjang() {
        let a = generate_token();
        let b = generate_token();
        assert_ne!(a, b);
        assert!(a.len() >= 40);
    }

    #[test]
    fn rahasia_hmac_tiga_puluh_dua_byte() {
        let s = generate_secret();
        assert_eq!(s.len(), 32);
        assert_ne!(s, generate_secret());
    }

    #[test]
    fn kata_sandi_awal_acak_dan_mudah_diketik() {
        let a = generate_initial_password();
        let b = generate_initial_password();
        assert_ne!(a, b);
        assert_eq!(a.len(), 10);

        // Karakter yang mudah tertukar saat dibaca dari kertas harus absen.
        for c in a.chars() {
            assert!(
                !"Il1O0".contains(c),
                "karakter ambigu `{c}` menyulitkan siswa mengetik ulang"
            );
        }
    }

    #[test]
    fn kata_sandi_awal_bisa_diverifikasi_setelah_di_hash() {
        let plain = generate_initial_password();
        let hashed = hash_password(&plain).unwrap();
        assert!(verify_password(&plain, &hashed));
    }

    #[test]
    fn kode_pairing_selalu_delapan_digit() {
        for _ in 0..50 {
            let c = generate_pairing_code();
            assert_eq!(c.len(), 8);
            assert!(c.chars().all(|ch| ch.is_ascii_digit()));
        }
    }

    #[test]
    fn constant_time_eq_benar() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
    }
}
