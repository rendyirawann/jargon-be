//! Penilaian kualitas foto pendaftaran, dihitung di server.
//!
//! Tablet sudah menyaring lebih dulu, tetapi server tidak boleh mempercayai
//! klien: foto berkualitas buruk yang lolos akan merusak akurasi pengenalan
//! selamanya, dan memperbaikinya berarti memanggil ulang siswa. Karena itu
//! pemeriksaan diulang di sisi server sebelum embedding disimpan.

use serde::Serialize;
use utoipa::ToSchema;

use crate::error::{ApiError, ApiResult};

/// Ukuran minimum sisi terpendek foto wajah yang sudah di-crop.
const MIN_SIDE: u32 = 112;
const MAX_SIDE: u32 = 4096;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct QualityReport {
    /// Skor gabungan 0..1. Semakin tinggi semakin baik.
    pub score: f32,
    /// Ketajaman (variance of Laplacian) yang dinormalisasi ke 0..1.
    pub sharpness: f32,
    /// Kecerahan rata-rata 0..1. Ideal di sekitar 0,5.
    pub brightness: f32,
    /// Sebaran intensitas — foto terlalu flat biasanya backlight/overexposed.
    pub contrast: f32,
    pub width: u32,
    pub height: u32,
    /// Daftar masalah yang terdeteksi, untuk ditampilkan ke operator.
    pub issues: Vec<String>,
}

impl QualityReport {
    pub fn acceptable(&self, min_score: f32) -> bool {
        self.score >= min_score
    }
}

/// Analisis buffer gambar (JPEG/PNG).
pub fn analyze(bytes: &[u8]) -> ApiResult<QualityReport> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| ApiError::field("image", &format!("gambar tidak dapat dibaca: {e}")))?;

    let (width, height) = (img.width(), img.height());
    let mut issues = Vec::new();

    if width.min(height) < MIN_SIDE {
        issues.push(format!(
            "resolusi terlalu kecil ({width}x{height}), minimum sisi {MIN_SIDE}px"
        ));
    }
    if width.max(height) > MAX_SIDE {
        return Err(ApiError::field(
            "image",
            &format!("resolusi terlalu besar ({width}x{height})"),
        ));
    }

    let gray = img.to_luma8();
    let (w, h) = (gray.width() as usize, gray.height() as usize);
    let px = gray.as_raw();

    // Kecerahan & kontras.
    let sum: u64 = px.iter().map(|&v| v as u64).sum();
    let mean = sum as f32 / px.len() as f32;
    let variance = px
        .iter()
        .map(|&v| {
            let d = v as f32 - mean;
            d * d
        })
        .sum::<f32>()
        / px.len() as f32;
    let brightness = mean / 255.0;
    let contrast = (variance.sqrt() / 64.0).min(1.0);

    // Ketajaman: variance dari Laplacian 4-tetangga.
    let mut lap_sum = 0.0f64;
    let mut lap_sq = 0.0f64;
    let mut count = 0u32;
    if w >= 3 && h >= 3 {
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let c = px[y * w + x] as f32;
                let l = px[y * w + x - 1] as f32;
                let r = px[y * w + x + 1] as f32;
                let u = px[(y - 1) * w + x] as f32;
                let d = px[(y + 1) * w + x] as f32;
                let v = (l + r + u + d - 4.0 * c) as f64;
                lap_sum += v;
                lap_sq += v * v;
                count += 1;
            }
        }
    }
    let lap_var = if count > 0 {
        let m = lap_sum / count as f64;
        (lap_sq / count as f64 - m * m).max(0.0)
    } else {
        0.0
    };
    // 500 adalah nilai empiris "tajam" untuk crop wajah 112-224px.
    let sharpness = ((lap_var / 500.0) as f32).min(1.0);

    if sharpness < 0.25 {
        issues.push("gambar terlalu blur, minta siswa diam saat difoto".into());
    }
    if brightness < 0.22 {
        issues.push("pencahayaan terlalu gelap".into());
    } else if brightness > 0.85 {
        issues.push("pencahayaan terlalu terang / overexposed".into());
    }
    if contrast < 0.15 {
        issues.push("kontras rendah, wajah kurang terlihat jelas".into());
    }

    // Penalti kecerahan berbentuk segitiga: puncak 1.0 pada 0,5.
    let brightness_score = 1.0 - ((brightness - 0.5).abs() * 2.0);
    let resolution_score = if width.min(height) >= MIN_SIDE { 1.0 } else { 0.4 };

    let score = (0.5 * sharpness
        + 0.25 * brightness_score.clamp(0.0, 1.0)
        + 0.15 * contrast
        + 0.10 * resolution_score)
        .clamp(0.0, 1.0);

    Ok(QualityReport {
        score,
        sharpness,
        brightness,
        contrast,
        width,
        height,
        issues,
    })
}

/// Deteksi tipe MIME dari magic bytes. Tidak mempercayai header klien.
pub fn sniff_mime(bytes: &[u8]) -> ApiResult<&'static str> {
    if bytes.len() < 12 {
        return Err(ApiError::field("image", "berkas terlalu kecil"));
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Ok("image/jpeg")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Ok("image/png")
    } else {
        Err(ApiError::field(
            "image",
            "format tidak didukung, gunakan JPEG atau PNG",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_of(w: u32, h: u32, f: impl Fn(u32, u32) -> u8) -> Vec<u8> {
        let mut img = image::GrayImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, image::Luma([f(x, y)]));
            }
        }
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageLuma8(img)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn gambar_rata_dinilai_blur() {
        let bytes = png_of(128, 128, |_, _| 128);
        let r = analyze(&bytes).unwrap();
        assert!(r.sharpness < 0.05);
        assert!(!r.issues.is_empty());
        assert!(!r.acceptable(0.45));
    }

    #[test]
    fn pola_kontras_tinggi_dinilai_tajam() {
        let bytes = png_of(128, 128, |x, y| if (x / 2 + y / 2) % 2 == 0 { 20 } else { 235 });
        let r = analyze(&bytes).unwrap();
        assert!(r.sharpness > 0.5, "sharpness = {}", r.sharpness);
    }

    #[test]
    fn resolusi_kecil_dilaporkan() {
        let bytes = png_of(64, 64, |x, y| ((x * y) % 255) as u8);
        let r = analyze(&bytes).unwrap();
        assert!(r.issues.iter().any(|i| i.contains("resolusi")));
    }

    #[test]
    fn mime_dikenali_dari_magic_bytes() {
        let png = png_of(120, 120, |_, _| 100);
        assert_eq!(sniff_mime(&png).unwrap(), "image/png");
        assert!(sniff_mime(b"bukan gambar sama sekali").is_err());
    }
}
