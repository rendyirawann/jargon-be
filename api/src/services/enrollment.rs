//! Pendaftaran wajah siswa.
//!
//! Satu-satunya tempat gambar wajah masuk ke sistem. Aturan yang ditegakkan:
//!
//! * Gambar diperiksa ulang kualitasnya di server (klien tidak dipercaya).
//! * Embedding harus berasal dari versi model yang sama dengan server.
//! * Sampel baru dibandingkan dengan sampel siswa itu sendiri; nilai yang
//!   terlalu rendah berarti kemungkinan besar salah orang, dan ditolak.
//! * Sampel baru juga dibandingkan dengan SISWA LAIN di sekolah yang sama;
//!   kemiripan sangat tinggi berarti wajah ini sudah terdaftar atas nama
//!   orang lain — biasanya karena operator salah memilih siswa.

use base64::Engine as _;
use uuid::Uuid;

use crate::domain::face::{EnrollFaceRequest, EnrollFaceResponse, RECOMMENDED_SAMPLES};
use crate::error::{ApiError, ApiResult};
use crate::face::{quality, vector};
use crate::services::storage::Storage;
use crate::state::AppState;

/// Kemiripan minimum terhadap sampel siswa yang sudah ada.
/// Di bawah ini, foto kemungkinan bukan orang yang sama.
const MIN_SELF_SIMILARITY: f32 = 0.45;
/// Kemiripan terhadap siswa LAIN yang dianggap tabrakan identitas.
const CROSS_STUDENT_CONFLICT: f32 = 0.80;

pub struct EnrollActor {
    pub user_id: Option<Uuid>,
    pub device_id: Option<Uuid>,
}

pub async fn enroll(
    state: &AppState,
    student_id: Uuid,
    school_id: Uuid,
    actor: EnrollActor,
    req: EnrollFaceRequest,
) -> ApiResult<EnrollFaceResponse> {
    let face_cfg = &state.cfg.face;

    if req.model_version != face_cfg.model_version {
        return Err(ApiError::field(
            "model_version",
            &format!(
                "versi model perangkat ({}) berbeda dengan server ({})",
                req.model_version, face_cfg.model_version
            ),
        ));
    }
    if !crate::domain::face::FACE_POSES.contains(&req.pose.as_str()) {
        return Err(ApiError::field(
            "pose",
            &format!("pose harus salah satu dari: {}", crate::domain::face::FACE_POSES.join(", ")),
        ));
    }

    vector::validate(&req.embedding, face_cfg.embedding_dim)?;
    let embedding = vector::normalized(&req.embedding);

    // --- Gambar --------------------------------------------------------
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(strip_data_uri(&req.image_base64))
        .map_err(|_| ApiError::field("image_base64", "bukan base64 yang valid"))?;

    if bytes.len() > state.cfg.max_upload_bytes {
        return Err(ApiError::field(
            "image_base64",
            &format!(
                "ukuran gambar melebihi batas {} KB",
                state.cfg.max_upload_bytes / 1024
            ),
        ));
    }

    let mime = quality::sniff_mime(&bytes)?;
    let report = quality::analyze(&bytes)?;
    if !report.acceptable(face_cfg.min_enroll_quality) {
        return Err(ApiError::validation(vec![crate::error::FieldError::new(
            "image_base64",
            format!(
                "kualitas foto belum memadai (skor {:.2}, minimum {:.2}). {}",
                report.score,
                face_cfg.min_enroll_quality,
                if report.issues.is_empty() {
                    "Ulangi pengambilan foto.".to_string()
                } else {
                    report.issues.join("; ")
                }
            ),
        )]));
    }

    // --- Verifikasi siswa & tenant -------------------------------------
    let student: Option<(Uuid, String, Uuid)> = sqlx::query_as(
        r#"
        SELECT id, full_name, school_id FROM students
        WHERE id = $1 AND school_id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(student_id)
    .bind(school_id)
    .fetch_optional(&state.db)
    .await?;
    let (student_id, student_name, school_id) =
        student.ok_or_else(|| ApiError::NotFound(format!("siswa `{student_id}`")))?;

    // --- Konsistensi biometrik -----------------------------------------
    let slice = state.face_index.get(&state.db, school_id).await?;
    let search = slice.search(&embedding);

    let mut self_similarity = None;
    if let Some(best) = &search.best {
        if best.student_id == student_id {
            self_similarity = Some(best.similarity);
        } else if best.similarity >= CROSS_STUDENT_CONFLICT {
            let other: Option<(String,)> =
                sqlx::query_as("SELECT full_name FROM students WHERE id = $1")
                    .bind(best.student_id)
                    .fetch_optional(&state.db)
                    .await?;
            let other_name = other.map(|r| r.0).unwrap_or_else(|| "siswa lain".into());
            return Err(ApiError::Conflict(format!(
                "Wajah ini sangat mirip dengan data milik {other_name} (kemiripan {:.0}%). \
                 Pastikan Anda memilih siswa yang benar.",
                best.similarity * 100.0
            )));
        }
    }

    // Bila siswa sudah punya sampel, sampel baru harus mirip dengan dirinya.
    let existing_count = slice
        .samples
        .iter()
        .filter(|s| s.student_id == student_id)
        .count();
    if existing_count > 0 {
        let own_best = own_similarity(&slice, student_id, &embedding);
        self_similarity = Some(own_best);
        if own_best < MIN_SELF_SIMILARITY {
            return Err(ApiError::Conflict(format!(
                "Foto ini hanya {:.0}% mirip dengan data wajah {student_name} yang sudah ada. \
                 Kemungkinan foto orang berbeda — periksa kembali.",
                own_best * 100.0
            )));
        }
    }

    // --- Simpan --------------------------------------------------------
    let ext = if mime == "image/png" { "png" } else { "jpg" };
    let image_key = Storage::face_key(school_id, student_id, ext);
    let sha = vector::sha256(&bytes);

    // Berkas ditulis lebih dulu; bila transaksi gagal, berkas yatim dibersihkan.
    state.storage.put(&image_key, &bytes).await?;

    let result = persist(
        state,
        student_id,
        school_id,
        &image_key,
        &sha,
        bytes.len() as i32,
        mime,
        &req.pose,
        report.score,
        &report,
        &embedding,
        &actor,
    )
    .await;

    let (enrollment_id, embedding_id, sample_count) = match result {
        Ok(v) => v,
        Err(e) => {
            let _ = state.storage.delete(&image_key).await;
            return Err(e);
        }
    };

    // Tablet harus segera bisa mengenali siswa ini.
    state.broadcast_face_invalidation(school_id).await;

    let ready = sample_count >= 1;
    let message = if sample_count >= RECOMMENDED_SAMPLES {
        format!(
            "Data wajah {student_name} lengkap ({sample_count} sampel). Siswa sudah bisa absen."
        )
    } else {
        format!(
            "Sampel ke-{sample_count} tersimpan. Tambahkan {} sampel lagi dari sudut berbeda untuk akurasi terbaik.",
            RECOMMENDED_SAMPLES - sample_count
        )
    };

    Ok(EnrollFaceResponse {
        enrollment_id,
        embedding_id,
        student_id,
        sample_count,
        ready,
        quality: report,
        self_similarity,
        message,
    })
}

#[allow(clippy::too_many_arguments)]
async fn persist(
    state: &AppState,
    student_id: Uuid,
    school_id: Uuid,
    image_key: &str,
    sha: &[u8],
    bytes_len: i32,
    mime: &str,
    pose: &str,
    quality_score: f32,
    report: &quality::QualityReport,
    embedding: &[f32],
    actor: &EnrollActor,
) -> ApiResult<(Uuid, Uuid, i16)> {
    let mut tx = state.db.begin().await?;

    let (enrollment_id,): (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO face_enrollments (
            student_id, school_id, image_key, image_sha256, image_bytes, mime_type,
            pose, quality_score, quality_detail, status, captured_by, device_id
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,'approved',$10,$11)
        RETURNING id
        "#,
    )
    .bind(student_id)
    .bind(school_id)
    .bind(image_key)
    .bind(sha)
    .bind(bytes_len)
    .bind(mime)
    .bind(pose)
    .bind(quality_score)
    .bind(serde_json::to_value(report).unwrap_or(serde_json::Value::Null))
    .bind(actor.user_id)
    .bind(actor.device_id)
    .fetch_one(&mut *tx)
    .await?;

    let (embedding_id,): (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO face_embeddings (
            student_id, school_id, enrollment_id, embedding, model_version, quality_score
        ) VALUES ($1,$2,$3,$4,$5,$6)
        RETURNING id
        "#,
    )
    .bind(student_id)
    .bind(school_id)
    .bind(enrollment_id)
    .bind(pgvector::Vector::from(embedding.to_vec()))
    .bind(&state.cfg.face.model_version)
    .bind(quality_score)
    .fetch_one(&mut *tx)
    .await?;

    // Trigger `sync_student_face_summary` sudah memperbarui students; baca
    // hasilnya agar respons konsisten dengan kondisi setelah commit.
    let (sample_count,): (i16,) =
        sqlx::query_as("SELECT face_sample_count FROM students WHERE id = $1")
            .bind(student_id)
            .fetch_one(&mut *tx)
            .await?;

    tx.commit().await?;
    Ok((enrollment_id, embedding_id, sample_count))
}

/// Kemiripan tertinggi terhadap sampel milik siswa itu sendiri.
fn own_similarity(
    slice: &crate::face::SchoolSlice,
    student_id: Uuid,
    query: &[f32],
) -> f32 {
    let dim = slice.dim;
    slice
        .samples
        .iter()
        .enumerate()
        .filter(|(_, s)| s.student_id == student_id)
        .map(|(i, _)| vector::cosine_normalized(query, &slice.data[i * dim..(i + 1) * dim]))
        .fold(f32::NEG_INFINITY, f32::max)
        .max(-1.0)
}

/// Hapus satu sampel wajah beserta berkasnya.
pub async fn delete_sample(
    state: &AppState,
    enrollment_id: Uuid,
    school_id: Uuid,
) -> ApiResult<()> {
    let row: Option<(String, Uuid)> = sqlx::query_as(
        r#"
        SELECT image_key, student_id FROM face_enrollments
        WHERE id = $1 AND school_id = $2
        "#,
    )
    .bind(enrollment_id)
    .bind(school_id)
    .fetch_optional(&state.db)
    .await?;

    let (image_key, _student_id) =
        row.ok_or_else(|| ApiError::NotFound(format!("data wajah `{enrollment_id}`")))?;

    // Cascade pada face_embeddings ikut menghapus vektornya, dan trigger
    // memperbarui ringkasan pada students.
    sqlx::query("DELETE FROM face_enrollments WHERE id = $1")
        .bind(enrollment_id)
        .execute(&state.db)
        .await?;

    let _ = state.storage.delete(&image_key).await;
    state.broadcast_face_invalidation(school_id).await;
    Ok(())
}

/// Buang prefix `data:image/jpeg;base64,` bila klien mengirimkannya.
fn strip_data_uri(input: &str) -> &str {
    let trimmed = input.trim();
    match trimmed.find(";base64,") {
        Some(idx) if trimmed.starts_with("data:") => &trimmed[idx + 8..],
        _ => trimmed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::face::index::{Sample, SchoolSlice};
    use std::time::Instant;

    #[test]
    fn data_uri_dibuang() {
        assert_eq!(strip_data_uri("data:image/jpeg;base64,QUJD"), "QUJD");
        assert_eq!(strip_data_uri("data:image/png;base64,QUJD"), "QUJD");
        assert_eq!(strip_data_uri("  QUJD  "), "QUJD");
        // String yang mengandung ";base64," tapi bukan data URI dibiarkan.
        assert_eq!(strip_data_uri("abc;base64,def"), "abc;base64,def");
    }

    fn slice_with(entries: &[(Uuid, Vec<f32>)]) -> SchoolSlice {
        let dim = entries[0].1.len();
        let mut samples = Vec::new();
        let mut data = Vec::new();
        for (sid, v) in entries {
            let mut buf = v.clone();
            vector::l2_normalize(&mut buf);
            data.extend_from_slice(&buf);
            samples.push(Sample { student_id: *sid, embedding_id: Uuid::new_v4() });
        }
        SchoolSlice { dim, samples, data, loaded_at: Instant::now(), model_version: "t".into() }
    }

    #[test]
    fn kemiripan_diri_mengabaikan_siswa_lain() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let slice = slice_with(&[
            (a, vec![1.0, 0.0, 0.0]),
            (b, vec![0.0, 1.0, 0.0]), // sangat mirip query, tapi siswa lain
        ]);

        let q = vector::normalized(&[0.0, 1.0, 0.0]);
        // Terhadap `a` seharusnya ~0, bukan ~1.
        let sim = own_similarity(&slice, a, &q);
        assert!(sim.abs() < 1e-5, "sim = {sim}");
    }

    #[test]
    fn kemiripan_diri_mengambil_sampel_terbaik() {
        let a = Uuid::new_v4();
        let slice = slice_with(&[
            (a, vec![0.0, 1.0, 0.0]),
            (a, vec![1.0, 0.0, 0.0]),
        ]);
        let q = vector::normalized(&[1.0, 0.0, 0.0]);
        assert!((own_similarity(&slice, a, &q) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn kemiripan_diri_tanpa_sampel_tidak_minus_infinity() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let slice = slice_with(&[(b, vec![1.0, 0.0])]);
        let sim = own_similarity(&slice, a, &vector::normalized(&[1.0, 0.0]));
        assert_eq!(sim, -1.0, "harus dijepit, bukan -inf");
    }

    #[test]
    fn ambang_konflik_lebih_tinggi_dari_ambang_diri() {
        // Invarian desain: menolak "mirip siswa lain" harus lebih ketat
        // daripada menerima "mirip diri sendiri".
        assert!(CROSS_STUDENT_CONFLICT > MIN_SELF_SIMILARITY);
    }
}
