//! Extractor kustom.
//!
//! Tujuannya satu: memastikan SEMUA kegagalan input — JSON rusak, field
//! hilang, maupun nilai tidak valid — keluar dalam bentuk [`crate::error::ErrorBody`]
//! yang sama. Rejection bawaan axum mengembalikan teks biasa, yang membuat
//! klien Flutter harus menangani dua format berbeda.

use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Query, Request};
use axum::Json;
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::error::{ApiError, FieldError};

/// `Json<T>` + validasi otomatis.
pub struct ValidJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate,
    Json<T>: FromRequest<S, Rejection = JsonRejection>,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(map_json_rejection)?;
        value.validate()?;
        Ok(ValidJson(value))
    }
}

/// `Json<T>` tanpa validasi, tapi tetap dengan format error yang seragam.
pub struct JsonBody<T>(pub T);

impl<S, T> FromRequest<S> for JsonBody<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
    Json<T>: FromRequest<S, Rejection = JsonRejection>,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(map_json_rejection)?;
        Ok(JsonBody(value))
    }
}

fn map_json_rejection(rejection: JsonRejection) -> ApiError {
    match &rejection {
        JsonRejection::JsonDataError(e) => {
            // Pesan serde menyebut path field, mis. "missing field `nis` at line 3".
            ApiError::validation(vec![FieldError::new("_body", friendly(&e.body_text()))])
        }
        JsonRejection::JsonSyntaxError(_) => {
            ApiError::BadRequest("body bukan JSON yang valid".into())
        }
        JsonRejection::MissingJsonContentType(_) => {
            ApiError::BadRequest("header Content-Type: application/json diperlukan".into())
        }
        _ => ApiError::BadRequest("body permintaan tidak dapat diproses".into()),
    }
}

/// Ringkas pesan serde agar tidak membocorkan detail internal.
fn friendly(raw: &str) -> String {
    let cleaned = raw
        .strip_prefix("Failed to deserialize the JSON body into the target type: ")
        .unwrap_or(raw);
    match cleaned.split(" at line ").next() {
        Some(head) if !head.is_empty() => head.to_string(),
        _ => cleaned.to_string(),
    }
}

/// Query string dengan error seragam.
pub struct ValidQuery<T>(pub T);

impl<S, T> axum::extract::FromRequestParts<S> for ValidQuery<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(|e| ApiError::BadRequest(format!("parameter query tidak valid: {e}")))?;
        Ok(ValidQuery(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pesan_serde_diringkas() {
        let raw = "Failed to deserialize the JSON body into the target type: missing field `nis` at line 3 column 5";
        assert_eq!(friendly(raw), "missing field `nis`");
    }

    #[test]
    fn pesan_tanpa_prefix_dibiarkan() {
        assert_eq!(friendly("something odd"), "something odd");
    }
}
