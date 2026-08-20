-- =====================================================================
-- 0009 : Operasional — audit API, impor massal, ekspor laporan
-- =====================================================================

-- Audit sisi API (terpisah dari activity_log milik dashboard Laravel).
CREATE TABLE IF NOT EXISTS audit_logs (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v7(),
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    actor_type  VARCHAR(15) NOT NULL CHECK (actor_type IN ('user','device','api_client','system')),
    actor_id    UUID,
    actor_label VARCHAR(150),
    school_id   UUID,
    action      VARCHAR(80) NOT NULL,        -- 'student.create', 'attendance.override', ...
    entity_type VARCHAR(60),
    entity_id   UUID,
    before      JSONB,
    after       JSONB,
    ip_address  VARCHAR(45),
    user_agent  TEXT,
    request_id  VARCHAR(60)
);
CREATE INDEX IF NOT EXISTS audit_logs_school_time_idx ON audit_logs (school_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS audit_logs_actor_idx       ON audit_logs (actor_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS audit_logs_entity_idx      ON audit_logs (entity_type, entity_id);

-- Impor massal siswa (CSV/XLSX). Wajib untuk onboarding 700rb+ siswa.
CREATE TABLE IF NOT EXISTS import_jobs (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id     UUID REFERENCES schools (id) ON DELETE CASCADE,
    kind          VARCHAR(20) NOT NULL DEFAULT 'students'
                  CHECK (kind IN ('students','classrooms','guardians','schools','users')),
    source_key    VARCHAR(300) NOT NULL,     -- object key file yang diunggah
    original_name VARCHAR(200),
    status        VARCHAR(15) NOT NULL DEFAULT 'pending'
                  CHECK (status IN ('pending','running','completed','failed','cancelled')),
    total_rows    INTEGER NOT NULL DEFAULT 0,
    processed_rows INTEGER NOT NULL DEFAULT 0,
    success_rows  INTEGER NOT NULL DEFAULT 0,
    failed_rows   INTEGER NOT NULL DEFAULT 0,
    error_report_key VARCHAR(300),
    dry_run       BOOLEAN NOT NULL DEFAULT FALSE,
    created_by    UUID REFERENCES users (id) ON DELETE SET NULL,
    started_at    TIMESTAMPTZ,
    finished_at   TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS import_jobs_school_idx ON import_jobs (school_id, created_at DESC);
CREATE INDEX IF NOT EXISTS import_jobs_status_idx ON import_jobs (status) WHERE status IN ('pending','running');

CREATE TABLE IF NOT EXISTS import_job_errors (
    id            BIGSERIAL PRIMARY KEY,
    import_job_id UUID NOT NULL REFERENCES import_jobs (id) ON DELETE CASCADE,
    row_number    INTEGER NOT NULL,
    column_name   VARCHAR(60),
    raw_value     TEXT,
    message       VARCHAR(300) NOT NULL
);
CREATE INDEX IF NOT EXISTS import_job_errors_job_idx ON import_job_errors (import_job_id, row_number);

-- Ekspor laporan asinkron (rekap bulanan bisa puluhan MB).
CREATE TABLE IF NOT EXISTS report_exports (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id   UUID REFERENCES schools (id) ON DELETE CASCADE,
    report_key  VARCHAR(60) NOT NULL,
    format      VARCHAR(10) NOT NULL DEFAULT 'xlsx' CHECK (format IN ('xlsx','csv','pdf')),
    params      JSONB NOT NULL DEFAULT '{}'::jsonb,
    status      VARCHAR(15) NOT NULL DEFAULT 'pending'
                CHECK (status IN ('pending','running','completed','failed')),
    result_key  VARCHAR(300),
    row_count   INTEGER,
    error       TEXT,
    requested_by UUID REFERENCES users (id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ,
    expires_at  TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS report_exports_school_idx ON report_exports (school_id, created_at DESC);

-- Kunci idempotensi: melindungi endpoint absensi dari retry ganda tablet
-- yang jaringannya putus-nyambung.
CREATE TABLE IF NOT EXISTS idempotency_keys (
    key         VARCHAR(120) PRIMARY KEY,
    scope       VARCHAR(60) NOT NULL,
    response    JSONB,
    status_code SMALLINT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at  TIMESTAMPTZ NOT NULL DEFAULT NOW() + INTERVAL '24 hours'
);
CREATE INDEX IF NOT EXISTS idempotency_keys_expiry_idx ON idempotency_keys (expires_at);
