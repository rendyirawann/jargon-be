-- =====================================================================
-- 0004 : Rombel (kelas), siswa, wali murid
--
-- CATATAN PENTING
--   Siswa TIDAK punya akun login. Siswa hanya "dikenali" lewat wajah di
--   tablet. Karena itu tidak ada kolom password/email login di sini.
-- =====================================================================

CREATE TABLE IF NOT EXISTS classrooms (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id           UUID NOT NULL REFERENCES schools (id) ON DELETE CASCADE,
    academic_year_id    UUID NOT NULL REFERENCES academic_years (id) ON DELETE RESTRICT,
    name                VARCHAR(60) NOT NULL,      -- 'X IPA 1'
    grade_level         SMALLINT NOT NULL,         -- 1..12
    major               VARCHAR(60),               -- jurusan (SMK/SMA)
    homeroom_teacher_id UUID REFERENCES users (id) ON DELETE SET NULL,  -- wali kelas
    capacity            SMALLINT NOT NULL DEFAULT 40,
    is_active           BOOLEAN NOT NULL DEFAULT TRUE,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at          TIMESTAMPTZ,
    UNIQUE (school_id, academic_year_id, name)
);
CREATE INDEX IF NOT EXISTS classrooms_school_idx   ON classrooms (school_id, academic_year_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS classrooms_homeroom_idx ON classrooms (homeroom_teacher_id) WHERE homeroom_teacher_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS students (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id           UUID NOT NULL REFERENCES schools (id) ON DELETE CASCADE,
    current_classroom_id UUID REFERENCES classrooms (id) ON DELETE SET NULL,

    nisn                VARCHAR(10),               -- unik nasional
    nis                 VARCHAR(20),               -- unik per sekolah
    full_name           VARCHAR(150) NOT NULL,
    gender              CHAR(1) CHECK (gender IN ('L','P')),
    birth_place         VARCHAR(100),
    birth_date          DATE,
    religion            VARCHAR(20),
    address             TEXT,
    phone               VARCHAR(20),
    photo_path          VARCHAR(255),              -- pas foto (bukan data biometrik)

    father_name         VARCHAR(150),
    mother_name         VARCHAR(150),

    status              VARCHAR(15) NOT NULL DEFAULT 'aktif'
                        CHECK (status IN ('aktif','lulus','pindah','keluar','cuti')),
    entry_year          SMALLINT,

    -- Ringkasan status biometrik agar daftar siswa tidak perlu join.
    face_enrolled       BOOLEAN NOT NULL DEFAULT FALSE,
    face_enrolled_at    TIMESTAMPTZ,
    face_sample_count   SMALLINT NOT NULL DEFAULT 0,

    metadata            JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at          TIMESTAMPTZ
);
CREATE UNIQUE INDEX IF NOT EXISTS students_nisn_unique ON students (nisn) WHERE nisn IS NOT NULL AND deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS students_school_nis_unique ON students (school_id, nis) WHERE nis IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS students_school_status_idx ON students (school_id, status) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS students_classroom_idx     ON students (current_classroom_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS students_name_trgm         ON students USING gin (full_name gin_trgm_ops);
CREATE INDEX IF NOT EXISTS students_face_pending_idx  ON students (school_id) WHERE face_enrolled = FALSE AND deleted_at IS NULL;

-- Riwayat penempatan kelas per tahun ajaran (untuk rapor & mutasi).
CREATE TABLE IF NOT EXISTS student_class_enrollments (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id       UUID NOT NULL REFERENCES students (id) ON DELETE CASCADE,
    classroom_id     UUID NOT NULL REFERENCES classrooms (id) ON DELETE CASCADE,
    school_id        UUID NOT NULL REFERENCES schools (id) ON DELETE CASCADE,
    academic_year_id UUID NOT NULL REFERENCES academic_years (id) ON DELETE CASCADE,
    started_at       DATE NOT NULL DEFAULT CURRENT_DATE,
    ended_at         DATE,
    is_current       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS student_class_current_unique
    ON student_class_enrollments (student_id, academic_year_id) WHERE is_current;
CREATE INDEX IF NOT EXISTS student_class_classroom_idx ON student_class_enrollments (classroom_id);

-- Wali murid / orang tua : target notifikasi WA / Telegram / Email.
CREATE TABLE IF NOT EXISTS student_guardians (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id        UUID NOT NULL REFERENCES students (id) ON DELETE CASCADE,
    school_id         UUID NOT NULL REFERENCES schools (id) ON DELETE CASCADE,
    relation          VARCHAR(20) NOT NULL DEFAULT 'wali'
                      CHECK (relation IN ('ayah','ibu','wali')),
    full_name         VARCHAR(150) NOT NULL,
    phone             VARCHAR(20),
    whatsapp          VARCHAR(20),
    email             VARCHAR(150),
    telegram_chat_id  VARCHAR(40),
    -- Kanal yang dipakai untuk mengirim notifikasi absensi anak ini.
    preferred_channel VARCHAR(15) NOT NULL DEFAULT 'whatsapp'
                      CHECK (preferred_channel IN ('whatsapp','telegram','email','none')),
    is_primary        BOOLEAN NOT NULL DEFAULT FALSE,
    notify_enabled    BOOLEAN NOT NULL DEFAULT TRUE,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS student_guardians_student_idx ON student_guardians (student_id);
CREATE UNIQUE INDEX IF NOT EXISTS student_guardians_primary_unique
    ON student_guardians (student_id) WHERE is_primary;

DROP TRIGGER IF EXISTS trg_students_updated ON students;
CREATE TRIGGER trg_students_updated BEFORE UPDATE ON students
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

DROP TRIGGER IF EXISTS trg_classrooms_updated ON classrooms;
CREATE TRIGGER trg_classrooms_updated BEFORE UPDATE ON classrooms
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
