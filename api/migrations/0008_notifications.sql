-- =====================================================================
-- 0008 : Notifikasi ke orang tua (WhatsApp / Telegram / Email)
--
-- Pola: TRANSACTIONAL OUTBOX.
--   Absensi tercatat dan pesan dimasukkan ke outbox dalam SATU transaksi.
--   Worker terpisah (src/workers/outbox.rs) yang mengirim dengan retry
--   eksponensial. Jadi provider WA yang sedang down tidak pernah membuat
--   absensi siswa gagal tercatat.
-- =====================================================================

CREATE TABLE IF NOT EXISTS notification_templates (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id   UUID REFERENCES schools (id) ON DELETE CASCADE,  -- NULL = template bawaan
    key         VARCHAR(40) NOT NULL
                CHECK (key IN ('check_in','check_out','late','absent','sick','permit','daily_recap','weekly_recap','custom')),
    channel     VARCHAR(15) NOT NULL
                CHECK (channel IN ('whatsapp','telegram','email')),
    subject     VARCHAR(200),                 -- dipakai email
    body        TEXT NOT NULL,
    is_active   BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS notification_templates_default_unique
    ON notification_templates (key, channel) WHERE school_id IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS notification_templates_school_unique
    ON notification_templates (school_id, key, channel) WHERE school_id IS NOT NULL;

-- Kredensial provider per sekolah (boleh pakai kredensial provinsi bila NULL).
CREATE TABLE IF NOT EXISTS notification_channels (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id   UUID REFERENCES schools (id) ON DELETE CASCADE,
    channel     VARCHAR(15) NOT NULL CHECK (channel IN ('whatsapp','telegram','email')),
    provider    VARCHAR(30) NOT NULL,   -- 'meta_cloud','fonnte','wablas','telegram_bot','smtp'
    -- Disimpan terenkripsi di level aplikasi (AES-GCM, kunci dari ENV).
    credentials JSONB NOT NULL DEFAULT '{}'::jsonb,
    rate_limit_per_minute INTEGER NOT NULL DEFAULT 60,
    is_active   BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS notification_channels_unique
    ON notification_channels (COALESCE(school_id, '00000000-0000-0000-0000-000000000000'::uuid), channel);

-- Preferensi notifikasi per sekolah: kejadian apa saja yang dikirim.
CREATE TABLE IF NOT EXISTS notification_policies (
    school_id            UUID PRIMARY KEY REFERENCES schools (id) ON DELETE CASCADE,
    notify_on_check_in   BOOLEAN NOT NULL DEFAULT TRUE,
    notify_on_check_out  BOOLEAN NOT NULL DEFAULT FALSE,
    notify_on_late       BOOLEAN NOT NULL DEFAULT TRUE,
    notify_on_absent     BOOLEAN NOT NULL DEFAULT TRUE,
    -- Notifikasi alfa dikirim setelah jam ini (setelah gerbang absen tutup).
    absent_notify_after  TIME NOT NULL DEFAULT '09:30',
    quiet_hours_start    TIME,
    quiet_hours_end      TIME,
    daily_recap_at       TIME,
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ------------------------------------------------------------------
-- Outbox (dipartisi per bulan; volume ~700rb pesan/hari saat penuh)
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS notification_outbox (
    id            UUID NOT NULL DEFAULT uuid_generate_v7(),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    school_id     UUID NOT NULL,
    student_id    UUID,
    guardian_id   UUID,
    attendance_id UUID,

    channel       VARCHAR(15) NOT NULL CHECK (channel IN ('whatsapp','telegram','email')),
    template_key  VARCHAR(40) NOT NULL,
    recipient     VARCHAR(200) NOT NULL,     -- no. WA / chat id / email
    subject       VARCHAR(200),
    body          TEXT NOT NULL,
    variables     JSONB NOT NULL DEFAULT '{}'::jsonb,

    status        VARCHAR(12) NOT NULL DEFAULT 'queued'
                  CHECK (status IN ('queued','sending','sent','failed','cancelled')),
    attempts      SMALLINT NOT NULL DEFAULT 0,
    max_attempts  SMALLINT NOT NULL DEFAULT 5,
    scheduled_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    locked_at     TIMESTAMPTZ,
    locked_by     VARCHAR(60),
    sent_at       TIMESTAMPTZ,
    provider      VARCHAR(30),
    provider_message_id VARCHAR(120),
    last_error    TEXT,

    PRIMARY KEY (created_at, id)
) PARTITION BY RANGE (created_at);

-- Index antrian: worker mengambil pekerjaan dengan status+scheduled_at.
CREATE INDEX IF NOT EXISTS notification_outbox_claim_idx
    ON notification_outbox (scheduled_at)
    WHERE status IN ('queued', 'sending');
CREATE INDEX IF NOT EXISTS notification_outbox_school_idx  ON notification_outbox (school_id, created_at DESC);
CREATE INDEX IF NOT EXISTS notification_outbox_student_idx ON notification_outbox (student_id, created_at DESC);
CREATE INDEX IF NOT EXISTS notification_outbox_status_idx  ON notification_outbox (status, created_at DESC);

CREATE OR REPLACE FUNCTION ensure_outbox_partitions(months_ahead INTEGER DEFAULT 3)
RETURNS VOID AS $$
DECLARE
    i INTEGER;
    d DATE;
BEGIN
    FOR i IN -1..months_ahead LOOP
        d := (date_trunc('month', CURRENT_DATE) + (i || ' month')::interval)::date;
        PERFORM ensure_monthly_partition('notification_outbox', d);
    END LOOP;
END;
$$ LANGUAGE plpgsql;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'notification_outbox_default') THEN
        CREATE TABLE notification_outbox_default PARTITION OF notification_outbox DEFAULT;
    END IF;
END $$;

SELECT ensure_outbox_partitions(6);
