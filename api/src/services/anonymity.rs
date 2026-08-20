//! Penyamaran identitas pelapor pada Panic Button.
//!
//! DUA JENIS HANDLE, DENGAN SIFAT YANG SENGAJA BERBEDA
//!
//! 1. **Handle laporan** — dibuat ACAK setiap kali laporan dibuat.
//!    Dua laporan dari siswa yang sama menghasilkan handle berbeda dan tidak
//!    dapat dikaitkan satu sama lain. Ini penting: bila handle stabil per
//!    pengguna, pihak sekolah dapat mengumpulkan beberapa laporan bertanda
//!    sama, menyilangkannya dengan tanggal kejadian dan isi laporan, lalu
//!    mempersempit siapa pelapornya. Anonimitas yang bisa dipersempit bukan
//!    anonimitas.
//!
//! 2. **Handle komentar** — STABIL per (laporan, penulis), diturunkan lewat
//!    HMAC dari kunci rahasia server. Dalam satu utas, komentator yang sama
//!    terlihat konsisten sehingga percakapan bisa diikuti; di utas lain,
//!    orang yang sama tampil dengan handle berbeda.
//!
//! Keduanya tidak dapat dibalik menjadi identitas. Pembukaan identitas hanya
//! mungkin lewat kolom `author_user_id` yang dijaga izin khusus dan dicatat
//! pada `panic_unmask_logs`.

use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// Karakter handle. Huruf yang mudah tertukar (I, O, 0, 1) dibuang supaya
/// petugas bisa menyebutkan handle lewat telepon tanpa salah dengar.
const ALPHABET: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";

/// Panjang bagian acak. 4 karakter dari 32 simbol = ~1 juta kemungkinan,
/// cukup agar tabrakan dalam satu utas praktis tidak terjadi, dan cukup
/// pendek untuk dibaca sekilas.
const CODE_LEN: usize = 4;

/// Label peran yang ditampilkan di depan handle.
///
/// Peran ditampilkan karena berguna bagi penanganan — laporan dari guru
/// ditangani berbeda dari laporan siswa — dan tidak mempersempit identitas
/// pada sekolah yang punya ratusan siswa.
pub fn role_label(role: &str) -> &'static str {
    match role {
        "siswa" => "Siswa",
        "orang_tua" => "Wali",
        "guru" => "Guru",
        "staff_tu" => "Staff",
        "kepala_sekolah" => "Kepsek",
        _ => "Warga",
    }
}

/// Handle acak untuk sebuah laporan baru, mis. `Siswa#7K4M`.
pub fn report_handle(role: &str) -> String {
    let mut buf = [0u8; CODE_LEN];
    rand::rng().fill_bytes(&mut buf);
    format!("{}#{}", role_label(role), encode(&buf))
}

/// Handle komentar yang stabil dalam satu utas.
///
/// `secret` adalah `JWT_SECRET` server. Tanpa mengetahuinya, handle tidak
/// dapat dihitung ulang oleh pihak luar untuk mencocokkan pengguna.
pub fn comment_handle(secret: &[u8], report_id: Uuid, user_id: Uuid, role: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret)
        .expect("HMAC menerima kunci dengan panjang apa pun");
    mac.update(b"panic-comment:");
    mac.update(report_id.as_bytes());
    mac.update(user_id.as_bytes());

    let digest = mac.finalize().into_bytes();
    format!("{}#{}", role_label(role), encode(&digest[..CODE_LEN]))
}

fn encode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(CODE_LEN)
        .map(|b| ALPHABET[(*b as usize) % ALPHABET.len()] as char)
        .collect()
}

/// Samarkan nama sekolah untuk feed publik lintas provinsi.
///
/// Pada laporan bertingkat keparahan tinggi, menampilkan nama sekolah persis
/// di feed publik dapat mempersempit pelapor sampai ke satu kelas. Nama
/// lengkap tetap terlihat oleh Dinas yang menangani.
pub fn blur_school_name(name: &str, jenjang: &str) -> String {
    // Ambil kata pertama saja: "SMA Negeri 1 Medan" -> "SMA di Medan".
    let kota = name
        .split_whitespace()
        .last()
        .filter(|w| w.len() > 2)
        .unwrap_or("Sumatera Utara");
    format!("{jenjang} di {kota}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn handle_laporan_selalu_berbeda() {
        // Sifat inti: dua laporan dari orang yang sama tidak dapat dikaitkan.
        let handles: HashSet<String> = (0..200).map(|_| report_handle("siswa")).collect();
        assert!(
            handles.len() > 150,
            "handle laporan terlalu sering berulang: {} unik dari 200",
            handles.len()
        );
    }

    #[test]
    fn handle_laporan_memakai_label_peran() {
        assert!(report_handle("siswa").starts_with("Siswa#"));
        assert!(report_handle("orang_tua").starts_with("Wali#"));
        assert!(report_handle("guru").starts_with("Guru#"));
        assert!(report_handle("peran_asing").starts_with("Warga#"));
    }

    #[test]
    fn handle_laporan_panjangnya_pasti() {
        let h = report_handle("siswa");
        let (label, code) = h.split_once('#').unwrap();
        assert_eq!(label, "Siswa");
        assert_eq!(code.len(), CODE_LEN);
    }

    #[test]
    fn handle_komentar_stabil_dalam_satu_utas() {
        let secret = b"rahasia-uji";
        let report = Uuid::new_v4();
        let user = Uuid::new_v4();

        let a = comment_handle(secret, report, user, "siswa");
        let b = comment_handle(secret, report, user, "siswa");
        assert_eq!(a, b, "komentator yang sama harus konsisten dalam satu utas");
    }

    #[test]
    fn handle_komentar_berbeda_antar_utas() {
        // Orang yang sama di dua laporan berbeda tidak boleh terlihat sama,
        // kalau tidak pola komentarnya bisa dipakai melacak.
        let secret = b"rahasia-uji";
        let user = Uuid::new_v4();

        let a = comment_handle(secret, Uuid::new_v4(), user, "siswa");
        let b = comment_handle(secret, Uuid::new_v4(), user, "siswa");
        assert_ne!(a, b);
    }

    #[test]
    fn handle_komentar_berbeda_antar_pengguna() {
        let secret = b"rahasia-uji";
        let report = Uuid::new_v4();

        let a = comment_handle(secret, report, Uuid::new_v4(), "siswa");
        let b = comment_handle(secret, report, Uuid::new_v4(), "siswa");
        assert_ne!(a, b);
    }

    #[test]
    fn handle_komentar_bergantung_pada_kunci_server() {
        // Tanpa kunci server, pihak luar tidak dapat menghitung ulang handle
        // untuk mencocokkannya dengan daftar pengguna.
        let report = Uuid::new_v4();
        let user = Uuid::new_v4();

        let a = comment_handle(b"kunci-satu", report, user, "siswa");
        let b = comment_handle(b"kunci-dua", report, user, "siswa");
        assert_ne!(a, b);
    }

    #[test]
    fn handle_tidak_memuat_karakter_ambigu() {
        for _ in 0..100 {
            let h = report_handle("siswa");
            let code = h.split('#').nth(1).unwrap();
            for c in code.chars() {
                assert!(
                    !"IO01".contains(c),
                    "karakter ambigu `{c}` menyulitkan pembacaan lewat telepon"
                );
            }
        }
    }

    #[test]
    fn nama_sekolah_disamarkan() {
        assert_eq!(
            blur_school_name("SMA Negeri 1 Medan", "SMA"),
            "SMA di Medan"
        );
        // Nama tanpa kota yang jelas tetap menghasilkan sesuatu yang aman.
        assert_eq!(blur_school_name("SD", "SD"), "SD di Sumatera Utara");
    }
}
