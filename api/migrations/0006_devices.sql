-- =====================================================================
-- 0006 : Perangkat (tablet) di sekolah
--
-- Tablet TIDAK memakai akun user. Tiap tablet dipasangkan (pairing) sekali
-- oleh operator sekolah lalu memegang device token jangka panjang + secret
-- HMAC untuk menandatangani setiap request absensi (anti replay/spoof).
-- =====================================================================

CREATE TABLE IF NOT EXISTS devices (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id     UUID NOT NULL REFERENCES schools (id) ON DELETE CASCADE,
    code          VARCHAR(40) NOT NULL UNIQUE,   -- 'MDN-SMA1-GATE-01'
    name          VARCHAR(120) NOT NULL,

    placement     VARCHAR(20) NOT NULL DEFAULT 'gate'
                  CHECK (placement IN ('gate','classroom','office','mobile')),
    classroom_id  UUID REFERENCES classrooms (id) ON DELETE SET NULL,

    -- auto = tentukan masuk/pulang dari jam & state absensi hari ini.
    mode          VARCHAR(15) NOT NULL DEFAULT 'auto'
                  CHECK (mode IN ('auto','check_in','check_out','enroll')),

    token_hash    BYTEA,                         -- sha256(device token)
    hmac_secret   BYTEA,                         -- kunci penandatangan request
    token_issued_at TIMESTAMPTZ,
    token_revoked_at TIMESTAMPTZ,

    pairing_code  VARCHAR(12),                   -- kode 8 digit sekali pakai
    pairing_expires_at TIMESTAMPTZ,

    app_version   VARCHAR(30),
    os_version    VARCHAR(60),
    last_seen_at  TIMESTAMPTZ,
    last_ip       VARCHAR(45),
    latitude      DOUBLE PRECISION,
    longitude     DOUBLE PRECISION,

    is_active     BOOLEAN NOT NULL DEFAULT TRUE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at    TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS devices_school_idx ON devices (school_id) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS devices_token_hash_unique ON devices (token_hash) WHERE token_hash IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS devices_pairing_code_unique ON devices (pairing_code) WHERE pairing_code IS NOT NULL;

ALTER TABLE face_enrollments
    DROP CONSTRAINT IF EXISTS face_enrollments_device_fk;
ALTER TABLE face_enrollments
    ADD CONSTRAINT face_enrollments_device_fk
    FOREIGN KEY (device_id) REFERENCES devices (id) ON DELETE SET NULL;

-- Heartbeat ringkas untuk memantau tablet mana yang offline.
CREATE TABLE IF NOT EXISTS device_heartbeats (
    id            BIGSERIAL PRIMARY KEY,
    device_id     UUID NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    school_id     UUID NOT NULL REFERENCES schools (id) ON DELETE CASCADE,
    reported_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    battery_pct   SMALLINT,
    queued_events INTEGER NOT NULL DEFAULT 0,
    app_version   VARCHAR(30),
    network       VARCHAR(20),
    embedding_model_version VARCHAR(40)
);
CREATE INDEX IF NOT EXISTS device_heartbeats_device_time_idx
    ON device_heartbeats (device_id, reported_at DESC);

-- Klien server-to-server (mis. dashboard Laravel memanggil API Rust).
CREATE TABLE IF NOT EXISTS api_clients (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        VARCHAR(80) NOT NULL UNIQUE,
    key_id      VARCHAR(40) NOT NULL UNIQUE,
    secret_hash BYTEA NOT NULL,
    scopes      TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    school_id   UUID REFERENCES schools (id) ON DELETE CASCADE,  -- NULL = provinsi
    is_active   BOOLEAN NOT NULL DEFAULT TRUE,
    last_used_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

DROP TRIGGER IF EXISTS trg_devices_updated ON devices;
CREATE TRIGGER trg_devices_updated BEFORE UPDATE ON devices
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
