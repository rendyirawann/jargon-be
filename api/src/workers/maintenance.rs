//! Worker pemeliharaan harian.
//!
//! Empat tugas yang harus tetap berjalan tanpa campur tangan operator:
//!
//! 1. **Partisi** — membuat partisi bulan berikutnya sebelum dibutuhkan.
//!    Tanpa ini, INSERT absensi awal bulan akan jatuh ke partisi DEFAULT dan
//!    perlahan melambat.
//! 2. **Tandai alfa** — siswa yang tidak pernah discan hari itu belum punya
//!    baris absensi sama sekali; tanpa langkah ini orang tuanya tidak pernah
//!    diberi tahu dan rekap bulanan menghitungnya sebagai "tidak ada data".
//! 3. **Rollup** — mengisi `attendance_daily_summary` agar dashboard tidak
//!    perlu memindai tabel 160 juta baris.
//! 4. **Bersih-bersih** — buang token kedaluwarsa, heartbeat lama, dan
//!    kunci idempotensi yang sudah lewat.

use std::time::Duration;

use chrono::{NaiveTime, Timelike};
use uuid::Uuid;

use crate::services::notify::{self, AttendanceNotifyContext, NotifyEvent};
use crate::services::reports;
use crate::state::AppState;
use crate::util;

/// Seberapa sering worker bangun untuk memeriksa apakah ada tugas jatuh tempo.
const TICK: Duration = Duration::from_secs(60);

pub async fn run(state: AppState, shutdown: tokio::sync::watch::Receiver<bool>) {
    tracing::info!("worker pemeliharaan berjalan");

    // Sekali di awal, agar instance baru langsung punya partisi yang benar.
    if let Err(e) = ensure_partitions(&state).await {
        tracing::error!(error = ?e, "gagal menyiapkan partisi saat start");
    }

    let mut last_absentee_run: Option<chrono::NaiveDate> = None;
    let mut last_rollup_hour: Option<u32> = None;
    let mut shutdown = shutdown;

    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    tracing::info!("worker pemeliharaan berhenti");
                    return;
                }
            }
            _ = tokio::time::sleep(TICK) => {
                let now = util::now_wib();
                let today = now.date_naive();

                // Tandai alfa setelah jam yang ditetapkan tiap sekolah.
                match mark_absentees_due(&state, last_absentee_run).await {
                    Ok(ran) if ran => last_absentee_run = Some(today),
                    Ok(_) => {}
                    Err(e) => tracing::error!(error = ?e, "gagal menandai siswa alfa"),
                }

                // Rollup tiap jam.
                if last_rollup_hour != Some(now.hour()) {
                    if let Err(e) = reports::rebuild_daily_rollup(&state.db, today).await {
                        tracing::error!(error = ?e, "gagal membangun rollup harian");
                    } else {
                        last_rollup_hour = Some(now.hour());
                    }
                }

                // Tugas tengah malam.
                if now.hour() == 0 && now.minute() < 2 {
                    if let Err(e) = ensure_partitions(&state).await {
                        tracing::error!(error = ?e, "gagal membuat partisi");
                    }
                    if let Err(e) = cleanup(&state).await {
                        tracing::error!(error = ?e, "gagal bersih-bersih");
                    }
                    // Rollup ulang hari sebelumnya setelah semua koreksi masuk.
                    let yesterday = today.pred_opt().unwrap_or(today);
                    if let Err(e) = reports::rebuild_daily_rollup(&state.db, yesterday).await {
                        tracing::error!(error = ?e, "gagal rollup hari sebelumnya");
                    }
                    // Buang index wajah yang lama tidak dipakai.
                    let evicted = state.face_index.evict_stale(Duration::from_secs(6 * 3600));
                    if evicted > 0 {
                        tracing::info!(evicted, "index wajah sekolah yang menganggur dibuang");
                    }
                }
            }
        }
    }
}

async fn ensure_partitions(state: &AppState) -> anyhow::Result<()> {
    sqlx::query("SELECT ensure_attendance_partitions(3)")
        .execute(&state.db)
        .await?;
    sqlx::query("SELECT ensure_outbox_partitions(3)")
        .execute(&state.db)
        .await?;
    tracing::info!("partisi bulanan dipastikan tersedia");
    Ok(())
}

/// Untuk setiap sekolah yang jam batas absennya sudah lewat, tandai siswa
/// yang belum absen sebagai `alfa` dan antrikan notifikasi ke wali.
///
/// `last_run` mencegah pekerjaan sama diulang berkali-kali dalam sehari.
async fn mark_absentees_due(
    state: &AppState,
    last_run: Option<chrono::NaiveDate>,
) -> anyhow::Result<bool> {
    let now = util::now_wib();
    let today = now.date_naive();

    if last_run == Some(today) {
        return Ok(false);
    }

    // Sekolah yang jam `absent_notify_after`-nya sudah terlewati.
    let due: Vec<(Uuid, String, NaiveTime)> = sqlx::query_as(
        r#"
        SELECT s.id, s.name, COALESCE(p.absent_notify_after, TIME '09:30')
        FROM schools s
        LEFT JOIN notification_policies p ON p.school_id = s.id
        WHERE s.is_active AND s.deleted_at IS NULL
          AND COALESCE(p.absent_notify_after, TIME '09:30') <= $1
        "#,
    )
    .bind(now.time())
    .fetch_all(&state.db)
    .await?;

    if due.is_empty() {
        return Ok(false);
    }

    let mut total_marked = 0u64;
    let mut schools_done = 0usize;

    for (school_id, school_name, _) in due {
        let rule =
            crate::services::rules::resolve_rule(&state.db, school_id, None, today).await?;
        if !rule.is_active_day(today) {
            continue;
        }
        if crate::services::rules::holiday_name(&state.db, school_id, today)
            .await?
            .is_some()
        {
            continue;
        }

        let marked = reports::mark_absentees(&state.db, school_id, today).await?;
        total_marked += marked;
        schools_done += 1;

        if marked > 0 && state.cfg.notify.enabled {
            if let Err(e) = notify_absentees(state, school_id, &school_name, today).await {
                tracing::error!(%school_id, error = ?e, "gagal mengantrikan notifikasi alfa");
            }
        }
    }

    if schools_done > 0 {
        tracing::info!(
            schools = schools_done,
            marked = total_marked,
            "penandaan siswa alfa selesai"
        );
    }

    // Dianggap "sudah dijalankan hari ini" hanya bila ada sekolah yang benar
    // benar diproses; kalau semua masih di luar jam, coba lagi nanti.
    Ok(schools_done > 0)
}

async fn notify_absentees(
    state: &AppState,
    school_id: Uuid,
    school_name: &str,
    date: chrono::NaiveDate,
) -> anyhow::Result<()> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        student_id: Uuid,
        student_name: String,
        student_nis: Option<String>,
        classroom_name: Option<String>,
    }

    let rows: Vec<Row> = sqlx::query_as(
        r#"
        SELECT id, student_id, student_name, student_nis, classroom_name
        FROM attendances
        WHERE school_id = $1
          AND attendance_date = $2
          AND status = 'alfa'
          AND notification_status = 'pending'
        LIMIT 5000
        "#,
    )
    .bind(school_id)
    .bind(date)
    .fetch_all(&state.db)
    .await?;

    for row in rows {
        let mut tx = state.db.begin().await?;
        let ctx = AttendanceNotifyContext {
            school_id,
            school_name: school_name.to_string(),
            student_id: row.student_id,
            student_name: row.student_name,
            student_nis: row.student_nis,
            classroom_name: row.classroom_name,
            attendance_id: row.id,
            attendance_date: date,
            status: crate::domain::attendance::AttendanceStatus::Alfa,
            check_in_at: None,
            check_out_at: None,
            late_minutes: 0,
        };
        let queued = notify::enqueue_attendance(&mut tx, &ctx, NotifyEvent::Absent).await?;
        if queued == 0 {
            // Tidak ada wali yang bisa dihubungi: tandai `skipped` agar baris
            // ini tidak diperiksa terus-menerus setiap siklus worker.
            sqlx::query(
                r#"
                UPDATE attendances SET notification_status = 'skipped'
                WHERE attendance_date = $1 AND id = $2
                "#,
            )
            .bind(date)
            .bind(row.id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
    }

    Ok(())
}

async fn cleanup(state: &AppState) -> anyhow::Result<()> {
    let tokens = sqlx::query(
        "DELETE FROM refresh_tokens WHERE expires_at < NOW() - INTERVAL '30 days'",
    )
    .execute(&state.db)
    .await?
    .rows_affected();

    let heartbeats = sqlx::query(
        "DELETE FROM device_heartbeats WHERE reported_at < NOW() - INTERVAL '30 days'",
    )
    .execute(&state.db)
    .await?
    .rows_affected();

    let idem = sqlx::query("DELETE FROM idempotency_keys WHERE expires_at < NOW()")
        .execute(&state.db)
        .await?
        .rows_affected();

    let exports = sqlx::query(
        "DELETE FROM report_exports WHERE expires_at IS NOT NULL AND expires_at < NOW()",
    )
    .execute(&state.db)
    .await?
    .rows_affected();

    // Kode pairing yang tidak pernah dipakai jangan dibiarkan hidup.
    let pairings = sqlx::query(
        r#"
        UPDATE devices SET pairing_code = NULL, pairing_expires_at = NULL
        WHERE pairing_code IS NOT NULL AND pairing_expires_at < NOW()
        "#,
    )
    .execute(&state.db)
    .await?
    .rows_affected();

    tracing::info!(
        tokens, heartbeats, idem, exports, pairings,
        "bersih-bersih harian selesai"
    );
    Ok(())
}
