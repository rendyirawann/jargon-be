//! Kanal email (SMTP).
//!
//! Transport SMTP dibangun sekali lalu digunakan ulang: lettre memelihara
//! connection pool di dalamnya, jadi membuat transport baru per pesan berarti
//! handshake TLS berulang untuk setiap notifikasi.

use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use once_cell::sync::OnceCell;

use crate::config::NotifyConfig;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

use super::DispatchResult;

type Mailer = AsyncSmtpTransport<Tokio1Executor>;

static MAILER: OnceCell<Mailer> = OnceCell::new();

fn mailer(cfg: &NotifyConfig) -> ApiResult<&'static Mailer> {
    MAILER.get_or_try_init(|| {
        let host = cfg.smtp_host.as_deref().ok_or_else(|| ApiError::Upstream {
            service: "email".into(),
            message: "SMTP_HOST belum dikonfigurasi".into(),
        })?;

        let mut builder = if cfg.smtp_starttls {
            Mailer::starttls_relay(host).map_err(|e| ApiError::Upstream {
                service: "email".into(),
                message: format!("gagal menyiapkan STARTTLS: {e}"),
            })?
        } else {
            Mailer::relay(host).map_err(|e| ApiError::Upstream {
                service: "email".into(),
                message: format!("gagal menyiapkan relay: {e}"),
            })?
        }
        .port(cfg.smtp_port);

        if let (Some(user), Some(pass)) = (&cfg.smtp_username, &cfg.smtp_password) {
            builder = builder.credentials(Credentials::new(user.clone(), pass.clone()));
        }

        Ok(builder.build())
    })
}

pub async fn send(
    state: &AppState,
    recipient: &str,
    subject: &str,
    body: &str,
) -> ApiResult<DispatchResult> {
    let cfg = &state.cfg.notify;
    let transport = mailer(cfg)?;

    let from = format!("{} <{}>", cfg.smtp_from_name, cfg.smtp_from);
    let email = Message::builder()
        .from(from.parse().map_err(|e| ApiError::Upstream {
            service: "email".into(),
            message: format!("alamat pengirim tidak valid: {e}"),
        })?)
        .to(recipient.parse().map_err(|e| ApiError::Upstream {
            service: "email".into(),
            message: format!("alamat tujuan tidak valid: {e}"),
        })?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body.to_string())
        .map_err(|e| ApiError::Upstream {
            service: "email".into(),
            message: format!("gagal menyusun email: {e}"),
        })?;

    let response = transport.send(email).await.map_err(|e| ApiError::Upstream {
        service: "email".into(),
        message: e.to_string(),
    })?;

    let message_id = response.message().next().map(|s| s.to_string());
    Ok(DispatchResult { provider: "smtp".into(), message_id })
}
