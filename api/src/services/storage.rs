//! Penyimpanan berkas (foto pendaftaran wajah, berkas impor, hasil ekspor).
//!
//! Implementasi saat ini menulis ke filesystem lokal. Di produksi direktori
//! ini diarahkan ke volume bersama / mount S3-compatible (MinIO) sehingga
//! beberapa replika API melihat berkas yang sama. Seluruh akses melewati
//! satu tipe [`Storage`] sehingga menambah backend lain nanti hanya
//! menyentuh berkas ini.
//!
//! Estimasi kapasitas: 700.000 siswa x 3 sampel x ~60 KB ≈ 126 GB.

use std::path::{Component, Path, PathBuf};

use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};

#[derive(Debug, Clone)]
pub struct Storage {
    root: PathBuf,
    public_base: String,
}

// `import_key`, `export_key`, `exists`, dan `root` melayani fitur impor massal
// siswa dan ekspor laporan (tabel `import_jobs` / `report_exports`) yang
// skemanya sudah ada. Diuji di modul ini agar aturan penamaan object key —
// termasuk pembersihan nama berkas — tidak berubah tanpa sengaja.
#[allow(dead_code)]
impl Storage {
    pub fn new(root: impl Into<PathBuf>, public_base: impl Into<String>) -> Self {
        Self { root: root.into(), public_base: public_base.into() }
    }

    /// Object key untuk foto pendaftaran wajah.
    ///
    /// Dipartisi per sekolah lalu per siswa agar satu direktori tidak pernah
    /// berisi ratusan ribu berkas (yang membuat operasi filesystem melambat).
    pub fn face_key(school_id: Uuid, student_id: Uuid, ext: &str) -> String {
        format!("faces/{school_id}/{student_id}/{}.{ext}", Uuid::new_v4())
    }

    pub fn import_key(school_id: Option<Uuid>, original: &str) -> String {
        let scope = school_id.map(|s| s.to_string()).unwrap_or_else(|| "provinsi".into());
        let safe = sanitize_filename(original);
        format!("imports/{scope}/{}-{safe}", Uuid::new_v4())
    }

    pub fn export_key(school_id: Option<Uuid>, name: &str) -> String {
        let scope = school_id.map(|s| s.to_string()).unwrap_or_else(|| "provinsi".into());
        format!("exports/{scope}/{}-{}", Uuid::new_v4(), sanitize_filename(name))
    }

    /// URL relatif yang bisa dilayani oleh reverse proxy / handler `/files`.
    pub fn public_url(&self, key: &str) -> String {
        format!("{}/{}", self.public_base.trim_end_matches('/'), key)
    }

    pub async fn put(&self, key: &str, bytes: &[u8]) -> ApiResult<()> {
        let path = self.resolve(key)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        // Tulis ke berkas sementara lalu rename: pembaca tidak pernah melihat
        // berkas setengah tertulis kalau proses mati di tengah jalan.
        let tmp = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
        {
            let mut f = tokio::fs::File::create(&tmp).await?;
            f.write_all(bytes).await?;
            f.sync_all().await?;
        }
        tokio::fs::rename(&tmp, &path).await?;
        Ok(())
    }

    pub async fn get(&self, key: &str) -> ApiResult<Vec<u8>> {
        let path = self.resolve(key)?;
        tokio::fs::read(&path)
            .await
            .map_err(|_| ApiError::NotFound(format!("berkas `{key}`")))
    }

    pub async fn delete(&self, key: &str) -> ApiResult<()> {
        let path = self.resolve(key)?;
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            // Menghapus berkas yang sudah tidak ada bukan kegagalan.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn exists(&self, key: &str) -> bool {
        match self.resolve(key) {
            Ok(p) => tokio::fs::metadata(&p).await.is_ok(),
            Err(_) => false,
        }
    }

    /// Ubah object key menjadi path absolut, menolak segala upaya
    /// path traversal (`../`, path absolut, prefix drive Windows).
    pub fn resolve(&self, key: &str) -> ApiResult<PathBuf> {
        let key = key.trim_start_matches('/');
        if key.is_empty() {
            return Err(ApiError::BadRequest("object key kosong".into()));
        }
        let rel = Path::new(key);
        for comp in rel.components() {
            match comp {
                Component::Normal(_) => {}
                _ => {
                    return Err(ApiError::BadRequest(
                        "object key tidak valid (path traversal terdeteksi)".into(),
                    ))
                }
            }
        }
        Ok(self.root.join(rel))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub async fn ensure_root(&self) -> ApiResult<()> {
        tokio::fs::create_dir_all(&self.root).await?;
        Ok(())
    }
}

/// Ubah nama berkas unggahan menjadi satu segmen path yang aman.
///
/// Langkah pertama mengambil basename: nama seperti `../../evil.sh` yang
/// dikirim klien hanya perlu diperlakukan sebagai `evil.sh`, bukan
/// diterjemahkan karakter per karakter menjadi `_.._evil.sh` yang masih
/// membawa jejak upaya traversal.
#[allow(dead_code)]
fn sanitize_filename(name: &str) -> String {
    let basename = name.rsplit(['/', '\\']).next().unwrap_or(name);

    let mut cleaned = String::with_capacity(basename.len());
    let mut prev_underscore = false;
    for c in basename.chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
            cleaned.push(c);
            prev_underscore = false;
        } else if !prev_underscore {
            cleaned.push('_');
            prev_underscore = true;
        }
    }

    let cleaned = cleaned.trim_matches(['.', '_', '-']).to_string();
    if cleaned.is_empty() {
        return "berkas".to_string();
    }

    // Potong dari belakang agar ekstensi ikut terselamatkan.
    let chars: Vec<char> = cleaned.chars().collect();
    if chars.len() > 80 {
        chars[chars.len() - 80..].iter().collect()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn storage() -> (Storage, tempdir::TempDirLike) {
        // Direktori sementara sederhana tanpa dependensi tambahan.
        let dir = std::env::temp_dir().join(format!("absensi-test-{}", Uuid::new_v4()));
        (Storage::new(dir.clone(), "/files"), tempdir::TempDirLike(dir))
    }

    // Pembungkus kecil supaya direktori uji terhapus otomatis.
    mod tempdir {
        pub struct TempDirLike(pub std::path::PathBuf);
        impl Drop for TempDirLike {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn menolak_path_traversal() {
        let (s, _g) = storage();
        assert!(s.resolve("../../etc/passwd").is_err());
        assert!(s.resolve("faces/../../rahasia").is_err());
        assert!(s.resolve("..").is_err());
        assert!(s.resolve("").is_err());
        assert!(s.resolve("faces/abc/def.jpg").is_ok());
    }

    #[test]
    fn key_dengan_garis_miring_awal_tetap_di_dalam_root() {
        // Object key kadang tiba dengan `/` di depan (mis. disalin dari URL).
        // Itu ditoleransi, tetapi hasilnya WAJIB tetap di bawah root — sifat
        // inilah yang benar-benar menentukan keamanan, bukan diterima/ditolak.
        let (s, _g) = storage();
        let resolved = s.resolve("/etc/passwd").expect("ditoleransi");
        assert!(
            resolved.starts_with(s.root()),
            "path keluar dari root: {resolved:?}"
        );
    }

    #[test]
    fn menolak_path_absolut_windows() {
        let (s, _g) = storage();
        assert!(s.resolve("C:\\Windows\\System32\\config").is_err());
        assert!(s.resolve("\\\\server\\share\\rahasia").is_err());
    }

    #[test]
    fn face_key_dipartisi_per_sekolah_dan_siswa() {
        let school = Uuid::new_v4();
        let student = Uuid::new_v4();
        let key = Storage::face_key(school, student, "jpg");
        assert!(key.starts_with(&format!("faces/{school}/{student}/")));
        assert!(key.ends_with(".jpg"));
        // Dua panggilan tidak boleh menghasilkan key yang sama.
        assert_ne!(key, Storage::face_key(school, student, "jpg"));
    }

    #[test]
    fn nama_berkas_dibersihkan() {
        assert_eq!(sanitize_filename("data siswa.xlsx"), "data_siswa.xlsx");
        assert_eq!(sanitize_filename(""), "berkas");
        assert_eq!(sanitize_filename("...."), "berkas");
    }

    #[test]
    fn nama_berkas_hanya_mengambil_basename() {
        // Upaya traversal pada nama berkas tidak boleh menyisakan jejak `..`.
        assert_eq!(sanitize_filename("../../evil.sh"), "evil.sh");
        assert_eq!(sanitize_filename("..\\..\\evil.sh"), "evil.sh");
        assert_eq!(
            sanitize_filename("/var/www/data siswa 2026.xlsx"),
            "data_siswa_2026.xlsx"
        );
    }

    #[test]
    fn nama_berkas_panjang_dipotong_tapi_ekstensi_dipertahankan() {
        let long = format!("{}.xlsx", "a".repeat(200));
        let out = sanitize_filename(&long);
        assert!(out.chars().count() <= 80);
        assert!(out.ends_with(".xlsx"), "ekstensi hilang: {out}");
    }

    #[tokio::test]
    async fn tulis_baca_hapus() {
        let (s, _g) = storage();
        s.ensure_root().await.unwrap();
        let key = "faces/a/b/c.jpg";

        s.put(key, b"halo dunia").await.unwrap();
        assert!(s.exists(key).await);
        assert_eq!(s.get(key).await.unwrap(), b"halo dunia");

        s.delete(key).await.unwrap();
        assert!(!s.exists(key).await);
        // Idempotent.
        s.delete(key).await.unwrap();
    }

    #[tokio::test]
    async fn membaca_berkas_tidak_ada_menghasilkan_not_found() {
        let (s, _g) = storage();
        s.ensure_root().await.unwrap();
        let err = s.get("faces/tidak/ada.jpg").await.unwrap_err();
        assert!(matches!(err, ApiError::NotFound(_)));
    }

    #[test]
    fn public_url_menggabungkan_base() {
        let s = Storage::new("/tmp/x", "/files");
        assert_eq!(s.public_url("faces/a.jpg"), "/files/faces/a.jpg");
    }
}
