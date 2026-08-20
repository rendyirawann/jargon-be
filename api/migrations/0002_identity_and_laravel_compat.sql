-- =====================================================================
-- 0002 : Identitas (users) + tabel infrastruktur Laravel
--
-- Database ini dipakai BERSAMA oleh dua service:
--   * absensi-be/api   (Rust / Axum)     -> hot path: face recognition, absensi, API
--   * absensi-be/admin (Laravel Octane)  -> dashboard /admin
--
-- sqlx adalah SUMBER KEBENARAN skema. Migration Laravel sudah di-guard
-- dengan Schema::hasTable() sehingga `php artisan migrate` menjadi no-op
-- yang aman. Bentuk kolom di bawah sengaja dibuat 1:1 dengan ekspektasi
-- Laravel (spatie/laravel-permission, spatie/laravel-activitylog,
-- cybercog/laravel-ban) agar Eloquent tetap bekerja tanpa modifikasi.
-- =====================================================================

CREATE TABLE IF NOT EXISTS users (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name              VARCHAR(255) NOT NULL,
    username          VARCHAR(255) NOT NULL,
    email             VARCHAR(255) NOT NULL,
    no_wa             VARCHAR(20),
    avatar            VARCHAR(255),
    last_ip           VARCHAR(45),
    last_login        TIMESTAMPTZ,
    banned_at         TIMESTAMPTZ,
    nik               VARCHAR(16),
    phone             VARCHAR(15),
    email_verified_at TIMESTAMPTZ,
    password          VARCHAR(255) NOT NULL,
    is_active         BOOLEAN NOT NULL DEFAULT TRUE,
    remember_token    VARCHAR(100),
    social_id         VARCHAR(255),
    social_type       VARCHAR(255),
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS users_username_unique ON users (username);
CREATE UNIQUE INDEX IF NOT EXISTS users_email_unique    ON users (email);

CREATE TABLE IF NOT EXISTS password_reset_tokens (
    email      VARCHAR(255) PRIMARY KEY,
    token      VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS sessions (
    id            VARCHAR(255) PRIMARY KEY,
    user_id       UUID,
    ip_address    VARCHAR(45),
    user_agent    TEXT,
    payload       TEXT NOT NULL,
    last_activity INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS sessions_user_id_index       ON sessions (user_id);
CREATE INDEX IF NOT EXISTS sessions_last_activity_index ON sessions (last_activity);

CREATE TABLE IF NOT EXISTS cache (
    key        VARCHAR(255) PRIMARY KEY,
    value      TEXT NOT NULL,
    expiration INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS cache_locks (
    key        VARCHAR(255) PRIMARY KEY,
    owner      VARCHAR(255) NOT NULL,
    expiration INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS jobs (
    id           BIGSERIAL PRIMARY KEY,
    queue        VARCHAR(255) NOT NULL,
    payload      TEXT NOT NULL,
    attempts     SMALLINT NOT NULL,
    reserved_at  INTEGER,
    available_at INTEGER NOT NULL,
    created_at   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS jobs_queue_index ON jobs (queue);

CREATE TABLE IF NOT EXISTS job_batches (
    id             VARCHAR(255) PRIMARY KEY,
    name           VARCHAR(255) NOT NULL,
    total_jobs     INTEGER NOT NULL,
    pending_jobs   INTEGER NOT NULL,
    failed_jobs    INTEGER NOT NULL,
    failed_job_ids TEXT NOT NULL,
    options        TEXT,
    cancelled_at   INTEGER,
    created_at     INTEGER NOT NULL,
    finished_at    INTEGER
);

CREATE TABLE IF NOT EXISTS failed_jobs (
    id         BIGSERIAL PRIMARY KEY,
    uuid       VARCHAR(255) NOT NULL UNIQUE,
    connection TEXT NOT NULL,
    queue      TEXT NOT NULL,
    payload    TEXT NOT NULL,
    exception  TEXT NOT NULL,
    failed_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ------------------------------------------------------------------
-- spatie/laravel-permission (teams = false, model_morph_key = model_id)
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS permissions (
    id         BIGSERIAL PRIMARY KEY,
    name       VARCHAR(255) NOT NULL,
    category   VARCHAR(255),
    guard_name VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    CONSTRAINT permissions_name_guard_name_unique UNIQUE (name, guard_name)
);

CREATE TABLE IF NOT EXISTS roles (
    id         BIGSERIAL PRIMARY KEY,
    name       VARCHAR(255) NOT NULL,
    guard_name VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    CONSTRAINT roles_name_guard_name_unique UNIQUE (name, guard_name)
);

CREATE TABLE IF NOT EXISTS model_has_permissions (
    permission_id BIGINT NOT NULL REFERENCES permissions (id) ON DELETE CASCADE,
    model_type    VARCHAR(255) NOT NULL,
    model_id      UUID NOT NULL,
    CONSTRAINT model_has_permissions_permission_model_type_primary
        PRIMARY KEY (permission_id, model_id, model_type)
);
CREATE INDEX IF NOT EXISTS model_has_permissions_model_id_model_type_index
    ON model_has_permissions (model_id, model_type);

CREATE TABLE IF NOT EXISTS model_has_roles (
    role_id    BIGINT NOT NULL REFERENCES roles (id) ON DELETE CASCADE,
    model_type VARCHAR(255) NOT NULL,
    model_id   UUID NOT NULL,
    CONSTRAINT model_has_roles_role_model_type_primary
        PRIMARY KEY (role_id, model_id, model_type)
);
CREATE INDEX IF NOT EXISTS model_has_roles_model_id_model_type_index
    ON model_has_roles (model_id, model_type);

CREATE TABLE IF NOT EXISTS role_has_permissions (
    permission_id BIGINT NOT NULL REFERENCES permissions (id) ON DELETE CASCADE,
    role_id       BIGINT NOT NULL REFERENCES roles (id) ON DELETE CASCADE,
    CONSTRAINT role_has_permissions_permission_id_role_id_primary
        PRIMARY KEY (permission_id, role_id)
);

-- ------------------------------------------------------------------
-- spatie/laravel-activitylog (nullableUuidMorphs)
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS activity_log (
    id           BIGSERIAL PRIMARY KEY,
    log_name     VARCHAR(255),
    event        VARCHAR(255),
    description  TEXT NOT NULL,
    subject_type VARCHAR(255),
    subject_id   UUID,
    causer_type  VARCHAR(255),
    causer_id    UUID,
    properties   JSONB,
    batch_uuid   UUID,
    created_at   TIMESTAMPTZ,
    updated_at   TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS activity_log_log_name_index ON activity_log (log_name);
CREATE INDEX IF NOT EXISTS subject               ON activity_log (subject_type, subject_id);
CREATE INDEX IF NOT EXISTS causer                ON activity_log (causer_type, causer_id);
CREATE INDEX IF NOT EXISTS activity_log_created_at_index ON activity_log (created_at DESC);

-- ------------------------------------------------------------------
-- cybercog/laravel-ban
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS bans (
    id              BIGSERIAL PRIMARY KEY,
    bannable_type   VARCHAR(255) NOT NULL,
    bannable_id     UUID NOT NULL,
    created_by_type VARCHAR(255),
    created_by_id   UUID,
    comment         TEXT,
    expired_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ,
    updated_at      TIMESTAMPTZ,
    deleted_at      TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS bans_bannable_index   ON bans (bannable_type, bannable_id);
CREATE INDEX IF NOT EXISTS bans_created_by_index ON bans (created_by_type, created_by_id);

-- ------------------------------------------------------------------
-- settings (key/value dashboard)
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS settings (
    id         BIGSERIAL PRIMARY KEY,
    key        VARCHAR(255) NOT NULL UNIQUE,
    value      TEXT,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
);

-- ------------------------------------------------------------------
-- Tabel migrations milik Laravel (agar `php artisan migrate:status` waras)
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS migrations (
    id        SERIAL PRIMARY KEY,
    migration VARCHAR(255) NOT NULL,
    batch     INTEGER NOT NULL
);
