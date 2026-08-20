//! Notifikasi ke wali murid: template, kebijakan, outbox, kirim manual.

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::Router;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::domain::notification::{
    mask_recipient, render_template, ChannelStat, NotificationPolicy, NotificationStats,
    NotificationTemplate, OutboxFilter, OutboxItem, SendMessageRequest, SendMessageResponse,
    SkippedRecipient, UpdatePolicyRequest, UpsertTemplateRequest, CHANNELS, TEMPLATE_KEYS,
};
use crate::error::{ApiError, ApiResult};
use crate::extract::{ValidJson, ValidQuery};
use crate::services::audit::AuditEntry;
use crate::state::AppState;
use crate::util::{self, ApiResponse, PageQuery, Paginated};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/notifications/templates", get(list_templates).post(upsert_template))
        .route("/notifications/policy", get(get_policy).patch(update_policy))
        .route("/notifications/outbox", get(list_outbox))
        .route("/notifications/outbox/{id}/retry", post(retry))
        .route("/notifications/send", post(send_manual))
        .route("/notifications/stats", get(stats))
}

/// Daftar template (milik sekolah + bawaan sistem sebagai fallback).
#[utoipa::path(
    get, path = "/v1/notifications/templates", tag = "Notifikasi",
    params(("school_id" = Option<Uuid>, Query, description = "ID sekolah")),
    security(("bearer" = [])),
    responses((status = 200, description = "Daftar template", body = [NotificationTemplate]))
)]
pub async fn list_templates(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(q): ValidQuery<SchoolScopeQuery>,
) -> ApiResult<ApiResponse<Vec<NotificationTemplate>>> {
    user.require("view_notification")?;
    let school = user.resolve_school(q.school_id)?;

    let rows: Vec<NotificationTemplate> = sqlx::query_as(
        r#"
        SELECT id, school_id, key, channel, subject, body, is_active, updated_at
        FROM notification_templates
        WHERE school_id IS NULL OR school_id = $1
        ORDER BY (school_id IS NULL), key, channel
        "#,
    )
    .bind(school)
    .fetch_all(&state.db)
    .await?;
    Ok(ApiResponse::new(rows))
}

#[derive(Debug, serde::Deserialize)]
pub struct SchoolScopeQuery {
    pub school_id: Option<Uuid>,
}

/// Simpan template khusus sekolah (menimpa template bawaan).
#[utoipa::path(
    post, path = "/v1/notifications/templates", tag = "Notifikasi",
    request_body = UpsertTemplateRequest,
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Template disimpan", body = NotificationTemplate),
        (status = 422, description = "Placeholder tidak dikenal / kunci salah")
    )
)]
pub async fn upsert_template(
    State(state): State<AppState>,
    user: AuthUser,
    ValidJson(body): ValidJson<UpsertTemplateRequest>,
) -> ApiResult<ApiResponse<NotificationTemplate>> {
    user.require("manage_notification_template")?;
    let school_id = user.require_school(body.school_id)?;

    if !TEMPLATE_KEYS.contains(&body.key.as_str()) {
        return Err(ApiError::field(
            "key",
            &format!("pilih salah satu: {}", TEMPLATE_KEYS.join(", ")),
        ));
    }
    if !CHANNELS.contains(&body.channel.as_str()) {
        return Err(ApiError::field(
            "channel",
            &format!("pilih salah satu: {}", CHANNELS.join(", ")),
        ));
    }
    // Placeholder salah tulis baru terlihat setelah pesan salah terkirim ke
    // ribuan orang tua, jadi diperiksa di sini.
    validate_placeholders(&body.body)?;
    if let Some(s) = &body.subject {
        validate_placeholders(s)?;
    }

    let (id,): (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO notification_templates (school_id, key, channel, subject, body, is_active)
        VALUES ($1,$2,$3,$4,$5,COALESCE($6,TRUE))
        ON CONFLICT (school_id, key, channel) WHERE school_id IS NOT NULL
        DO UPDATE SET subject = EXCLUDED.subject, body = EXCLUDED.body,
                      is_active = EXCLUDED.is_active, updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(school_id)
    .bind(&body.key)
    .bind(&body.channel)
    .bind(body.subject.as_deref())
    .bind(&body.body)
    .bind(body.is_active)
    .fetch_one(&state.db)
    .await?;

    let row: NotificationTemplate = sqlx::query_as(
        r#"
        SELECT id, school_id, key, channel, subject, body, is_active, updated_at
        FROM notification_templates WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    AuditEntry::by_user(&user, "notification_template.upsert")
        .school(school_id)
        .entity("notification_template", id)
        .after(&row)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(row, "Template notifikasi disimpan"))
}

/// Kebijakan notifikasi sekolah.
#[utoipa::path(
    get, path = "/v1/notifications/policy", tag = "Notifikasi",
    params(("school_id" = Option<Uuid>, Query, description = "ID sekolah")),
    security(("bearer" = [])),
    responses((status = 200, description = "Kebijakan notifikasi", body = NotificationPolicy))
)]
pub async fn get_policy(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(q): ValidQuery<SchoolScopeQuery>,
) -> ApiResult<ApiResponse<NotificationPolicy>> {
    user.require("view_notification")?;
    let school_id = user.require_school(q.school_id)?;
    crate::services::notify::ensure_policy(&state.db, school_id).await?;
    Ok(ApiResponse::new(fetch_policy(&state, school_id).await?))
}

/// Ubah kebijakan notifikasi.
#[utoipa::path(
    patch, path = "/v1/notifications/policy", tag = "Notifikasi",
    params(("school_id" = Option<Uuid>, Query, description = "ID sekolah")),
    request_body = UpdatePolicyRequest,
    security(("bearer" = [])),
    responses((status = 200, description = "Kebijakan diperbarui", body = NotificationPolicy))
)]
pub async fn update_policy(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(q): ValidQuery<SchoolScopeQuery>,
    crate::extract::JsonBody(body): crate::extract::JsonBody<UpdatePolicyRequest>,
) -> ApiResult<ApiResponse<NotificationPolicy>> {
    user.require("manage_notification_template")?;
    let school_id = user.require_school(q.school_id)?;
    crate::services::notify::ensure_policy(&state.db, school_id).await?;

    sqlx::query(
        r#"
        UPDATE notification_policies SET
            notify_on_check_in  = COALESCE($2, notify_on_check_in),
            notify_on_check_out = COALESCE($3, notify_on_check_out),
            notify_on_late      = COALESCE($4, notify_on_late),
            notify_on_absent    = COALESCE($5, notify_on_absent),
            absent_notify_after = COALESCE($6, absent_notify_after),
            quiet_hours_start   = COALESCE($7, quiet_hours_start),
            quiet_hours_end     = COALESCE($8, quiet_hours_end),
            daily_recap_at      = COALESCE($9, daily_recap_at),
            updated_at          = NOW()
        WHERE school_id = $1
        "#,
    )
    .bind(school_id)
    .bind(body.notify_on_check_in)
    .bind(body.notify_on_check_out)
    .bind(body.notify_on_late)
    .bind(body.notify_on_absent)
    .bind(body.absent_notify_after)
    .bind(body.quiet_hours_start)
    .bind(body.quiet_hours_end)
    .bind(body.daily_recap_at)
    .execute(&state.db)
    .await?;

    let policy = fetch_policy(&state, school_id).await?;
    AuditEntry::by_user(&user, "notification_policy.update")
        .school(school_id)
        .after(&policy)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(policy, "Kebijakan notifikasi diperbarui"))
}

/// Riwayat pesan terkirim/gagal. Nomor tujuan disamarkan.
#[utoipa::path(
    get, path = "/v1/notifications/outbox", tag = "Notifikasi",
    params(PageQuery, OutboxFilter),
    security(("bearer" = [])),
    responses((status = 200, description = "Daftar pesan", body = [OutboxItem]))
)]
pub async fn list_outbox(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(page): ValidQuery<PageQuery>,
    ValidQuery(filter): ValidQuery<OutboxFilter>,
) -> ApiResult<Paginated<OutboxItem>> {
    user.require("view_notification")?;
    let school = user.resolve_school(filter.school_id)?;

    // Outbox dipartisi per bulan; batasi ke 90 hari agar planner hanya
    // menyentuh beberapa partisi terbaru.
    let (total,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)::bigint FROM notification_outbox o
        WHERE o.created_at > NOW() - INTERVAL '90 days'
          AND ($1::uuid IS NULL OR o.school_id = $1)
          AND ($2::uuid IS NULL OR o.student_id = $2)
          AND ($3::text IS NULL OR o.status = $3)
          AND ($4::text IS NULL OR o.channel = $4)
        "#,
    )
    .bind(school)
    .bind(filter.student_id)
    .bind(filter.status.as_deref())
    .bind(filter.channel.as_deref())
    .fetch_one(&state.db)
    .await?;

    let mut items: Vec<OutboxItem> = sqlx::query_as(
        r#"
        SELECT o.id, o.school_id, o.student_id, s.full_name AS student_name, o.channel,
               o.template_key, o.recipient, o.subject, o.body, o.status, o.attempts,
               o.provider, o.last_error, o.scheduled_at, o.sent_at, o.created_at
        FROM notification_outbox o
        LEFT JOIN students s ON s.id = o.student_id
        WHERE o.created_at > NOW() - INTERVAL '90 days'
          AND ($1::uuid IS NULL OR o.school_id = $1)
          AND ($2::uuid IS NULL OR o.student_id = $2)
          AND ($3::text IS NULL OR o.status = $3)
          AND ($4::text IS NULL OR o.channel = $4)
        ORDER BY o.created_at DESC
        LIMIT $5 OFFSET $6
        "#,
    )
    .bind(school)
    .bind(filter.student_id)
    .bind(filter.status.as_deref())
    .bind(filter.channel.as_deref())
    .bind(page.per_page())
    .bind(page.offset())
    .fetch_all(&state.db)
    .await?;

    // Daftar log tidak boleh menjadi sumber ekspor nomor telepon orang tua.
    for item in &mut items {
        item.recipient = mask_recipient(&item.recipient);
    }

    Ok(Paginated::new(items, page.page(), page.per_page(), total))
}

/// Coba kirim ulang pesan yang gagal.
#[utoipa::path(
    post, path = "/v1/notifications/outbox/{id}/retry", tag = "Notifikasi",
    params(("id" = Uuid, Path, description = "ID pesan")),
    security(("bearer" = [])),
    responses((status = 200, description = "Pesan dijadwalkan ulang"))
)]
pub async fn retry(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require("send_notification")?;

    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT school_id FROM notification_outbox WHERE id = $1 AND created_at > NOW() - INTERVAL '90 days'",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let (school_id,) = row.ok_or_else(|| ApiError::NotFound(format!("pesan `{id}`")))?;
    user.resolve_school(Some(school_id))?;

    let affected = sqlx::query(
        r#"
        UPDATE notification_outbox
           SET status = 'queued', attempts = 0, scheduled_at = NOW(),
               locked_at = NULL, locked_by = NULL, last_error = NULL
         WHERE id = $1 AND status IN ('failed', 'cancelled')
        "#,
    )
    .bind(id)
    .execute(&state.db)
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(ApiError::Conflict(
            "hanya pesan berstatus gagal/dibatalkan yang bisa dikirim ulang".into(),
        ));
    }

    Ok(ApiResponse::with_message(
        serde_json::json!({ "requeued": true }),
        "Pesan dijadwalkan untuk dikirim ulang",
    ))
}

/// Kirim pesan bebas ke wali murid dari halaman monitoring.
#[utoipa::path(
    post, path = "/v1/notifications/send", tag = "Notifikasi",
    request_body = SendMessageRequest,
    security(("bearer" = [])),
    responses((status = 200, description = "Pesan diantrikan", body = SendMessageResponse))
)]
pub async fn send_manual(
    State(state): State<AppState>,
    user: AuthUser,
    ValidJson(body): ValidJson<SendMessageRequest>,
) -> ApiResult<ApiResponse<SendMessageResponse>> {
    user.require("send_notification")?;

    if let Some(c) = &body.channel {
        if !CHANNELS.contains(&c.as_str()) {
            return Err(ApiError::field("channel", "kanal tidak dikenal"));
        }
    }

    #[derive(sqlx::FromRow)]
    struct Target {
        student_id: Uuid,
        student_name: String,
        school_id: Uuid,
        school_name: String,
        classroom_name: Option<String>,
        guardian_id: Option<Uuid>,
        guardian_name: Option<String>,
        whatsapp: Option<String>,
        phone: Option<String>,
        email: Option<String>,
        telegram_chat_id: Option<String>,
        preferred_channel: Option<String>,
    }

    let targets: Vec<Target> = sqlx::query_as(
        r#"
        SELECT s.id AS student_id, s.full_name AS student_name, s.school_id,
               sc.name AS school_name, c.name AS classroom_name,
               g.id AS guardian_id, g.full_name AS guardian_name,
               g.whatsapp, g.phone, g.email, g.telegram_chat_id, g.preferred_channel
        FROM students s
        JOIN schools sc ON sc.id = s.school_id
        LEFT JOIN classrooms c ON c.id = s.current_classroom_id
        LEFT JOIN student_guardians g
               ON g.student_id = s.id AND g.notify_enabled AND g.is_primary
        WHERE s.id = ANY($1) AND s.deleted_at IS NULL
        "#,
    )
    .bind(&body.student_ids)
    .fetch_all(&state.db)
    .await?;

    let mut queued = 0usize;
    let mut skipped = Vec::new();

    for t in targets {
        if user.resolve_school(Some(t.school_id)).is_err() {
            skipped.push(SkippedRecipient {
                student_id: t.student_id,
                student_name: t.student_name,
                reason: "bukan siswa sekolah Anda".into(),
            });
            continue;
        }

        let channel = body
            .channel
            .clone()
            .or_else(|| t.preferred_channel.clone())
            .unwrap_or_else(|| "whatsapp".into());

        let recipient = match channel.as_str() {
            "whatsapp" => t
                .whatsapp
                .as_deref()
                .or(t.phone.as_deref())
                .and_then(crate::domain::student::normalize_phone),
            "telegram" => t.telegram_chat_id.clone().filter(|v| !v.trim().is_empty()),
            "email" => t.email.clone().filter(|v| v.contains('@')),
            _ => None,
        };

        let Some(recipient) = recipient else {
            skipped.push(SkippedRecipient {
                student_id: t.student_id,
                student_name: t.student_name,
                reason: format!("wali murid belum punya kontak {channel}"),
            });
            continue;
        };

        let vars = vec![
            ("nama_siswa", t.student_name.clone()),
            ("kelas", t.classroom_name.clone().unwrap_or_else(|| "-".into())),
            ("sekolah", t.school_name.clone()),
            ("tanggal", util::format_date_id(util::today_wib())),
            ("nama_wali", t.guardian_name.clone().unwrap_or_default()),
        ];
        let rendered = render_template(&body.body, &vars);
        let subject = body.subject.as_deref().map(|s| render_template(s, &vars));

        sqlx::query(
            r#"
            INSERT INTO notification_outbox
                (school_id, student_id, guardian_id, channel, template_key,
                 recipient, subject, body, status, scheduled_at)
            VALUES ($1,$2,$3,$4,'custom',$5,$6,$7,'queued',NOW())
            "#,
        )
        .bind(t.school_id)
        .bind(t.student_id)
        .bind(t.guardian_id)
        .bind(&channel)
        .bind(&recipient)
        .bind(subject.as_deref())
        .bind(&rendered)
        .execute(&state.db)
        .await?;

        queued += 1;
    }

    AuditEntry::by_user(&user, "notification.send_manual")
        .after(&serde_json::json!({ "queued": queued, "skipped": skipped.len() }))
        .write(&state.db)
        .await;

    let msg = format!("{queued} pesan diantrikan, {} dilewati", skipped.len());
    Ok(ApiResponse::with_message(
        SendMessageResponse { queued, skipped },
        msg,
    ))
}

/// Statistik pengiriman notifikasi.
#[utoipa::path(
    get, path = "/v1/notifications/stats", tag = "Notifikasi",
    params(("school_id" = Option<Uuid>, Query, description = "ID sekolah")),
    security(("bearer" = [])),
    responses((status = 200, description = "Statistik notifikasi", body = NotificationStats))
)]
pub async fn stats(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(q): ValidQuery<SchoolScopeQuery>,
) -> ApiResult<ApiResponse<NotificationStats>> {
    user.require("view_notification")?;
    let school = user.resolve_school(q.school_id)?;

    let (queued, sent_today, failed_today): (i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE status = 'queued')::bigint,
            COUNT(*) FILTER (WHERE status = 'sent'   AND sent_at::date = CURRENT_DATE)::bigint,
            COUNT(*) FILTER (WHERE status = 'failed' AND created_at::date = CURRENT_DATE)::bigint
        FROM notification_outbox
        WHERE created_at > NOW() - INTERVAL '7 days'
          AND ($1::uuid IS NULL OR school_id = $1)
        "#,
    )
    .bind(school)
    .fetch_one(&state.db)
    .await?;

    let by_channel: Vec<ChannelStat> = sqlx::query_as(
        r#"
        SELECT channel,
               COUNT(*) FILTER (WHERE status = 'sent')::bigint   AS sent,
               COUNT(*) FILTER (WHERE status = 'failed')::bigint AS failed,
               COUNT(*) FILTER (WHERE status = 'queued')::bigint AS queued
        FROM notification_outbox
        WHERE created_at > NOW() - INTERVAL '7 days'
          AND ($1::uuid IS NULL OR school_id = $1)
        GROUP BY channel
        ORDER BY channel
        "#,
    )
    .bind(school)
    .fetch_all(&state.db)
    .await?;

    Ok(ApiResponse::new(NotificationStats {
        queued,
        sent_today,
        failed_today,
        by_channel,
    }))
}

// =====================================================================

pub async fn fetch_policy(state: &AppState, school_id: Uuid) -> ApiResult<NotificationPolicy> {
    let row: NotificationPolicy = sqlx::query_as(
        r#"
        SELECT school_id, notify_on_check_in, notify_on_check_out, notify_on_late,
               notify_on_absent, absent_notify_after, quiet_hours_start,
               quiet_hours_end, daily_recap_at
        FROM notification_policies WHERE school_id = $1
        "#,
    )
    .bind(school_id)
    .fetch_one(&state.db)
    .await?;
    Ok(row)
}

/// Tolak template yang memuat placeholder di luar daftar resmi.
fn validate_placeholders(body: &str) -> ApiResult<()> {
    let known = crate::domain::notification::TEMPLATE_VARIABLES;
    let mut unknown = Vec::new();
    let mut rest = body;

    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else { break };
        let name = after[..end].trim();
        if !name.is_empty() && !known.contains(&name) {
            unknown.push(name.to_string());
        }
        rest = &after[end + 2..];
    }

    if unknown.is_empty() {
        Ok(())
    } else {
        unknown.sort();
        unknown.dedup();
        Err(ApiError::field(
            "body",
            &format!(
                "placeholder tidak dikenal: {}. Yang tersedia: {}",
                unknown.join(", "),
                known.join(", ")
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_dikenal_diterima() {
        assert!(validate_placeholders("Ananda {{nama_siswa}} kelas {{kelas}} hadir.").is_ok());
    }

    #[test]
    fn placeholder_salah_tulis_ditolak() {
        let err = validate_placeholders("Ananda {{nama_sisw}} hadir.").unwrap_err();
        match err {
            ApiError::Validation(errs) => {
                assert!(errs[0].message.contains("nama_sisw"));
            }
            other => panic!("harus validation error, dapat {other:?}"),
        }
    }

    #[test]
    fn teks_tanpa_placeholder_diterima() {
        assert!(validate_placeholders("Pengumuman rapat wali murid.").is_ok());
    }

    #[test]
    fn kurung_tidak_lengkap_tidak_panik() {
        assert!(validate_placeholders("Halo {{nama_siswa").is_ok());
        assert!(validate_placeholders("}}{{").is_ok());
    }

    #[test]
    fn beberapa_placeholder_salah_dilaporkan_sekaligus() {
        let err = validate_placeholders("{{aaa}} dan {{bbb}} dan {{kelas}}").unwrap_err();
        match err {
            ApiError::Validation(errs) => {
                assert!(errs[0].message.contains("aaa"));
                assert!(errs[0].message.contains("bbb"));
            }
            other => panic!("harus validation error, dapat {other:?}"),
        }
    }
}
