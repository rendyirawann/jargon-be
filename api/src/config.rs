//! Konfigurasi aplikasi, seluruhnya dibaca dari environment.
//!
//! Tidak ada nilai rahasia yang di-hardcode. Lihat `.env.example`.

use std::time::Duration;

use crate::error::{ApiError, ApiResult};

// `public_url` dan `trusted_proxy` dibaca dari environment dan ditampilkan di
// log start-up; keduanya juga menjadi kontrak konfigurasi yang dipakai
// deployment (nginx/Kubernetes) walau belum dirujuk kode Rust lain.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Config {
    pub app_name: String,
    pub app_env: String,
    pub bind_addr: String,
    pub public_url: String,

    pub database_url: String,
    pub db_max_connections: u32,
    pub db_min_connections: u32,
    pub db_acquire_timeout: Duration,
    pub db_statement_timeout_ms: u32,

    pub redis_url: Option<String>,

    pub jwt_secret: String,
    pub jwt_issuer: String,
    pub access_token_ttl: Duration,
    pub refresh_token_ttl: Duration,

    /// Kunci AES-256-GCM (32 byte, hex) untuk mengenkripsi kredensial provider
    /// notifikasi yang tersimpan di kolom JSONB.
    pub secrets_key: Option<[u8; 32]>,

    pub storage_root: String,
    pub storage_public_base: String,
    pub max_upload_bytes: usize,

    pub face: FaceConfig,
    pub notify: NotifyConfig,

    pub cors_allowed_origins: Vec<String>,
    pub enable_swagger: bool,
    pub trusted_proxy: bool,
    pub workers_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct FaceConfig {
    /// Dimensi embedding. Harus sama dengan model di tablet.
    pub embedding_dim: usize,
    pub model_version: String,
    /// Ambang cosine similarity minimum untuk menerima kecocokan.
    pub match_threshold: f32,
    /// Selisih minimum antara kandidat terbaik dan kedua. Mencegah
    /// penerimaan saat dua siswa (mis. kembar) sama-sama mirip.
    pub match_margin: f32,
    /// Skor liveness minimum yang dilaporkan tablet.
    pub min_liveness: f32,
    /// Kualitas minimum foto pendaftaran.
    pub min_enroll_quality: f32,
    /// Berapa lama cache vektor per sekolah dianggap segar.
    pub index_ttl: Duration,
    /// Toleransi selisih jam tablet vs server (anti replay).
    pub clock_skew_tolerance: Duration,
    /// Jeda minimum antar dua scan siswa yang sama pada arah yang sama.
    pub scan_cooldown: Duration,
}

#[derive(Debug, Clone)]
pub struct NotifyConfig {
    pub enabled: bool,
    pub worker_batch_size: i64,
    pub worker_interval: Duration,

    pub wa_provider: String,
    pub wa_base_url: Option<String>,
    pub wa_token: Option<String>,
    pub wa_phone_number_id: Option<String>,

    pub telegram_bot_token: Option<String>,

    pub smtp_host: Option<String>,
    pub smtp_port: u16,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub smtp_from: String,
    pub smtp_from_name: String,
    pub smtp_starttls: bool,
}

fn env_str(key: &str) -> Option<String> {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() && v != "null" => Some(v.trim().to_string()),
        _ => None,
    }
}

fn env_or(key: &str, default: &str) -> String {
    env_str(key).unwrap_or_else(|| default.to_string())
}

fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    env_str(key).and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn env_bool(key: &str, default: bool) -> bool {
    match env_str(key).map(|v| v.to_ascii_lowercase()) {
        Some(v) => matches!(v.as_str(), "1" | "true" | "yes" | "on"),
        None => default,
    }
}

impl Config {
    pub fn from_env() -> ApiResult<Self> {
        let database_url = env_str("DATABASE_URL").ok_or_else(|| {
            ApiError::Config("DATABASE_URL wajib diisi (contoh: postgres://user:pass@localhost:5432/absensi)".into())
        })?;

        let jwt_secret = env_str("JWT_SECRET").ok_or_else(|| {
            ApiError::Config("JWT_SECRET wajib diisi (minimal 32 karakter acak)".into())
        })?;
        if jwt_secret.len() < 32 {
            return Err(ApiError::Config(
                "JWT_SECRET terlalu pendek, gunakan minimal 32 karakter".into(),
            ));
        }

        let secrets_key = match env_str("SECRETS_KEY_HEX") {
            Some(hex) => {
                let bytes = decode_hex(&hex).ok_or_else(|| {
                    ApiError::Config("SECRETS_KEY_HEX harus hex valid".into())
                })?;
                if bytes.len() != 32 {
                    return Err(ApiError::Config(
                        "SECRETS_KEY_HEX harus 32 byte (64 karakter hex)".into(),
                    ));
                }
                let mut key = [0u8; 32];
                key.copy_from_slice(&bytes);
                Some(key)
            }
            None => None,
        };

        Ok(Self {
            app_name: env_or("APP_NAME", "Jargon GO API"),
            app_env: env_or("APP_ENV", "local"),
            bind_addr: env_or("BIND_ADDR", "0.0.0.0:8080"),
            public_url: env_or("PUBLIC_URL", "http://localhost:8080"),

            database_url,
            db_max_connections: env_parse("DB_MAX_CONNECTIONS", 32),
            db_min_connections: env_parse("DB_MIN_CONNECTIONS", 4),
            db_acquire_timeout: Duration::from_secs(env_parse("DB_ACQUIRE_TIMEOUT_SECS", 10)),
            db_statement_timeout_ms: env_parse("DB_STATEMENT_TIMEOUT_MS", 15_000),

            redis_url: env_str("REDIS_URL"),

            jwt_secret,
            jwt_issuer: env_or("JWT_ISSUER", "jargon-api"),
            access_token_ttl: Duration::from_secs(env_parse("ACCESS_TOKEN_TTL_SECS", 3_600)),
            refresh_token_ttl: Duration::from_secs(env_parse("REFRESH_TOKEN_TTL_SECS", 2_592_000)),

            secrets_key,

            storage_root: env_or("STORAGE_ROOT", "./storage"),
            storage_public_base: env_or("STORAGE_PUBLIC_BASE", "/files"),
            max_upload_bytes: env_parse("MAX_UPLOAD_BYTES", 8 * 1024 * 1024),

            face: FaceConfig {
                embedding_dim: env_parse("FACE_EMBEDDING_DIM", 128),
                model_version: env_or("FACE_MODEL_VERSION", "faceapi-v1"),
                match_threshold: env_parse("FACE_MATCH_THRESHOLD", 0.62_f32),
                match_margin: env_parse("FACE_MATCH_MARGIN", 0.04_f32),
                min_liveness: env_parse("FACE_MIN_LIVENESS", 0.5_f32),
                min_enroll_quality: env_parse("FACE_MIN_ENROLL_QUALITY", 0.45_f32),
                index_ttl: Duration::from_secs(env_parse("FACE_INDEX_TTL_SECS", 300)),
                clock_skew_tolerance: Duration::from_secs(env_parse("FACE_CLOCK_SKEW_SECS", 120)),
                scan_cooldown: Duration::from_secs(env_parse("FACE_SCAN_COOLDOWN_SECS", 60)),
            },

            notify: NotifyConfig {
                enabled: env_bool("NOTIFY_ENABLED", true),
                worker_batch_size: env_parse("NOTIFY_BATCH_SIZE", 100),
                worker_interval: Duration::from_secs(env_parse("NOTIFY_INTERVAL_SECS", 5)),

                wa_provider: env_or("WA_PROVIDER", "fonnte"),
                wa_base_url: env_str("WA_BASE_URL"),
                wa_token: env_str("WA_TOKEN"),
                wa_phone_number_id: env_str("WA_PHONE_NUMBER_ID"),

                telegram_bot_token: env_str("TELEGRAM_BOT_TOKEN"),

                smtp_host: env_str("SMTP_HOST"),
                smtp_port: env_parse("SMTP_PORT", 587),
                smtp_username: env_str("SMTP_USERNAME"),
                smtp_password: env_str("SMTP_PASSWORD"),
                smtp_from: env_or("SMTP_FROM", "no-reply@disdik.sumutprov.go.id"),
                smtp_from_name: env_or("SMTP_FROM_NAME", "Absensi Disdik Sumut"),
                smtp_starttls: env_bool("SMTP_STARTTLS", true),
            },

            cors_allowed_origins: env_or("CORS_ALLOWED_ORIGINS", "*")
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            enable_swagger: env_bool("ENABLE_SWAGGER", true),
            trusted_proxy: env_bool("TRUSTED_PROXY", false),
            workers_enabled: env_bool("WORKERS_ENABLED", true),
        })
    }

    pub fn is_production(&self) -> bool {
        matches!(self.app_env.as_str(), "production" | "prod")
    }
}

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}
