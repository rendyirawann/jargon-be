//! Kanal Telegram (Bot API).
//!
//! Telegram adalah kanal paling murah — gratis dan tanpa batas praktis —
//! sehingga cocok sebagai pilihan utama untuk sekolah dengan anggaran
//! terbatas. Syaratnya wali murid harus lebih dulu menekan `/start` pada bot
//! sekolah agar `chat_id` bisa diperoleh; alur itu ditangani oleh endpoint
//! webhook di `routes/notifications.rs`.

use serde_json::json;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

use super::DispatchResult;

pub async fn send(state: &AppState, chat_id: &str, body: &str) -> ApiResult<DispatchResult> {
    let token = state
        .cfg
        .notify
        .telegram_bot_token
        .as_deref()
        .ok_or_else(|| ApiError::Upstream {
            service: "telegram".into(),
            message: "TELEGRAM_BOT_TOKEN belum dikonfigurasi".into(),
        })?;

    let url = format!("https://api.telegram.org/bot{token}/sendMessage");
    let payload = json!({
        "chat_id": chat_id,
        "text": body,
        "parse_mode": "HTML",
        "disable_web_page_preview": true,
    });

    let resp = state.http.post(url).json(&payload).send().await?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or(json!({}));
    let ok = parsed.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);

    if !status.is_success() || !ok {
        let description = parsed
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("respons tidak dikenali");
        return Err(ApiError::Upstream {
            service: "telegram".into(),
            message: format!("HTTP {status}: {description}"),
        });
    }

    let message_id = parsed
        .get("result")
        .and_then(|r| r.get("message_id"))
        .map(|v| v.to_string());

    Ok(DispatchResult { provider: "telegram_bot".into(), message_id })
}

/// Bersihkan teks agar aman dipakai dengan `parse_mode: HTML`.
///
/// Nama siswa boleh berisi `&` atau `<`; tanpa escape, Telegram menolak
/// seluruh pesan dengan galat parsing. Dipakai saat merender nilai variabel
/// ke dalam template Telegram — badan template sendiri memang berisi tag HTML
/// dan karena itu tidak di-escape.
#[allow(dead_code)]
pub fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_html_mengamankan_karakter_khusus() {
        assert_eq!(escape_html("A & B"), "A &amp; B");
        assert_eq!(escape_html("<b>x</b>"), "&lt;b&gt;x&lt;/b&gt;");
        assert_eq!(escape_html("Budi"), "Budi");
    }

    #[test]
    fn escape_html_urutan_ampersand_benar() {
        // Ampersand harus di-escape lebih dulu agar tidak menghasilkan
        // "&amp;lt;" ganda.
        assert_eq!(escape_html("&<"), "&amp;&lt;");
    }
}
