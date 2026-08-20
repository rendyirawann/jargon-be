//! Operasi vektor untuk embedding wajah.
//!
//! Konvensi: semua embedding disimpan dan dibandingkan dalam bentuk
//! L2-normalized. Dengan begitu cosine similarity = dot product, sehingga
//! pencocokan hanya butuh satu perkalian-jumlah tanpa akar kuadrat.

use sha2::{Digest, Sha256};

use crate::error::{ApiError, ApiResult};

/// Memastikan embedding masuk akal sebelum dipakai:
/// dimensi benar, tidak ada NaN/Inf, dan norm-nya tidak nol.
pub fn validate(embedding: &[f32], expected_dim: usize) -> ApiResult<()> {
    if embedding.len() != expected_dim {
        return Err(ApiError::field(
            "embedding",
            &format!(
                "dimensi embedding harus {expected_dim}, diterima {}",
                embedding.len()
            ),
        ));
    }
    if embedding.iter().any(|v| !v.is_finite()) {
        return Err(ApiError::field("embedding", "berisi nilai NaN/Infinity"));
    }
    if norm(embedding) < 1e-6 {
        return Err(ApiError::field("embedding", "vektor nol tidak valid"));
    }
    Ok(())
}

pub fn norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Normalisasi in-place. Idempotent untuk vektor yang sudah normal.
pub fn l2_normalize(v: &mut [f32]) {
    let n = norm(v);
    if n > 1e-12 {
        let inv = 1.0 / n;
        for x in v.iter_mut() {
            *x *= inv;
        }
    }
}

pub fn normalized(v: &[f32]) -> Vec<f32> {
    let mut out = v.to_vec();
    l2_normalize(&mut out);
    out
}

/// Dot product. Di-unroll 4x agar autovectorizer LLVM menghasilkan SIMD
/// (SSE/AVX di server x86_64, NEON di ARM) tanpa `unsafe`.
#[inline]
pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let n = a.len();
    let chunks = n / 4;
    let mut s0 = 0.0f32;
    let mut s1 = 0.0f32;
    let mut s2 = 0.0f32;
    let mut s3 = 0.0f32;
    for i in 0..chunks {
        let j = i * 4;
        s0 += a[j] * b[j];
        s1 += a[j + 1] * b[j + 1];
        s2 += a[j + 2] * b[j + 2];
        s3 += a[j + 3] * b[j + 3];
    }
    let mut s = s0 + s1 + s2 + s3;
    for i in (chunks * 4)..n {
        s += a[i] * b[i];
    }
    s
}

/// Cosine similarity untuk dua vektor yang sudah dinormalisasi.
/// Hasil dijepit ke [-1, 1] agar bebas dari galat pembulatan.
#[inline]
pub fn cosine_normalized(a: &[f32], b: &[f32]) -> f32 {
    dot(a, b).clamp(-1.0, 1.0)
}

/// Hash stabil sebuah embedding, untuk deteksi replay.
///
/// Nilai dikuantisasi ke 4 desimal sebelum di-hash sehingga embedding yang
/// dikirim ulang apa adanya menghasilkan hash sama, sementara wajah yang
/// benar-benar difoto ulang (selalu sedikit berbeda) tidak.
pub fn hash_embedding(embedding: &[f32]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    for v in embedding {
        let q = (v * 10_000.0).round() as i32;
        hasher.update(q.to_le_bytes());
    }
    hasher.finalize().to_vec()
}

pub fn sha256(bytes: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalisasi_menghasilkan_norm_satu() {
        let mut v = vec![3.0, 4.0];
        l2_normalize(&mut v);
        assert!((norm(&v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn vektor_identik_similarity_satu() {
        let a = normalized(&[0.1, 0.9, -0.3, 0.5]);
        assert!((cosine_normalized(&a, &a) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn vektor_ortogonal_similarity_nol() {
        let a = normalized(&[1.0, 0.0]);
        let b = normalized(&[0.0, 1.0]);
        assert!(cosine_normalized(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn dot_konsisten_untuk_panjang_bukan_kelipatan_empat() {
        let a: Vec<f32> = (0..13).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..13).map(|i| (13 - i) as f32).collect();
        let expected: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
        assert!((dot(&a, &b) - expected).abs() < 1e-3);
    }

    #[test]
    fn hash_stabil_dan_peka_perubahan() {
        let a = vec![0.1234_f32, -0.5, 0.9];
        assert_eq!(hash_embedding(&a), hash_embedding(&a.clone()));
        let b = vec![0.1235_f32, -0.5, 0.9];
        assert_ne!(hash_embedding(&a), hash_embedding(&b));
    }

    #[test]
    fn validasi_menolak_dimensi_salah() {
        assert!(validate(&[1.0, 0.0], 3).is_err());
        assert!(validate(&[1.0, 0.0], 2).is_ok());
    }
}
