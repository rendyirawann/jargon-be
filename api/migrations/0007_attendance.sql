-- =====================================================================
-- 0007 : Absensi
--
-- SKALA
--   700.000+ siswa x ~230 hari efektif  = ~160 juta baris/tahun pada
--   `attendances`, dan lebih besar lagi pada `attendance_events`.
--   Keduanya karena itu dipartisi RANGE per bulan. Query dashboard selalu
--   membawa rentang tanggal sehingga planner cukup menyentuh 1-2 partisi.
--
-- ISI BARIS ABSENSI (sesuai kebutuhan: tanpa data wajah)
--   id siswa, nama siswa, id kelas, nama kelas, id sekolah, nama sekolah,
--   jam masuk, jam pulang. Nama disimpan sebagai snapshot supaya rekap
--   historis tetap benar walau siswa pindah kelas/berganti nama.
-- =====================================================================

-- ------------------------------------------------------------------
-- Aturan jam masuk / pulang. Level sekolah, boleh dioverride per kelas.
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS attendance_rules (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id          UUID NOT NULL REFERENCES schools (id) ON DELETE CASCADE,
    classroom_id       UUID REFERENCES classrooms (id) ON DELETE CASCADE, -- NULL = seluruh sekolah
    name               VARCHAR(80) NOT NULL DEFAULT 'Jadwal Reguler',

    check_in_opens_at  TIME NOT NULL DEFAULT '05:30',
    check_in_start_at  TIME NOT NULL DEFAULT '06:30',   -- mulai dihitung hadir
    check_in_due_at    TIME NOT NULL DEFAULT '07:15',   -- lebih dari ini = terlambat
    check_in_closes_at TIME NOT NULL DEFAULT '09:00',   -- lebih dari ini = alfa
    check_out_opens_at TIME NOT NULL DEFAULT '12:00',
    check_out_closes_at TIME NOT NULL DEFAULT '18:00',

    late_grace_minutes SMALLINT NOT NULL DEFAULT 0,
    -- Bitmask hari aktif: bit0=Senin .. bit6=Minggu. 0b0011111 = 31 (Sen-Jum).
    active_weekdays    SMALLINT NOT NULL DEFAULT 31,
    require_check_out  BOOLEAN NOT NULL DEFAULT TRUE,

    effective_from     DATE NOT NULL DEFAULT CURRENT_DATE,
    effective_to       DATE,
    is_active          BOOLEAN NOT NULL DEFAULT TRUE,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (check_in_due_at >= check_in_start_at),
    CHECK (check_in_closes_at >= check_in_due_at)
);
CREATE INDEX IF NOT EXISTS attendance_rules_lookup_idx
    ON attendance_rules (school_id, classroom_id, is_active);

-- Hari libur : NULL school_id = libur nasional/provinsi.
CREATE TABLE IF NOT EXISTS holidays (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id   UUID REFERENCES schools (id) ON DELETE CASCADE,
    holiday_date DATE NOT NULL,
    name        VARCHAR(150) NOT NULL,
    kind        VARCHAR(20) NOT NULL DEFAULT 'nasional'
                CHECK (kind IN ('nasional','provinsi','sekolah','cuti_bersama','ujian')),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS holidays_unique_global
    ON holidays (holiday_date) WHERE school_id IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS holidays_unique_school
    ON holidays (school_id, holiday_date) WHERE school_id IS NOT NULL;

-- ------------------------------------------------------------------
-- attendances : satu baris per siswa per hari (dipartisi per bulan)
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS attendances (
    id                  UUID NOT NULL DEFAULT uuid_generate_v7(),
    attendance_date     DATE NOT NULL,

    school_id           UUID NOT NULL,
    student_id          UUID NOT NULL,
    classroom_id        UUID,
    academic_year_id    UUID,

    -- Snapshot denormalisasi (lihat catatan di header).
    student_name        VARCHAR(150) NOT NULL,
    student_nis         VARCHAR(20),
    classroom_name      VARCHAR(60),
    school_name         VARCHAR(200) NOT NULL,

    check_in_at         TIMESTAMPTZ,
    check_out_at        TIMESTAMPTZ,

    status              VARCHAR(12) NOT NULL DEFAULT 'alfa'
                        CHECK (status IN ('hadir','terlambat','izin','sakit','alfa','dispensasi')),
    late_minutes        INTEGER NOT NULL DEFAULT 0,
    duration_minutes    INTEGER,

    check_in_method     VARCHAR(10) CHECK (check_in_method  IN ('face','manual','import')),
    check_out_method    VARCHAR(10) CHECK (check_out_method IN ('face','manual','import')),
    check_in_device_id  UUID,
    check_out_device_id UUID,
    check_in_similarity REAL,
    check_out_similarity REAL,

    notes               VARCHAR(300),
    marked_by           UUID,     -- diisi bila status diubah manual oleh guru/staff
    marked_at           TIMESTAMPTZ,

    notified_at         TIMESTAMPTZ,
    notification_status VARCHAR(12) NOT NULL DEFAULT 'pending'
                        CHECK (notification_status IN ('pending','queued','sent','failed','skipped')),

    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (attendance_date, id),
    UNIQUE (attendance_date, student_id)
) PARTITION BY RANGE (attendance_date);

-- Index dibuat pada tabel induk -> otomatis diwariskan ke tiap partisi.
CREATE INDEX IF NOT EXISTS attendances_school_date_idx  ON attendances (school_id, attendance_date);
CREATE INDEX IF NOT EXISTS attendances_class_date_idx   ON attendances (classroom_id, attendance_date);
CREATE INDEX IF NOT EXISTS attendances_student_idx      ON attendances (student_id, attendance_date DESC);
CREATE INDEX IF NOT EXISTS attendances_status_idx       ON attendances (school_id, attendance_date, status);
CREATE INDEX IF NOT EXISTS attendances_notify_pending_idx
    ON attendances (attendance_date) WHERE notification_status = 'pending';

-- ------------------------------------------------------------------
-- attendance_events : log mentah setiap percobaan scan (audit + forensik)
--   TIDAK menyimpan gambar maupun embedding. Hanya hash untuk anti-replay.
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS attendance_events (
    id             UUID NOT NULL DEFAULT uuid_generate_v7(),
    occurred_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    school_id      UUID NOT NULL,
    device_id      UUID,
    student_id     UUID,               -- NULL bila wajah tidak dikenali
    attendance_id  UUID,

    event_type     VARCHAR(15) NOT NULL
                   CHECK (event_type IN ('check_in','check_out','unknown','rejected','duplicate','enroll')),
    outcome        VARCHAR(15) NOT NULL
                   CHECK (outcome IN ('accepted','rejected','ignored')),
    reason         VARCHAR(60),        -- 'below_threshold','replay','out_of_window',...

    similarity     REAL,
    liveness_score REAL,
    model_version  VARCHAR(40),
    embedding_hash BYTEA,              -- sha256(embedding) : deteksi pengiriman ulang
    client_time    TIMESTAMPTZ,
    latency_ms     INTEGER,
    ip_address     VARCHAR(45),

    PRIMARY KEY (occurred_at, id)
) PARTITION BY RANGE (occurred_at);

CREATE INDEX IF NOT EXISTS attendance_events_school_time_idx ON attendance_events (school_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS attendance_events_device_time_idx ON attendance_events (device_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS attendance_events_student_idx     ON attendance_events (student_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS attendance_events_replay_idx      ON attendance_events (embedding_hash);

-- ------------------------------------------------------------------
-- Pembuat partisi otomatis. Dipanggil oleh worker `rollup` tiap hari
-- sehingga partisi bulan berikutnya selalu sudah ada sebelum dibutuhkan.
-- ------------------------------------------------------------------
CREATE OR REPLACE FUNCTION ensure_monthly_partition(
    parent      TEXT,
    period_start DATE
) RETURNS VOID AS $$
DECLARE
    part_name TEXT;
    period_end DATE;
BEGIN
    period_start := date_trunc('month', period_start)::date;
    period_end   := (period_start + INTERVAL '1 month')::date;
    part_name    := format('%s_p%s', parent, to_char(period_start, 'YYYYMM'));

    IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = part_name) THEN
        EXECUTE format(
            'CREATE TABLE %I PARTITION OF %I FOR VALUES FROM (%L) TO (%L)',
            part_name, parent, period_start, period_end
        );
    END IF;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION ensure_attendance_partitions(months_ahead INTEGER DEFAULT 3)
RETURNS VOID AS $$
DECLARE
    i INTEGER;
    d DATE;
BEGIN
    FOR i IN -1..months_ahead LOOP
        d := (date_trunc('month', CURRENT_DATE) + (i || ' month')::interval)::date;
        PERFORM ensure_monthly_partition('attendances', d);
        PERFORM ensure_monthly_partition('attendance_events', d);
    END LOOP;
END;
$$ LANGUAGE plpgsql;

-- Partisi DEFAULT sebagai jaring pengaman agar INSERT tidak pernah gagal.
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'attendances_default') THEN
        CREATE TABLE attendances_default PARTITION OF attendances DEFAULT;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'attendance_events_default') THEN
        CREATE TABLE attendance_events_default PARTITION OF attendance_events DEFAULT;
    END IF;
END $$;

SELECT ensure_attendance_partitions(6);

-- ------------------------------------------------------------------
-- Rollup harian per kelas : sumber angka dashboard (tanpa scan 160jt baris)
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS attendance_daily_summary (
    school_id       UUID NOT NULL REFERENCES schools (id) ON DELETE CASCADE,
    -- Kolom PK tidak boleh NULL, jadi baris agregat tingkat sekolah memakai
    -- UUID nil sebagai penanda "seluruh sekolah".
    classroom_id    UUID NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000',
    summary_date    DATE NOT NULL,
    total_students  INTEGER NOT NULL DEFAULT 0,
    hadir           INTEGER NOT NULL DEFAULT 0,
    terlambat       INTEGER NOT NULL DEFAULT 0,
    izin            INTEGER NOT NULL DEFAULT 0,
    sakit           INTEGER NOT NULL DEFAULT 0,
    alfa            INTEGER NOT NULL DEFAULT 0,
    dispensasi      INTEGER NOT NULL DEFAULT 0,
    avg_check_in    TIME,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (school_id, summary_date, classroom_id)
);
CREATE INDEX IF NOT EXISTS attendance_daily_summary_date_idx ON attendance_daily_summary (summary_date DESC);

DROP TRIGGER IF EXISTS trg_attendance_rules_updated ON attendance_rules;
CREATE TRIGGER trg_attendance_rules_updated BEFORE UPDATE ON attendance_rules
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
