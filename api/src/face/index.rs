//! Index wajah per sekolah, di-cache di dalam proses.
//!
//! MENGAPA TIDAK LANGSUNG kNN KE POSTGRES?
//!   Populasi total 700.000+ siswa, tetapi pencocokan SELALU dibatasi pada
//!   satu sekolah — sebuah tablet di SMA N 1 Medan tidak pernah perlu
//!   membandingkan wajah dengan siswa di Nias. Satu sekolah rata-rata hanya
//!   ratusan sampai ~2.000 embedding. Untuk ukuran itu, pencarian eksak
//!   brute-force di memori (dot product 512-d, autovectorized) memakan waktu
//!   di bawah satu milidetik — lebih cepat DAN lebih akurat daripada ANN
//!   melalui jaringan ke database.
//!
//! Postgres + index HNSW tetap menjadi sumber kebenaran dan dipakai untuk
//! memuat/menyegarkan cache serta untuk verifikasi silang bila diperlukan.
//!
//! Memori: 512 dim x 4 byte = 2 KB per embedding. Seluruh provinsi
//! (700rb siswa x 3 sampel) ~ 4 GB; namun satu instance API hanya menyimpan
//! sekolah yang aktif memakainya, dan entri kedaluwarsa dibuang oleh TTL.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use sqlx::PgPool;
use uuid::Uuid;

use super::vector;
use crate::error::ApiResult;

/// Satu sampel wajah milik seorang siswa.
///
/// `embedding_id` dibawa agar hasil pencocokan bisa ditelusuri kembali ke
/// baris `face_embeddings` tertentu saat menyelidiki salah-kenal di lapangan.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Sample {
    pub student_id: Uuid,
    pub embedding_id: Uuid,
}

/// Kumpulan embedding untuk satu sekolah.
///
/// Vektor disimpan dalam satu buffer datar (`data`) agar ramah cache CPU:
/// sampel ke-i menempati `data[i*dim .. (i+1)*dim]`.
#[allow(dead_code)]
pub struct SchoolSlice {
    pub dim: usize,
    pub samples: Vec<Sample>,
    pub data: Vec<f32>,
    pub loaded_at: Instant,
    /// Versi model embedding yang termuat; dipakai saat memeriksa apakah satu
    /// sekolah masih memakai model lama setelah upgrade.
    pub model_version: String,
    /// Ambang kemiripan khusus sekolah ini, bila disetel.
    ///
    /// Ikut di-cache di sini, BUKAN diambil per scan.
    ///
    /// Sebelumnya `effective_threshold` menjalankan satu query terpisah pada
    /// SETIAP pemindaian. Pada jam puncak absensi (~520 scan/detik) itu
    /// berarti 520 query per detik untuk satu nilai yang berubah paling
    /// sering setahun sekali. Dimuat bersama slice: satu query lebih sedikit
    /// per scan, tanpa struktur cache tambahan.
    ///
    /// Harganya: perubahan ambang di dashboard baru berlaku setelah TTL
    /// slice habis (FACE_INDEX_TTL_SECS, bawaan 300 detik) atau setelah ada
    /// pendaftaran wajah di sekolah itu. Untuk nilai yang disetel sekali
    /// lalu dibiarkan, itu pertukaran yang layak.
    pub match_threshold: Option<f32>,
}

impl SchoolSlice {
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    #[inline]
    fn row(&self, i: usize) -> &[f32] {
        &self.data[i * self.dim..(i + 1) * self.dim]
    }

    /// Cari kecocokan terbaik. Mengembalikan skor terbaik per SISWA
    /// (bukan per sampel) supaya `runner_up` benar-benar berarti "siswa lain
    /// yang paling mirip", bukan sampel kedua dari siswa yang sama.
    pub fn search(&self, query: &[f32]) -> SearchOutcome {
        let mut best = Candidate::default();
        let mut runner_up = Candidate::default();

        for (i, sample) in self.samples.iter().enumerate() {
            let sim = vector::cosine_normalized(query, self.row(i));

            if sim > best.similarity {
                if sample.student_id != best.student_id {
                    // Pemenang lama turun menjadi runner-up.
                    if best.student_id != Uuid::nil() {
                        runner_up = best.clone();
                    }
                }
                best = Candidate {
                    student_id: sample.student_id,
                    embedding_id: sample.embedding_id,
                    similarity: sim,
                };
            } else if sample.student_id != best.student_id && sim > runner_up.similarity {
                runner_up = Candidate {
                    student_id: sample.student_id,
                    embedding_id: sample.embedding_id,
                    similarity: sim,
                };
            }
        }

        SearchOutcome {
            best: (best.student_id != Uuid::nil()).then_some(best),
            runner_up: (runner_up.student_id != Uuid::nil()).then_some(runner_up),
            candidates_scanned: self.samples.len(),
        }
    }
}

/// `embedding_id` menunjuk sampel mana yang menghasilkan skor tertinggi —
/// bekal utama saat menyelidiki laporan salah-kenal di lapangan.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Candidate {
    pub student_id: Uuid,
    pub embedding_id: Uuid,
    pub similarity: f32,
}

impl Default for Candidate {
    fn default() -> Self {
        Self {
            student_id: Uuid::nil(),
            embedding_id: Uuid::nil(),
            similarity: f32::NEG_INFINITY,
        }
    }
}

#[derive(Debug)]
pub struct SearchOutcome {
    pub best: Option<Candidate>,
    pub runner_up: Option<Candidate>,
    pub candidates_scanned: usize,
}

impl SearchOutcome {
    /// Selisih antara kandidat terbaik dan siswa lain terdekat.
    /// Selisih kecil = ambigu (mis. saudara kembar) -> sebaiknya ditolak.
    pub fn margin(&self) -> f32 {
        match (&self.best, &self.runner_up) {
            (Some(b), Some(r)) => b.similarity - r.similarity,
            (Some(_), None) => f32::INFINITY,
            _ => 0.0,
        }
    }
}

/// Cache index untuk seluruh sekolah yang sedang aktif.
pub struct FaceIndex {
    slices: DashMap<Uuid, Arc<SchoolSlice>>,
    ttl: Duration,
    dim: usize,
}

impl FaceIndex {
    pub fn new(dim: usize, ttl: Duration) -> Self {
        Self { slices: DashMap::new(), ttl, dim }
    }

    /// Ambil slice sekolah; muat dari database bila belum ada atau kedaluwarsa.
    pub async fn get(&self, pool: &PgPool, school_id: Uuid) -> ApiResult<Arc<SchoolSlice>> {
        if let Some(existing) = self.slices.get(&school_id) {
            if existing.loaded_at.elapsed() < self.ttl {
                return Ok(existing.clone());
            }
        }
        let fresh = Arc::new(self.load(pool, school_id).await?);
        self.slices.insert(school_id, fresh.clone());
        Ok(fresh)
    }

    /// Paksa muat ulang pada permintaan berikutnya. Dipanggil setiap kali ada
    /// pendaftaran/penghapusan wajah agar tablet langsung mengenali siswa baru.
    pub fn invalidate(&self, school_id: Uuid) {
        self.slices.remove(&school_id);
    }

    /// Buang seluruh cache. Dipakai saat versi model embedding berubah, karena
    /// vektor lintas versi tidak sebanding.
    #[allow(dead_code)]
    pub fn invalidate_all(&self) {
        self.slices.clear();
    }

    pub fn cached_schools(&self) -> usize {
        self.slices.len()
    }

    pub fn cached_samples(&self) -> usize {
        self.slices.iter().map(|e| e.value().len()).sum()
    }

    /// Buang slice yang sudah lama tidak dipakai agar memori tidak membengkak
    /// saat satu instance melayani banyak sekolah.
    pub fn evict_stale(&self, idle: Duration) -> usize {
        let mut removed = 0;
        self.slices.retain(|_, v| {
            let keep = v.loaded_at.elapsed() < idle;
            if !keep {
                removed += 1;
            }
            keep
        });
        removed
    }

    async fn load(&self, pool: &PgPool, school_id: Uuid) -> ApiResult<SchoolSlice> {
        let started = Instant::now();

        // Ambang sekolah diambil BERSAMA slice, bukan per scan. Lihat
        // catatan pada SchoolSlice::match_threshold.
        let threshold: Option<(Option<f32>,)> =
            sqlx::query_as("SELECT face_match_threshold FROM schools WHERE id = $1")
                .bind(school_id)
                .fetch_optional(pool)
                .await?;
        let match_threshold = threshold.and_then(|r| r.0);

        // ORDER BY (student_id, created_at) dijawab langsung index
        // face_embeddings_school_student_idx, sehingga tidak ada tahap sort.
        // Tanpa index itu, setiap pemuatan slice mengurutkan ulang seluruh
        // vektor sekolah — pekerjaan yang berulang setiap TTL habis, untuk
        // ribuan sekolah.
        let rows: Vec<(Uuid, Uuid, pgvector::Vector, String)> = sqlx::query_as(
            r#"
            SELECT fe.id, fe.student_id, fe.embedding, fe.model_version
            FROM face_embeddings fe
            JOIN students s ON s.id = fe.student_id
            WHERE fe.school_id = $1
              AND fe.is_active
              AND s.deleted_at IS NULL
              AND s.status = 'aktif'
            ORDER BY fe.school_id, fe.student_id, fe.created_at
            "#,
        )
        .bind(school_id)
        .fetch_all(pool)
        .await?;

        let mut samples = Vec::with_capacity(rows.len());
        let mut data = Vec::with_capacity(rows.len() * self.dim);
        let mut model_version = String::new();

        for (embedding_id, student_id, embedding, mv) in rows {
            let slice = embedding.as_slice();
            if slice.len() != self.dim {
                tracing::warn!(
                    %embedding_id, expected = self.dim, got = slice.len(),
                    "embedding dilewati: dimensi tidak sesuai"
                );
                continue;
            }
            if model_version.is_empty() {
                model_version = mv;
            }
            // Normalisasi ulang saat memuat: murah, dan menjamin invarian
            // walau ada data lama yang tersimpan tanpa normalisasi.
            let mut buf = slice.to_vec();
            vector::l2_normalize(&mut buf);
            data.extend_from_slice(&buf);
            samples.push(Sample { student_id, embedding_id });
        }

        tracing::info!(
            %school_id,
            samples = samples.len(),
            elapsed_ms = started.elapsed().as_millis(),
            "index wajah sekolah dimuat"
        );

        Ok(SchoolSlice {
            dim: self.dim,
            samples,
            data,
            loaded_at: Instant::now(),
            model_version,
            match_threshold,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slice_of(entries: &[(Uuid, Vec<f32>)]) -> SchoolSlice {
        let dim = entries[0].1.len();
        let mut samples = Vec::new();
        let mut data = Vec::new();
        for (student_id, v) in entries {
            let mut buf = v.clone();
            vector::l2_normalize(&mut buf);
            data.extend_from_slice(&buf);
            samples.push(Sample { student_id: *student_id, embedding_id: Uuid::new_v4() });
        }
        SchoolSlice {
            dim,
            samples,
            data,
            loaded_at: Instant::now(),
            model_version: "test".into(),
            match_threshold: None,
        }
    }

    #[test]
    fn memilih_siswa_paling_mirip() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let s = slice_of(&[(a, vec![1.0, 0.0, 0.0]), (b, vec![0.0, 1.0, 0.0])]);

        let q = vector::normalized(&[0.9, 0.1, 0.0]);
        let out = s.search(&q);
        assert_eq!(out.best.unwrap().student_id, a);
    }

    #[test]
    fn runner_up_selalu_siswa_yang_berbeda() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        // Dua sampel milik `a` dan satu milik `b`.
        let s = slice_of(&[
            (a, vec![1.0, 0.0, 0.0]),
            (a, vec![0.98, 0.02, 0.0]),
            (b, vec![0.0, 0.0, 1.0]),
        ]);

        let q = vector::normalized(&[1.0, 0.0, 0.0]);
        let out = s.search(&q);
        assert_eq!(out.best.as_ref().unwrap().student_id, a);
        assert_eq!(out.runner_up.as_ref().unwrap().student_id, b);
        // Margin harus besar karena b sangat berbeda.
        assert!(out.margin() > 0.5, "margin = {}", out.margin());
    }

    #[test]
    fn margin_kecil_saat_dua_siswa_mirip() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let s = slice_of(&[(a, vec![1.0, 0.0]), (b, vec![0.999, 0.045])]);
        let q = vector::normalized(&[1.0, 0.0]);
        let out = s.search(&q);
        assert!(out.margin() < 0.05, "margin = {}", out.margin());
    }

    #[test]
    fn slice_kosong_tidak_menghasilkan_kecocokan() {
        let s = SchoolSlice {
            dim: 3,
            samples: vec![],
            data: vec![],
            loaded_at: Instant::now(),
            model_version: "test".into(),
            match_threshold: None,
        };
        let out = s.search(&[1.0, 0.0, 0.0]);
        assert!(out.best.is_none());
        assert_eq!(out.candidates_scanned, 0);
    }
}
