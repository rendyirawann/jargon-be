//! Worker pengirim notifikasi.
//!
//! Mengambil pesan dari `notification_outbox` lalu mengirimnya. Beberapa
//! instance API boleh berjalan bersamaan: klaim pekerjaan memakai
//! `FOR UPDATE SKIP LOCKED`, sehingga dua worker tidak akan pernah mengirim
//! pesan yang sama, dan worker yang mati tidak menahan pekerjaan siapa pun.

use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use uuid::Uuid;

use crate::services::notify::{self, PendingMessage};
use crate::state::AppState;

/// Pesan yang "sedang dikirim" lebih lama dari ini dianggap ditinggalkan
/// (proses mati di tengah jalan) dan boleh diklaim ulang.
const STUCK_AFTER: i64 = 300;

pub async fn run(state: AppState, shutdown: tokio::sync::watch::Receiver<bool>) {
    if !state.cfg.notify.enabled {
        tracing::info!("worker notifikasi dimatikan (NOTIFY_ENABLED=false)");
        return;
    }

    let worker_id = format!("outbox-{}", Uuid::new_v4());
    let interval = effective_interval(state.cfg.notify.worker_interval);
    tracing::info!(%worker_id, interval_secs = interval.as_secs(), "worker notifikasi berjalan");

    let mut shutdown = shutdown;
    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    tracing::info!(%worker_id, "worker notifikasi berhenti");
                    return;
                }
            }
            _ = tokio::time::sleep(interval) => {
                if let Err(e) = tick(&state, &worker_id).await {
                    tracing::error!(error = ?e, "siklus worker notifikasi gagal");
                }
            }
        }
    }
}

async fn tick(state: &AppState, worker_id: &str) -> anyhow::Result<()> {
    reclaim_stuck(state).await?;

    let batch = claim(state, worker_id, state.cfg.notify.worker_batch_size).await?;
    if batch.is_empty() {
        return Ok(());
    }

    tracing::debug!(count = batch.len(), "mengirim batch notifikasi");

    for msg in batch {
        match notify::dispatch(state, &msg).await {
            Ok(result) => mark_sent(state, &msg, &result).await?,
            Err(e) => mark_failed(state, &msg, &e.to_string()).await?,
        }
    }
    Ok(())
}

/// Klaim sejumlah pesan sekaligus dalam satu transaksi.
async fn claim(
    state: &AppState,
    worker_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<PendingMessage>> {
    let mut tx = state.db.begin().await?;

    // Subquery memilih baris dengan SKIP LOCKED; UPDATE menandainya sebagai
    // 'sending' sehingga instance lain tidak melihatnya lagi.
    let rows: Vec<PendingMessage> = sqlx::query_as(
        r#"
        WITH kandidat AS (
            SELECT created_at, id
            FROM notification_outbox
            WHERE status = 'queued'
              AND scheduled_at <= NOW()
              AND created_at > NOW() - INTERVAL '7 days'
            ORDER BY scheduled_at
            LIMIT $1
            FOR UPDATE SKIP LOCKED
        )
        UPDATE notification_outbox o
           SET status = 'sending', locked_at = NOW(), locked_by = $2,
               attempts = o.attempts + 1
          FROM kandidat k
         WHERE o.created_at = k.created_at AND o.id = k.id
        RETURNING o.id, o.created_at, o.school_id, o.channel, o.recipient,
                  o.subject, o.body, o.attempts, o.max_attempts
        "#,
    )
    .bind(limit)
    .bind(worker_id)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(rows)
}

/// Kembalikan pesan yang tergantung di status 'sending' ke antrean.
async fn reclaim_stuck(state: &AppState) -> anyhow::Result<()> {
    let affected = sqlx::query(
        r#"
        UPDATE notification_outbox
           SET status = 'queued', locked_at = NULL, locked_by = NULL
         WHERE status = 'sending'
           AND locked_at < NOW() - make_interval(secs => $1::double precision)
           AND created_at > NOW() - INTERVAL '7 days'
        "#,
    )
    .bind(STUCK_AFTER as f64)
    .execute(&state.db)
    .await?
    .rows_affected();

    if affected > 0 {
        tracing::warn!(count = affected, "pesan tertahan dikembalikan ke antrean");
    }
    Ok(())
}

async fn mark_sent(
    state: &AppState,
    msg: &PendingMessage,
    result: &notify::DispatchResult,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE notification_outbox
           SET status = 'sent', sent_at = NOW(), provider = $3,
               provider_message_id = $4, last_error = NULL,
               locked_at = NULL, locked_by = NULL
         WHERE created_at = $1 AND id = $2
        "#,
    )
    .bind(msg.created_at)
    .bind(msg.id)
    .bind(&result.provider)
    .bind(result.message_id.as_deref())
    .execute(&state.db)
    .await?;

    // Absensi terkait ditandai 'sent' hanya bila tidak ada lagi pesan yang
    // menggantung untuk baris itu.
    sqlx::query(
        r#"
        UPDATE attendances a
           SET notification_status = 'sent'
          FROM notification_outbox o
         WHERE o.created_at = $1 AND o.id = $2
           AND a.id = o.attendance_id
           AND NOT EXISTS (
               SELECT 1 FROM notification_outbox p
               WHERE p.attendance_id = o.attendance_id
                 AND p.status IN ('queued','sending')
           )
        "#,
    )
    .bind(msg.created_at)
    .bind(msg.id)
    .execute(&state.db)
    .await?;

    Ok(())
}

async fn mark_failed(
    state: &AppState,
    msg: &PendingMessage,
    error: &str,
) -> anyhow::Result<()> {
    let exhausted = msg.attempts >= msg.max_attempts;

    if exhausted {
        sqlx::query(
            r#"
            UPDATE notification_outbox
               SET status = 'failed', last_error = $3,
                   locked_at = NULL, locked_by = NULL
             WHERE created_at = $1 AND id = $2
            "#,
        )
        .bind(msg.created_at)
        .bind(msg.id)
        .bind(truncate(error, 1000))
        .execute(&state.db)
        .await?;

        sqlx::query(
            r#"
            UPDATE attendances a
               SET notification_status = 'failed'
              FROM notification_outbox o
             WHERE o.created_at = $1 AND o.id = $2 AND a.id = o.attendance_id
            "#,
        )
        .bind(msg.created_at)
        .bind(msg.id)
        .execute(&state.db)
        .await?;

        tracing::error!(
            id = %msg.id, channel = %msg.channel, attempts = msg.attempts,
            error, "notifikasi gagal permanen"
        );
    } else {
        let delay = notify::retry_delay_seconds(msg.attempts);
        let next = Utc::now() + ChronoDuration::seconds(delay);

        sqlx::query(
            r#"
            UPDATE notification_outbox
               SET status = 'queued', scheduled_at = $3, last_error = $4,
                   locked_at = NULL, locked_by = NULL
             WHERE created_at = $1 AND id = $2
            "#,
        )
        .bind(msg.created_at)
        .bind(msg.id)
        .bind(next)
        .bind(truncate(error, 1000))
        .execute(&state.db)
        .await?;

        tracing::warn!(
            id = %msg.id, channel = %msg.channel, attempts = msg.attempts,
            retry_in_seconds = delay, error, "notifikasi gagal, dijadwalkan ulang"
        );
    }

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

/// Interval polling yang layak berdasarkan konfigurasi.
pub fn effective_interval(configured: Duration) -> Duration {
    // Terlalu sering = beban query sia-sia; terlalu jarang = orang tua
    // menerima kabar anaknya terlambat setengah jam kemudian.
    configured.clamp(Duration::from_secs(1), Duration::from_secs(60))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_dijepit_ke_rentang_waras() {
        assert_eq!(
            effective_interval(Duration::from_millis(10)),
            Duration::from_secs(1)
        );
        assert_eq!(
            effective_interval(Duration::from_secs(600)),
            Duration::from_secs(60)
        );
        assert_eq!(
            effective_interval(Duration::from_secs(5)),
            Duration::from_secs(5)
        );
    }

    #[test]
    fn truncate_aman_multibyte() {
        assert_eq!(truncate("abc", 10), "abc");
        assert_eq!(truncate("абвгд", 3), "абв");
    }
}
