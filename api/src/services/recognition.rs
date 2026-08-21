//! Jalur panas: mengubah satu embedding wajah menjadi satu baris absensi.
//!
//! Ini endpoint tersibuk di seluruh sistem. Pada pukul 06:30-07:15, ribuan
//! tablet di seluruh Sumatera Utara memanggilnya nyaris bersamaan. Karena itu
//! urutan pemeriksaan disusun dari yang paling murah ke paling mahal, agar
//! payload yang jelas tidak valid ditolak sebelum menyentuh database:
//!
//! ```text
//!   1. versi model cocok?          (banding string)
//!   2. dimensi & isi vektor waras? (memori)
//!   3. liveness lolos?             (banding float)
//!   4. jam perangkat masuk akal?   (aritmetika)
//!   5. nonce belum terpakai?       (Redis, 1 RTT — opsional)
//!   6. hari sekolah / bukan libur? (Postgres, ter-cache di planner)
//!   7. cocokkan wajah              (memori, index per sekolah)
//!   8. cooldown & anti-replay      (Postgres)
//!   9. tulis absensi + outbox      (satu transaksi)
//! ```

use std::time::Instant;

use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;

use crate::auth::AuthDevice;
use crate::domain::attendance::{
    AttendanceRecord, AttendanceStatus, RecognizeAction, RecognizeRequest, RecognizeResponse,
    RecognizedStudent, ScanDirection,
};
use crate::error::{ApiError, ApiResult};
use crate::face::vector;
use crate::services::notify::{self, AttendanceNotifyContext, NotifyEvent};
use crate::services::rules::{self, AutoDirection, EffectiveRule};
use crate::state::AppState;
use crate::util;

/// Ringkasan identitas siswa untuk ditampilkan di tablet & disimpan sebagai
/// snapshot pada baris absensi.
#[derive(Debug, Clone, sqlx::FromRow)]
struct StudentSnapshot {
    id: Uuid,
    full_name: String,
    nis: Option<String>,
    nisn: Option<String>,
    classroom_id: Option<Uuid>,
    classroom_name: Option<String>,
    academic_year_id: Option<Uuid>,
    school_id: Uuid,
    school_name: String,
    photo_path: Option<String>,
}

/// Hasil satu pemrosesan scan, sebelum diubah menjadi respons HTTP.
struct Outcome {
    action: RecognizeAction,
    message: String,
    student: Option<StudentSnapshot>,
    attendance: Option<AttendanceRecord>,
    similarity: Option<f32>,
    margin: Option<f32>,
    candidates_scanned: usize,
    /// Alasan teknis untuk log (`below_threshold`, `replay`, ...).
    reason: Option<&'static str>,
    event_type: &'static str,
}

impl Outcome {
    fn rejected(reason: &'static str, message: impl Into<String>) -> Self {
        Self {
            action: RecognizeAction::Rejected,
            message: message.into(),
            student: None,
            attendance: None,
            similarity: None,
            margin: None,
            candidates_scanned: 0,
            reason: Some(reason),
            event_type: "rejected",
        }
    }
}

pub async fn recognize(
    state: &AppState,
    device: &AuthDevice,
    req: RecognizeRequest,
) -> ApiResult<RecognizeResponse> {
    let started = Instant::now();
    let face_cfg = &state.cfg.face;

    // --- 1. Versi model ------------------------------------------------
    // Embedding dari model berbeda tidak sebanding sama sekali; mencocokkannya
    // akan menghasilkan identifikasi acak. Ini harus jadi pemeriksaan pertama.
    if req.model_version != face_cfg.model_version {
        return finish(
            state,
            device,
            &req,
            Outcome::rejected(
                "model_mismatch",
                format!(
                    "Versi model di perangkat ({}) berbeda dengan server ({}). Perbarui aplikasi.",
                    req.model_version, face_cfg.model_version
                ),
            ),
            started,
        )
        .await;
    }

    // --- 2. Bentuk vektor ---------------------------------------------
    vector::validate(&req.embedding, face_cfg.embedding_dim)?;
    let query = vector::normalized(&req.embedding);
    let embedding_hash = vector::hash_embedding(&query);

    // --- 3. Liveness ---------------------------------------------------
    if req.liveness_score < face_cfg.min_liveness {
        return finish(
            state,
            device,
            &req,
            Outcome::rejected(
                "liveness_failed",
                "Wajah tidak terverifikasi hidup. Hadapkan wajah langsung ke kamera, jangan foto.",
            ),
            started,
        )
        .await;
    }

    // --- 4. Jam perangkat ----------------------------------------------
    let now = Utc::now();
    let skew = (now - req.client_time).num_seconds().abs();
    if skew > face_cfg.clock_skew_tolerance.as_secs() as i64 {
        return finish(
            state,
            device,
            &req,
            Outcome::rejected(
                "clock_skew",
                "Jam perangkat tidak sinkron dengan server. Aktifkan jam otomatis pada tablet.",
            ),
            started,
        )
        .await;
    }

    // --- 5. Nonce sekali pakai -----------------------------------------
    let nonce_key = format!("{}:{}", device.id, req.nonce);
    if !state
        .claim_nonce(&nonce_key, face_cfg.clock_skew_tolerance * 2)
        .await?
    {
        return finish(
            state,
            device,
            &req,
            Outcome::rejected("replay_nonce", "Permintaan duplikat terdeteksi."),
            started,
        )
        .await;
    }

    // --- 6. Hari sekolah -----------------------------------------------
    let today = util::today_wib();
    let local_now = rules::local_time(now);
    let rule = rules::resolve_rule(&state.db, device.school_id, device.classroom_id, today).await?;

    if !rule.is_active_day(today) {
        return finish(
            state,
            device,
            &req,
            Outcome::rejected("not_school_day", "Hari ini bukan hari sekolah."),
            started,
        )
        .await;
    }
    if let Some(name) = rules::holiday_name(&state.db, device.school_id, today).await? {
        return finish(
            state,
            device,
            &req,
            Outcome::rejected("holiday", format!("Hari ini libur: {name}.")),
            started,
        )
        .await;
    }

    // --- 7. Cocokkan wajah ---------------------------------------------
    let slice = state.face_index.get(&state.db, device.school_id).await?;
    if slice.is_empty() {
        return finish(
            state,
            device,
            &req,
            Outcome::rejected(
                "no_enrollment",
                "Belum ada data wajah siswa terdaftar di sekolah ini.",
            ),
            started,
        )
        .await;
    }

    let search = slice.search(&query);
    let candidates_scanned = search.candidates_scanned;
    // Dihitung sebelum `search.best` dipindahkan keluar di bawah.
    let margin = search.margin();
    // Ambang dibaca dari slice yang SUDAH di-cache, bukan lewat query
    // tersendiri. Lihat catatan pada SchoolSlice::match_threshold: satu
    // query per scan dikalikan ~520 scan/detik adalah beban nyata untuk
    // nilai yang hampir tidak pernah berubah.
    let threshold = slice
        .match_threshold
        .unwrap_or(state.cfg.face.match_threshold);

    let Some(best) = search.best else {
        return finish(
            state,
            device,
            &req,
            Outcome {
                action: RecognizeAction::NoMatch,
                message: "Wajah tidak dikenali.".into(),
                student: None,
                attendance: None,
                similarity: None,
                margin: None,
                candidates_scanned,
                reason: Some("no_candidate"),
                event_type: "unknown",
            },
            started,
        )
        .await;
    };

    if best.similarity < threshold {
        return finish(
            state,
            device,
            &req,
            Outcome {
                action: RecognizeAction::NoMatch,
                message: "Wajah tidak dikenali. Coba lagi atau hubungi petugas.".into(),
                student: None,
                attendance: None,
                similarity: Some(best.similarity),
                margin: Some(margin),
                candidates_scanned,
                reason: Some("below_threshold"),
                event_type: "unknown",
            },
            started,
        )
        .await;
    }

    // Skor tinggi tapi selisih tipis: dua siswa sama-sama mirip (kembar,
    // saudara). Menebak berarti mencatat kehadiran orang yang salah, yang
    // lebih merugikan daripada meminta scan ulang.
    if margin.is_finite() && margin < face_cfg.match_margin {
        return finish(
            state,
            device,
            &req,
            Outcome {
                action: RecognizeAction::LowConfidence,
                message: "Wajah mirip dengan lebih dari satu siswa. Silakan absen manual ke petugas."
                    .into(),
                student: None,
                attendance: None,
                similarity: Some(best.similarity),
                margin: Some(margin),
                candidates_scanned,
                reason: Some("ambiguous_match"),
                event_type: "rejected",
            },
            started,
        )
        .await;
    }

    let Some(student) = load_student(state, best.student_id, device.school_id).await? else {
        // Index memuat siswa yang kini tidak aktif/terhapus. Buang cache agar
        // scan berikutnya sudah bersih.
        state.face_index.invalidate(device.school_id);
        return finish(
            state,
            device,
            &req,
            Outcome::rejected("student_inactive", "Data siswa tidak aktif. Hubungi petugas."),
            started,
        )
        .await;
    };

    // --- 8. Anti-replay embedding & cooldown ---------------------------
    if is_replayed_embedding(state, device.id, &embedding_hash).await? {
        return finish(
            state,
            device,
            &req,
            Outcome {
                action: RecognizeAction::Rejected,
                message: "Data wajah yang sama dikirim ulang. Lakukan pemindaian baru.".into(),
                student: Some(student),
                attendance: None,
                similarity: Some(best.similarity),
                margin: Some(margin),
                candidates_scanned,
                reason: Some("replay_embedding"),
                event_type: "rejected",
            },
            started,
        )
        .await;
    }

    // --- 9. Tentukan arah lalu tulis -----------------------------------
    let existing = load_today_attendance(state, today, student.id).await?;
    let already_checked_in = existing
        .as_ref()
        .map(|a| a.check_in_at.is_some())
        .unwrap_or(false);

    let direction = match req.direction.unwrap_or(ScanDirection::Auto) {
        ScanDirection::CheckIn => Some(AutoDirection::CheckIn),
        ScanDirection::CheckOut => Some(AutoDirection::CheckOut),
        ScanDirection::Auto => match device.mode.as_str() {
            "check_in" => Some(AutoDirection::CheckIn),
            "check_out" => Some(AutoDirection::CheckOut),
            _ => rule.auto_direction(local_now, already_checked_in),
        },
    };

    let outcome = match direction {
        None => Outcome {
            action: RecognizeAction::AlreadyRecorded,
            message: if already_checked_in {
                format!(
                    "{} sudah absen masuk hari ini pukul {}.",
                    student.full_name,
                    existing
                        .as_ref()
                        .and_then(|a| util::format_time_wib(a.check_in_at))
                        .unwrap_or_else(|| "-".into())
                )
            } else {
                "Di luar jam absensi.".to_string()
            },
            student: Some(student.clone()),
            attendance: existing.clone(),
            similarity: Some(best.similarity),
            margin: Some(margin),
            candidates_scanned,
            reason: Some("out_of_window"),
            event_type: "duplicate",
        },
        Some(AutoDirection::CheckIn) => {
            if already_checked_in {
                Outcome {
                    action: RecognizeAction::AlreadyRecorded,
                    message: format!(
                        "{} sudah absen masuk pukul {}.",
                        student.full_name,
                        existing
                            .as_ref()
                            .and_then(|a| util::format_time_wib(a.check_in_at))
                            .unwrap_or_else(|| "-".into())
                    ),
                    student: Some(student.clone()),
                    attendance: existing.clone(),
                    similarity: Some(best.similarity),
                    margin: Some(margin),
                    candidates_scanned,
                    reason: Some("already_checked_in"),
                    event_type: "duplicate",
                }
            } else {
                let record = write_check_in(
                    state,
                    device,
                    &student,
                    &rule,
                    today,
                    now,
                    best.similarity,
                )
                .await?;
                let status = AttendanceStatus::parse(&record.status).unwrap_or(AttendanceStatus::Hadir);
                Outcome {
                    action: RecognizeAction::CheckedIn,
                    message: greeting(&student.full_name, status, record.late_minutes),
                    student: Some(student.clone()),
                    attendance: Some(record),
                    similarity: Some(best.similarity),
                    margin: Some(margin),
                    candidates_scanned,
                    reason: None,
                    event_type: "check_in",
                }
            }
        }
        Some(AutoDirection::CheckOut) => {
            if !already_checked_in {
                Outcome {
                    action: RecognizeAction::Rejected,
                    message: format!(
                        "{} belum tercatat absen masuk hari ini. Hubungi petugas.",
                        student.full_name
                    ),
                    student: Some(student.clone()),
                    attendance: existing.clone(),
                    similarity: Some(best.similarity),
                    margin: Some(margin),
                    candidates_scanned,
                    reason: Some("check_out_without_check_in"),
                    event_type: "rejected",
                }
            } else if existing.as_ref().and_then(|a| a.check_out_at).is_some() {
                Outcome {
                    action: RecognizeAction::AlreadyRecorded,
                    message: format!("{} sudah absen pulang.", student.full_name),
                    student: Some(student.clone()),
                    attendance: existing.clone(),
                    similarity: Some(best.similarity),
                    margin: Some(margin),
                    candidates_scanned,
                    reason: Some("already_checked_out"),
                    event_type: "duplicate",
                }
            } else {
                let record =
                    write_check_out(state, device, &student, today, now, best.similarity).await?;
                Outcome {
                    action: RecognizeAction::CheckedOut,
                    message: format!(
                        "Selamat jalan, {}. Absen pulang pukul {}.",
                        first_name(&student.full_name),
                        util::format_time_wib(record.check_out_at).unwrap_or_default()
                    ),
                    student: Some(student.clone()),
                    attendance: Some(record),
                    similarity: Some(best.similarity),
                    margin: Some(margin),
                    candidates_scanned,
                    reason: None,
                    event_type: "check_out",
                }
            }
        }
    };

    finish(state, device, &req, outcome, started).await
}

// =====================================================================
// Penulisan absensi
// =====================================================================

async fn write_check_in(
    state: &AppState,
    device: &AuthDevice,
    student: &StudentSnapshot,
    rule: &EffectiveRule,
    date: NaiveDate,
    now: DateTime<Utc>,
    similarity: f32,
) -> ApiResult<AttendanceRecord> {
    let (status, late_minutes) = rule.classify_check_in(rules::local_time(now));

    let mut tx = state.db.begin().await?;

    // Upsert dengan penjaga `check_in_at IS NULL`: bila dua tablet memindai
    // siswa yang sama pada saat yang sama, hanya satu yang menang dan yang
    // kedua tidak menimpa jam masuk.
    let row: Option<(Uuid,)> = sqlx::query_as(
        r#"
        INSERT INTO attendances (
            attendance_date, school_id, student_id, classroom_id, academic_year_id,
            student_name, student_nis, classroom_name, school_name,
            check_in_at, status, late_minutes,
            check_in_method, check_in_device_id, check_in_similarity
        ) VALUES (
            $1, $2, $3, $4, $5,
            $6, $7, $8, $9,
            $10, $11, $12,
            'face', $13, $14
        )
        ON CONFLICT (attendance_date, student_id) DO UPDATE SET
            check_in_at         = EXCLUDED.check_in_at,
            status              = EXCLUDED.status,
            late_minutes        = EXCLUDED.late_minutes,
            check_in_method     = EXCLUDED.check_in_method,
            check_in_device_id  = EXCLUDED.check_in_device_id,
            check_in_similarity = EXCLUDED.check_in_similarity,
            classroom_id        = COALESCE(attendances.classroom_id, EXCLUDED.classroom_id),
            classroom_name      = COALESCE(attendances.classroom_name, EXCLUDED.classroom_name),
            updated_at          = NOW()
        WHERE attendances.check_in_at IS NULL
        RETURNING id
        "#,
    )
    .bind(date)
    .bind(student.school_id)
    .bind(student.id)
    .bind(student.classroom_id)
    .bind(student.academic_year_id)
    .bind(&student.full_name)
    .bind(&student.nis)
    .bind(&student.classroom_name)
    .bind(&student.school_name)
    .bind(now)
    .bind(status.as_str())
    .bind(late_minutes)
    .bind(device.id)
    .bind(similarity)
    .fetch_optional(&mut *tx)
    .await?;

    let attendance_id = match row {
        Some((id,)) => id,
        None => {
            // Perlombaan kalah: baris sudah punya jam masuk. Ambil apa adanya.
            tx.rollback().await?;
            return load_today_attendance(state, date, student.id)
                .await?
                .ok_or_else(|| ApiError::Conflict("absensi sudah tercatat".into()));
        }
    };

    let ctx = AttendanceNotifyContext {
        school_id: student.school_id,
        school_name: student.school_name.clone(),
        student_id: student.id,
        student_name: student.full_name.clone(),
        student_nis: student.nis.clone(),
        classroom_name: student.classroom_name.clone(),
        attendance_id,
        attendance_date: date,
        status,
        check_in_at: Some(now),
        check_out_at: None,
        late_minutes,
    };
    if state.cfg.notify.enabled {
        notify::enqueue_attendance(&mut tx, &ctx, NotifyEvent::CheckIn).await?;
    }

    tx.commit().await?;

    load_today_attendance(state, date, student.id)
        .await?
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("absensi hilang setelah commit")))
}

async fn write_check_out(
    state: &AppState,
    device: &AuthDevice,
    student: &StudentSnapshot,
    date: NaiveDate,
    now: DateTime<Utc>,
    similarity: f32,
) -> ApiResult<AttendanceRecord> {
    let mut tx = state.db.begin().await?;

    let row: Option<(Uuid, String)> = sqlx::query_as(
        r#"
        UPDATE attendances SET
            check_out_at         = $3,
            check_out_method     = 'face',
            check_out_device_id  = $4,
            check_out_similarity = $5,
            duration_minutes     = GREATEST(0, EXTRACT(EPOCH FROM ($3 - check_in_at))::int / 60),
            updated_at           = NOW()
        WHERE attendance_date = $1
          AND student_id = $2
          AND check_out_at IS NULL
          AND check_in_at IS NOT NULL
        RETURNING id, status
        "#,
    )
    .bind(date)
    .bind(student.id)
    .bind(now)
    .bind(device.id)
    .bind(similarity)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((attendance_id, status_text)) = row else {
        tx.rollback().await?;
        return load_today_attendance(state, date, student.id)
            .await?
            .ok_or_else(|| ApiError::Conflict("absen pulang tidak dapat dicatat".into()));
    };

    let status = AttendanceStatus::parse(&status_text).unwrap_or(AttendanceStatus::Hadir);
    let ctx = AttendanceNotifyContext {
        school_id: student.school_id,
        school_name: student.school_name.clone(),
        student_id: student.id,
        student_name: student.full_name.clone(),
        student_nis: student.nis.clone(),
        classroom_name: student.classroom_name.clone(),
        attendance_id,
        attendance_date: date,
        status,
        check_in_at: None,
        check_out_at: Some(now),
        late_minutes: 0,
    };
    if state.cfg.notify.enabled {
        notify::enqueue_attendance(&mut tx, &ctx, NotifyEvent::CheckOut).await?;
    }

    tx.commit().await?;

    load_today_attendance(state, date, student.id)
        .await?
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("absensi hilang setelah commit")))
}

// =====================================================================
// Query pendukung
// =====================================================================

async fn load_student(
    state: &AppState,
    student_id: Uuid,
    school_id: Uuid,
) -> ApiResult<Option<StudentSnapshot>> {
    let row: Option<StudentSnapshot> = sqlx::query_as(
        r#"
        SELECT s.id, s.full_name, s.nis, s.nisn,
               s.current_classroom_id AS classroom_id,
               c.name  AS classroom_name,
               c.academic_year_id,
               s.school_id,
               sc.name AS school_name,
               s.photo_path
        FROM students s
        JOIN schools sc ON sc.id = s.school_id
        LEFT JOIN classrooms c ON c.id = s.current_classroom_id
        WHERE s.id = $1
          AND s.school_id = $2
          AND s.deleted_at IS NULL
          AND s.status = 'aktif'
        "#,
    )
    .bind(student_id)
    .bind(school_id)
    .fetch_optional(&state.db)
    .await?;
    Ok(row)
}

pub async fn load_today_attendance(
    state: &AppState,
    date: NaiveDate,
    student_id: Uuid,
) -> ApiResult<Option<AttendanceRecord>> {
    let row: Option<AttendanceRecord> = sqlx::query_as(
        r#"
        SELECT id, attendance_date, school_id, school_name, student_id, student_name,
               student_nis, classroom_id, classroom_name, check_in_at, check_out_at,
               status, late_minutes, duration_minutes, check_in_method, check_out_method,
               notes, notification_status
        FROM attendances
        WHERE attendance_date = $1 AND student_id = $2
        "#,
    )
    .bind(date)
    .bind(student_id)
    .fetch_optional(&state.db)
    .await?;
    Ok(row)
}


/// Apakah embedding identik pernah dikirim perangkat ini dalam 10 menit
/// terakhir? Embedding wajah asli selalu sedikit berbeda tiap frame, jadi
/// hash yang sama persis berarti payload lama diputar ulang.
async fn is_replayed_embedding(
    state: &AppState,
    device_id: Uuid,
    hash: &[u8],
) -> ApiResult<bool> {
    let row: Option<(i32,)> = sqlx::query_as(
        r#"
        SELECT 1 FROM attendance_events
        WHERE device_id = $1
          AND embedding_hash = $2
          AND occurred_at > NOW() - INTERVAL '10 minutes'
        LIMIT 1
        "#,
    )
    .bind(device_id)
    .bind(hash)
    .fetch_optional(&state.db)
    .await?;
    Ok(row.is_some())
}

// =====================================================================
// Log & respons
// =====================================================================

async fn finish(
    state: &AppState,
    device: &AuthDevice,
    req: &RecognizeRequest,
    outcome: Outcome,
    started: Instant,
) -> ApiResult<RecognizeResponse> {
    let elapsed = started.elapsed();
    let latency_ms = elapsed.as_millis() as i32;

    let outcome_label = match outcome.action {
        RecognizeAction::CheckedIn | RecognizeAction::CheckedOut => "accepted",
        RecognizeAction::AlreadyRecorded => "ignored",
        _ => "rejected",
    };

    // Hash tetap dicatat (untuk anti-replay), embedding-nya TIDAK.
    let embedding_hash = if req.embedding.len() == state.cfg.face.embedding_dim {
        Some(vector::hash_embedding(&vector::normalized(&req.embedding)))
    } else {
        None
    };

    let log = sqlx::query(
        r#"
        INSERT INTO attendance_events (
            school_id, device_id, student_id, attendance_id,
            event_type, outcome, reason, similarity, liveness_score,
            model_version, embedding_hash, client_time, latency_ms
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
        "#,
    )
    .bind(device.school_id)
    .bind(device.id)
    .bind(outcome.student.as_ref().map(|s| s.id))
    .bind(outcome.attendance.as_ref().map(|a| a.id))
    .bind(outcome.event_type)
    .bind(outcome_label)
    .bind(outcome.reason)
    .bind(outcome.similarity)
    .bind(req.liveness_score)
    .bind(&req.model_version)
    .bind(embedding_hash)
    .bind(req.client_time)
    .bind(latency_ms)
    .execute(&state.db)
    .await;

    // Kegagalan menulis log tidak boleh membatalkan absensi yang sudah commit.
    if let Err(e) = log {
        tracing::error!(error = %e, "gagal menulis attendance_events");
    }

    // Perangkat terlihat aktif.
    let pool = state.db.clone();
    let device_id = device.id;
    tokio::spawn(async move {
        let _ = sqlx::query("UPDATE devices SET last_seen_at = NOW() WHERE id = $1")
            .bind(device_id)
            .execute(&pool)
            .await;
    });

    Ok(RecognizeResponse {
        matched: matches!(
            outcome.action,
            RecognizeAction::CheckedIn
                | RecognizeAction::CheckedOut
                | RecognizeAction::AlreadyRecorded
        ),
        action: outcome.action,
        message: outcome.message,
        student: outcome.student.map(|s| RecognizedStudent {
            id: s.id,
            full_name: s.full_name,
            nis: s.nis,
            nisn: s.nisn,
            classroom_id: s.classroom_id,
            classroom_name: s.classroom_name,
            school_id: s.school_id,
            school_name: s.school_name,
            photo_url: s.photo_path.map(|p| state.storage.public_url(&p)),
        }),
        attendance: outcome.attendance,
        similarity: outcome.similarity,
        margin: outcome.margin.filter(|m| m.is_finite()),
        candidates_scanned: outcome.candidates_scanned,
        processing_ms: elapsed.as_millis() as u64,
    })
}

/// Sapaan yang tampil di layar tablet.
fn greeting(full_name: &str, status: AttendanceStatus, late_minutes: i32) -> String {
    let nama = first_name(full_name);
    match status {
        AttendanceStatus::Terlambat => format!(
            "Halo {nama}, absen tercatat. Kamu terlambat {late_minutes} menit."
        ),
        AttendanceStatus::Alfa => format!(
            "Halo {nama}, absen tercatat di luar jam masuk. Segera lapor ke petugas."
        ),
        _ => format!("Selamat pagi, {nama}. Absen berhasil!"),
    }
}

/// Ambil satu-dua kata pertama nama agar pas di layar tablet.
fn first_name(full_name: &str) -> String {
    let mut parts = full_name.split_whitespace();
    match (parts.next(), parts.next()) {
        (Some(a), Some(b)) if a.chars().count() <= 3 => format!("{a} {b}"),
        (Some(a), _) => a.to_string(),
        _ => full_name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nama_panggilan_mengambil_kata_pertama() {
        assert_eq!(first_name("Budi Santoso Wijaya"), "Budi");
        // Nama pendek digabung dengan kata berikutnya agar tidak ambigu.
        assert_eq!(first_name("Sri Wahyuni Lubis"), "Sri Wahyuni");
        assert_eq!(first_name("Aisyah"), "Aisyah");
        assert_eq!(first_name(""), "");
    }

    #[test]
    fn sapaan_menyebut_keterlambatan() {
        let msg = greeting("Budi Santoso", AttendanceStatus::Terlambat, 7);
        assert!(msg.contains("Budi"));
        assert!(msg.contains("7 menit"));
    }

    #[test]
    fn sapaan_hadir_positif() {
        let msg = greeting("Budi Santoso", AttendanceStatus::Hadir, 0);
        assert!(msg.contains("berhasil"));
        assert!(!msg.contains("terlambat"));
    }

    #[test]
    fn outcome_rejected_membawa_alasan() {
        let o = Outcome::rejected("holiday", "Hari ini libur.");
        assert_eq!(o.reason, Some("holiday"));
        assert_eq!(o.event_type, "rejected");
        assert!(matches!(o.action, RecognizeAction::Rejected));
        assert!(o.student.is_none());
    }
}
