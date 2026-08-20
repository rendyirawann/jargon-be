-- =====================================================================
-- 0003 : Multi-tenancy — wilayah, sekolah, tahun ajaran, cakupan akses user
--
-- MODEL TENANT
--   Tenant = SEKOLAH (schools.id).
--   * superadmin / admin_dinas : users.school_id NULL  -> cakupan PROVINSI
--   * kepala_sekolah/guru/staff: users.school_id diisi -> cakupan 1 sekolah
--   * pengawas (opsional)      : baris di user_school_scopes -> N sekolah
--
-- Semua tabel domain membawa kolom school_id sehingga filter tenant
-- selalu bisa didorong ke index (dan ke Row Level Security).
-- =====================================================================

CREATE TABLE IF NOT EXISTS regions (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code       VARCHAR(10)  NOT NULL UNIQUE,   -- kode BPS, mis. '1275'
    name       VARCHAR(150) NOT NULL,          -- 'Kota Medan'
    kind       VARCHAR(20)  NOT NULL DEFAULT 'kabupaten'
               CHECK (kind IN ('provinsi', 'kabupaten', 'kota')),
    parent_id  UUID REFERENCES regions (id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS schools (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    npsn              VARCHAR(12) NOT NULL UNIQUE,       -- Nomor Pokok Sekolah Nasional
    name              VARCHAR(200) NOT NULL,
    slug              VARCHAR(220) NOT NULL UNIQUE,
    jenjang           VARCHAR(10) NOT NULL
                      CHECK (jenjang IN ('PAUD','TK','SD','SMP','SMA','SMK','SLB')),
    status            VARCHAR(10) NOT NULL DEFAULT 'negeri'
                      CHECK (status IN ('negeri','swasta')),
    region_id         UUID REFERENCES regions (id) ON DELETE SET NULL,
    address           TEXT,
    village           VARCHAR(120),
    district          VARCHAR(120),
    postal_code       VARCHAR(10),
    latitude          DOUBLE PRECISION,
    longitude         DOUBLE PRECISION,
    geofence_radius_m INTEGER NOT NULL DEFAULT 250,
    phone             VARCHAR(30),
    email             VARCHAR(150),
    principal_name    VARCHAR(150),
    logo_path         VARCHAR(255),
    timezone          VARCHAR(40) NOT NULL DEFAULT 'Asia/Jakarta',
    -- Ambang kemiripan wajah khusus sekolah ini (NULL = pakai default global).
    face_match_threshold REAL,
    settings          JSONB NOT NULL DEFAULT '{}'::jsonb,
    is_active         BOOLEAN NOT NULL DEFAULT TRUE,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at        TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS schools_region_idx  ON schools (region_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS schools_jenjang_idx ON schools (jenjang)   WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS schools_name_trgm   ON schools USING gin (name gin_trgm_ops);

CREATE TABLE IF NOT EXISTS academic_years (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name       VARCHAR(20) NOT NULL UNIQUE,   -- '2026/2027'
    start_date DATE NOT NULL,
    end_date   DATE NOT NULL,
    is_active  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (end_date > start_date)
);
-- Hanya boleh ada satu tahun ajaran aktif.
CREATE UNIQUE INDEX IF NOT EXISTS academic_years_single_active
    ON academic_years ((is_active)) WHERE is_active;

CREATE TABLE IF NOT EXISTS school_terms (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    academic_year_id UUID NOT NULL REFERENCES academic_years (id) ON DELETE CASCADE,
    name             VARCHAR(20) NOT NULL CHECK (name IN ('Ganjil','Genap')),
    start_date       DATE NOT NULL,
    end_date         DATE NOT NULL,
    is_active        BOOLEAN NOT NULL DEFAULT FALSE,
    UNIQUE (academic_year_id, name)
);

-- ------------------------------------------------------------------
-- Kolom tambahan pada users untuk kebutuhan tenancy & kepegawaian.
-- (users dibuat di 0002 agar Laravel tetap kompatibel.)
-- ------------------------------------------------------------------
ALTER TABLE users ADD COLUMN IF NOT EXISTS school_id     UUID REFERENCES schools (id) ON DELETE SET NULL;
ALTER TABLE users ADD COLUMN IF NOT EXISTS employee_no   VARCHAR(30);   -- NIP / NUPTK
ALTER TABLE users ADD COLUMN IF NOT EXISTS position      VARCHAR(100);  -- jabatan
ALTER TABLE users ADD COLUMN IF NOT EXISTS telegram_chat_id VARCHAR(40);
ALTER TABLE users ADD COLUMN IF NOT EXISTS must_change_password BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE users ADD COLUMN IF NOT EXISTS deleted_at    TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS users_school_idx ON users (school_id) WHERE deleted_at IS NULL;

-- Cakupan tambahan (mis. pengawas yang membina beberapa sekolah).
CREATE TABLE IF NOT EXISTS user_school_scopes (
    user_id    UUID NOT NULL REFERENCES users (id)   ON DELETE CASCADE,
    school_id  UUID NOT NULL REFERENCES schools (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, school_id)
);
CREATE INDEX IF NOT EXISTS user_school_scopes_school_idx ON user_school_scopes (school_id);

-- Refresh token untuk sesi mobile/tablet (rotasi + revoke).
CREATE TABLE IF NOT EXISTS refresh_tokens (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v7(),
    user_id    UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash BYTEA NOT NULL UNIQUE,      -- sha256(token) — token mentah tidak disimpan
    issued_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    replaced_by UUID,
    user_agent TEXT,
    ip_address VARCHAR(45)
);
CREATE INDEX IF NOT EXISTS refresh_tokens_user_idx ON refresh_tokens (user_id) WHERE revoked_at IS NULL;

DROP TRIGGER IF EXISTS trg_schools_updated ON schools;
CREATE TRIGGER trg_schools_updated BEFORE UPDATE ON schools
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
