//! Tipe error tunggal untuk seluruh API + representasi JSON-nya.
//!
//! Semua endpoint mengembalikan bentuk error yang sama sehingga klien
//! (tablet Flutter maupun dashboard Laravel) hanya perlu satu parser.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("konfigurasi tidak valid: {0}")]
    Config(String),

    #[error("{0}")]
    BadRequest(String),

    #[error("validasi gagal")]
    Validation(Vec<FieldError>),

    #[error("autentikasi diperlukan: {0}")]
    Unauthorized(String),

    #[error("akses ditolak: {0}")]
    Forbidden(String),

    #[error("{0} tidak ditemukan")]
    NotFound(String),

    #[error("{0}")]
    Conflict(String),

    #[error("terlalu banyak permintaan")]
    TooManyRequests,

    #[error("layanan {service} tidak tersedia: {message}")]
    Upstream { service: String, message: String },

    #[error(transparent)]
    Database(#[from] sqlx::Error),

    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct FieldError {
    /// Nama field yang bermasalah.
    pub field: String,
    /// Penjelasan singkat dalam bahasa Indonesia.
    pub message: String,
}

impl FieldError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self { field: field.into(), message: message.into() }
    }
}

/// Bentuk baku body error.
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    /// Selalu `false` pada respons error.
    pub success: bool,
    /// Kode mesin, mis. `validation_error`, `not_found`.
    pub code: String,
    /// Pesan yang boleh ditampilkan ke pengguna.
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<FieldError>>,
}

impl ApiError {
    pub fn validation(errors: Vec<FieldError>) -> Self {
        ApiError::Validation(errors)
    }

    pub fn field(field: &str, message: &str) -> Self {
        ApiError::Validation(vec![FieldError::new(field, message)])
    }

    fn parts(&self) -> (StatusCode, &'static str) {
        match self {
            ApiError::Config(_) => (StatusCode::INTERNAL_SERVER_ERROR, "config_error"),
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            ApiError::Validation(_) => (StatusCode::UNPROCESSABLE_ENTITY, "validation_error"),
            ApiError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "unauthorized"),
            ApiError::Forbidden(_) => (StatusCode::FORBIDDEN, "forbidden"),
            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            ApiError::Conflict(_) => (StatusCode::CONFLICT, "conflict"),
            ApiError::TooManyRequests => (StatusCode::TOO_MANY_REQUESTS, "too_many_requests"),
            ApiError::Upstream { .. } => (StatusCode::BAD_GATEWAY, "upstream_error"),
            ApiError::Database(e) => match e {
                // Pelanggaran unique -> 409 agar klien bisa membedakan.
                sqlx::Error::Database(db) if db.code().as_deref() == Some("23505") => {
                    (StatusCode::CONFLICT, "duplicate")
                }
                // Pelanggaran foreign key.
                sqlx::Error::Database(db) if db.code().as_deref() == Some("23503") => {
                    (StatusCode::UNPROCESSABLE_ENTITY, "invalid_reference")
                }
                sqlx::Error::RowNotFound => (StatusCode::NOT_FOUND, "not_found"),
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "database_error"),
            },
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = self.parts();

        // Detail teknis hanya masuk log, tidak pernah ke klien.
        let message = match &self {
            ApiError::Database(e) => {
                tracing::error!(error = %e, "kesalahan database");
                match self.parts().1 {
                    "duplicate" => "Data sudah ada.".to_string(),
                    "invalid_reference" => "Referensi data tidak valid.".to_string(),
                    "not_found" => "Data tidak ditemukan.".to_string(),
                    _ => "Terjadi kesalahan pada basis data.".to_string(),
                }
            }
            ApiError::Internal(e) => {
                tracing::error!(error = ?e, "kesalahan internal");
                "Terjadi kesalahan internal pada server.".to_string()
            }
            ApiError::Config(e) => {
                tracing::error!(error = %e, "konfigurasi tidak valid");
                "Konfigurasi server tidak valid.".to_string()
            }
            ApiError::Validation(_) => "Data yang dikirim tidak valid.".to_string(),
            other => other.to_string(),
        };

        let errors = match self {
            ApiError::Validation(errs) => Some(errs),
            _ => None,
        };

        (
            status,
            Json(ErrorBody { success: false, code: code.to_string(), message, errors }),
        )
            .into_response()
    }
}

impl From<validator::ValidationErrors> for ApiError {
    fn from(value: validator::ValidationErrors) -> Self {
        let mut out = Vec::new();
        for (field, kind) in value.errors() {
            if let validator::ValidationErrorsKind::Field(items) = kind {
                for item in items {
                    let msg = item
                        .message
                        .as_ref()
                        .map(|m| m.to_string())
                        .unwrap_or_else(|| format!("tidak valid ({})", item.code));
                    out.push(FieldError::new(field.to_string(), msg));
                }
            }
        }
        if out.is_empty() {
            out.push(FieldError::new("_", "tidak valid"));
        }
        ApiError::Validation(out)
    }
}

impl From<redis::RedisError> for ApiError {
    fn from(value: redis::RedisError) -> Self {
        ApiError::Internal(anyhow::anyhow!("redis: {value}"))
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(value: reqwest::Error) -> Self {
        ApiError::Upstream { service: "http".into(), message: value.to_string() }
    }
}

impl From<std::io::Error> for ApiError {
    fn from(value: std::io::Error) -> Self {
        ApiError::Internal(anyhow::anyhow!("io: {value}"))
    }
}
