//! Kanal WhatsApp.
//!
//! Tiga provider didukung karena masing-masing punya alasan pakai:
//!   * `fonnte` / `wablas` — provider lokal, murah, cukup untuk pilot
//!     beberapa sekolah, tidak perlu verifikasi Meta Business.
//!   * `meta_cloud`       — WhatsApp Cloud API resmi. Wajib untuk skala
//!     provinsi (700rb siswa) karena punya SLA & rate limit yang jelas.
//!
//! Provider dipilih lewat `WA_PROVIDER`. Menambah provider baru berarti
//! menambah satu cabang di [`send`], tanpa menyentuh worker maupun outbox.

use serde_json::json;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

use super::DispatchResult;

pub async fn send(state: &AppState, recipient: &str, body: &str) -> ApiResult<DispatchResult> {
    let cfg = &state.cfg.notify;
    let token = cfg.wa_token.as_deref().ok_or_else(|| ApiError::Upstream {
        service: "whatsapp".into(),
        message: "WA_TOKEN belum dikonfigurasi".into(),
    })?;

    match cfg.wa_provider.as_str() {
        "fonnte" => send_fonnte(state, token, recipient, body).await,
        "wablas" => send_wablas(state, token, recipient, body).await,
        "meta_cloud" => send_meta_cloud(state, token, recipient, body).await,
        other => Err(ApiError::Upstream {
            service: "whatsapp".into(),
            message: format!("provider `{other}` tidak dikenal"),
        }),
    }
}

async fn send_fonnte(
    state: &AppState,
    token: &str,
    recipient: &str,
    body: &str,
) -> ApiResult<DispatchResult> {
    let url = state
        .cfg
        .notify
        .wa_base_url
        .clone()
        .unwrap_or_else(|| "https://api.fonnte.com/send".to_string());

    let resp = state
        .http
        .post(url)
        .header("Authorization", token)
        .form(&[("target", recipient), ("message", body)])
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(ApiError::Upstream {
            service: "fonnte".into(),
            message: format!("HTTP {status}: {}", truncate(&text, 300)),
        });
    }

    // Fonnte membalas 200 walau gagal, jadi body harus diperiksa.
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or(json!({}));
    if parsed.get("status").and_then(|v| v.as_bool()) == Some(false) {
        let reason = parsed
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("alasan tidak diketahui");
        return Err(ApiError::Upstream {
            service: "fonnte".into(),
            message: reason.to_string(),
        });
    }

    let message_id = parsed
        .get("id")
        .map(|v| v.to_string())
        .map(|s| s.trim_matches('"').to_string());

    Ok(DispatchResult { provider: "fonnte".into(), message_id })
}

async fn send_wablas(
    state: &AppState,
    token: &str,
    recipient: &str,
    body: &str,
) -> ApiResult<DispatchResult> {
    let url = state
        .cfg
        .notify
        .wa_base_url
        .clone()
        .unwrap_or_else(|| "https://console.wablas.com/api/send-message".to_string());

    let resp = state
        .http
        .post(url)
        .header("Authorization", token)
        .form(&[("phone", recipient), ("message", body)])
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(ApiError::Upstream {
            service: "wablas".into(),
            message: format!("HTTP {status}: {}", truncate(&text, 300)),
        });
    }
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or(json!({}));
    if parsed.get("status").and_then(|v| v.as_bool()) == Some(false) {
        return Err(ApiError::Upstream {
            service: "wablas".into(),
            message: truncate(&text, 300),
        });
    }
    Ok(DispatchResult { provider: "wablas".into(), message_id: None })
}

async fn send_meta_cloud(
    state: &AppState,
    token: &str,
    recipient: &str,
    body: &str,
) -> ApiResult<DispatchResult> {
    let phone_number_id =
        state
            .cfg
            .notify
            .wa_phone_number_id
            .as_deref()
            .ok_or_else(|| ApiError::Upstream {
                service: "meta_cloud".into(),
                message: "WA_PHONE_NUMBER_ID belum dikonfigurasi".into(),
            })?;

    let base = state
        .cfg
        .notify
        .wa_base_url
        .clone()
        .unwrap_or_else(|| "https://graph.facebook.com/v21.0".to_string());
    let url = format!("{}/{}/messages", base.trim_end_matches('/'), phone_number_id);

    let payload = json!({
        "messaging_product": "whatsapp",
        "recipient_type": "individual",
        "to": recipient,
        "type": "text",
        "text": { "preview_url": false, "body": body }
    });

    let resp = state
        .http
        .post(url)
        .bearer_auth(token)
        .json(&payload)
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(ApiError::Upstream {
            service: "meta_cloud".into(),
            message: format!("HTTP {status}: {}", truncate(&text, 400)),
        });
    }

    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or(json!({}));
    let message_id = parsed
        .get("messages")
        .and_then(|m| m.get(0))
        .and_then(|m| m.get("id"))
        .and_then(|v| v.as_str())
        .map(String::from);

    Ok(DispatchResult { provider: "meta_cloud".into(), message_id })
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>() + "..."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_menghormati_batas_karakter() {
        assert_eq!(truncate("abc", 10), "abc");
        assert_eq!(truncate("abcdefghij", 3), "abc...");
    }

    #[test]
    fn truncate_aman_untuk_karakter_multibyte() {
        // Tidak boleh panik memotong di tengah karakter UTF-8.
        let s = "гагаНАДА—ané";
        let out = truncate(s, 4);
        assert_eq!(out.chars().count(), 7); // 4 + "..."
    }
}
