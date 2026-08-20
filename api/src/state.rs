//! State bersama yang diinjeksikan ke setiap handler.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::jwt::JwtKeys;
use crate::auth::{password, AuthClient, AuthDevice};
use crate::config::Config;
use crate::error::{ApiError, ApiResult};
use crate::face::{vector, FaceIndex};
use crate::services::storage::Storage;

/// Berapa lama hasil lookup perangkat dianggap valid.
/// Perangkat melakukan request tiap beberapa detik; tanpa cache ini setiap
/// scan absensi akan menambah satu query ke `devices`.
const DEVICE_CACHE_TTL: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub cfg: Arc<Config>,
    pub jwt: JwtKeys,
    pub face_index: Arc<FaceIndex>,
    pub storage: Arc<Storage>,
    pub http: reqwest::Client,
    pub redis: Option<redis::aio::ConnectionManager>,
    pub started_at: Instant,

    device_cache: Arc<DashMap<Vec<u8>, (AuthDevice, Instant)>>,
}

impl AppState {
    pub async fn bootstrap(cfg: Config) -> ApiResult<Self> {
        let statement_timeout = cfg.db_statement_timeout_ms;

        let db = PgPoolOptions::new()
            .max_connections(cfg.db_max_connections)
            .min_connections(cfg.db_min_connections)
            .acquire_timeout(cfg.db_acquire_timeout)
            .test_before_acquire(true)
            .after_connect(move |conn, _meta| {
                Box::pin(async move {
                    // Batas waktu per statement mencegah satu query lambat
                    // menahan koneksi selamanya saat jam sibuk absensi pagi.
                    sqlx::query(&format!("SET statement_timeout = {statement_timeout}"))
                        .execute(&mut *conn)
                        .await?;
                    sqlx::query("SET timezone = 'Asia/Jakarta'")
                        .execute(&mut *conn)
                        .await?;
                    Ok(())
                })
            })
            .connect(&cfg.database_url)
            .await
            .map_err(|e| {
                ApiError::Config(format!("gagal terhubung ke PostgreSQL: {e}"))
            })?;

        let redis = match &cfg.redis_url {
            Some(url) => match redis::Client::open(url.as_str()) {
                Ok(client) => match redis::aio::ConnectionManager::new(client).await {
                    Ok(cm) => {
                        tracing::info!("Redis terhubung");
                        Some(cm)
                    }
                    Err(e) => {
                        // Redis bersifat opsional: hanya untuk rate limit &
                        // invalidasi lintas-instance. API tetap jalan tanpanya.
                        tracing::warn!(error = %e, "Redis tidak dapat dihubungi, melanjutkan tanpa cache terdistribusi");
                        None
                    }
                },
                Err(e) => {
                    tracing::warn!(error = %e, "REDIS_URL tidak valid, diabaikan");
                    None
                }
            },
            None => None,
        };

        let storage = Storage::new(cfg.storage_root.clone(), cfg.storage_public_base.clone());
        storage.ensure_root().await?;

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .connect_timeout(Duration::from_secs(5))
            .user_agent(format!("jargon-api/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| ApiError::Config(format!("gagal membuat HTTP client: {e}")))?;

        let jwt = JwtKeys::new(&cfg.jwt_secret, &cfg.jwt_issuer);
        let face_index = Arc::new(FaceIndex::new(cfg.face.embedding_dim, cfg.face.index_ttl));

        Ok(Self {
            db,
            cfg: Arc::new(cfg),
            jwt,
            face_index,
            storage: Arc::new(storage),
            http,
            redis,
            started_at: Instant::now(),
            device_cache: Arc::new(DashMap::new()),
        })
    }

    // -----------------------------------------------------------------
    // Perangkat
    // -----------------------------------------------------------------

    /// Cari perangkat berdasarkan token mentah. Hasil di-cache singkat.
    pub async fn lookup_device(&self, raw_token: &str) -> ApiResult<AuthDevice> {
        let hash = vector::sha256(raw_token.as_bytes());

        if let Some(entry) = self.device_cache.get(&hash) {
            let (device, at) = entry.value();
            if at.elapsed() < DEVICE_CACHE_TTL {
                return Ok(device.clone());
            }
        }

        let row: Option<(Uuid, Uuid, String, String, String, String, Option<Uuid>, Option<Vec<u8>>)> =
            sqlx::query_as(
                r#"
                SELECT d.id, d.school_id, d.code, d.name, d.mode, d.placement,
                       d.classroom_id, d.hmac_secret
                FROM devices d
                JOIN schools s ON s.id = d.school_id
                WHERE d.token_hash = $1
                  AND d.is_active
                  AND d.deleted_at IS NULL
                  AND d.token_revoked_at IS NULL
                  AND s.is_active
                  AND s.deleted_at IS NULL
                "#,
            )
            .bind(&hash)
            .fetch_optional(&self.db)
            .await?;

        let (id, school_id, code, name, mode, placement, classroom_id, hmac_secret) = row
            .ok_or_else(|| {
                ApiError::Unauthorized("token perangkat tidak dikenal atau sudah dicabut".into())
            })?;

        let device = AuthDevice {
            id,
            school_id,
            code,
            name,
            mode,
            placement,
            classroom_id,
            hmac_secret,
        };
        self.device_cache.insert(hash, (device.clone(), Instant::now()));
        Ok(device)
    }

    /// Buang perangkat dari cache — dipanggil saat token dicabut / diubah,
    /// agar pencabutan berlaku seketika, bukan setelah TTL.
    pub fn invalidate_device_cache(&self) {
        self.device_cache.clear();
    }

    // -----------------------------------------------------------------
    // Klien server-to-server
    // -----------------------------------------------------------------

    /// Verifikasi kredensial layanan. Dipakai oleh extractor [`AuthClient`].
    #[allow(dead_code)]
    pub async fn lookup_api_client(&self, key_id: &str, secret: &str) -> ApiResult<AuthClient> {
        let row: Option<(Uuid, String, Option<Uuid>, Vec<u8>, Vec<String>)> = sqlx::query_as(
            r#"
            SELECT id, name, school_id, secret_hash, scopes
            FROM api_clients
            WHERE key_id = $1 AND is_active
            "#,
        )
        .bind(key_id)
        .fetch_optional(&self.db)
        .await?;

        let (id, name, school_id, secret_hash, scopes) = row
            .ok_or_else(|| ApiError::Unauthorized("kredensial API tidak dikenal".into()))?;

        let given = vector::sha256(secret.as_bytes());
        if !password::constant_time_eq(&given, &secret_hash) {
            return Err(ApiError::Unauthorized("kredensial API tidak valid".into()));
        }

        // Tidak menunggu hasilnya: pembaruan `last_used_at` tidak boleh
        // memperlambat jalur request.
        let pool = self.db.clone();
        tokio::spawn(async move {
            let _ = sqlx::query("UPDATE api_clients SET last_used_at = NOW() WHERE id = $1")
                .bind(id)
                .execute(&pool)
                .await;
        });

        Ok(AuthClient { id, name, school_id, scopes })
    }

    // -----------------------------------------------------------------
    // Redis (opsional)
    // -----------------------------------------------------------------

    /// Rate limit sederhana berbasis penghitung per jendela waktu.
    /// Bila Redis tidak tersedia, request selalu diizinkan (fail-open) —
    /// membatasi absensi siswa lebih buruk daripada kehilangan rate limit.
    pub async fn rate_limit(&self, key: &str, max: u64, window: Duration) -> ApiResult<()> {
        let Some(mut conn) = self.redis.clone() else {
            return Ok(());
        };
        let redis_key = format!("rl:{key}");
        let count: u64 = redis::cmd("INCR")
            .arg(&redis_key)
            .query_async(&mut conn)
            .await
            .unwrap_or(0);
        if count == 1 {
            let _: Result<(), _> = redis::cmd("EXPIRE")
                .arg(&redis_key)
                .arg(window.as_secs())
                .query_async::<()>(&mut conn)
                .await;
        }
        if count > max {
            return Err(ApiError::TooManyRequests);
        }
        Ok(())
    }

    /// Tandai sebuah nonce sudah dipakai. Mengembalikan `false` bila nonce
    /// sudah pernah terpakai (indikasi replay).
    pub async fn claim_nonce(&self, nonce: &str, ttl: Duration) -> ApiResult<bool> {
        let Some(mut conn) = self.redis.clone() else {
            return Ok(true);
        };
        let ok: Option<String> = redis::cmd("SET")
            .arg(format!("nonce:{nonce}"))
            .arg(1)
            .arg("NX")
            .arg("EX")
            .arg(ttl.as_secs().max(1))
            .query_async(&mut conn)
            .await
            .unwrap_or(None);
        Ok(ok.is_some())
    }

    /// Publikasikan invalidasi index wajah ke instance lain.
    pub async fn broadcast_face_invalidation(&self, school_id: Uuid) {
        self.face_index.invalidate(school_id);
        if let Some(mut conn) = self.redis.clone() {
            let _: Result<i64, _> = redis::cmd("PUBLISH")
                .arg("absensi:face_index_invalidate")
                .arg(school_id.to_string())
                .query_async(&mut conn)
                .await;
        }
    }

    pub fn uptime(&self) -> Duration {
        self.started_at.elapsed()
    }
}
