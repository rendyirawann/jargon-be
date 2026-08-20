-- =====================================================================
-- 0005 : Biometrik wajah
--
-- ATURAN PRIVASI (ditegakkan oleh skema, bukan sekadar konvensi)
--   1. GAMBAR WAJAH hanya tersimpan pada face_enrollments — yaitu saat
--      pendaftaran awal siswa. Tabel absensi & log scan tidak punya kolom
--      untuk menyimpan gambar maupun vektor.
--   2. Saat absen harian, tablet HANYA mengirim embedding (vektor 512-d).
--      Server memakainya untuk pencocokan lalu MEMBUANGNYA. Yang disimpan
--      pada log hanya hash (embedding_hash) untuk deteksi replay.
--   3. Menghapus siswa akan meng-cascade seluruh data biometriknya.
-- =====================================================================

-- Dimensi embedding mengikuti MobileFaceNet / ArcFace = 512.
-- Nilai SELALU disimpan sudah L2-normalized sehingga cosine distance
-- (<=>) setara dengan inner product — cepat dan stabil.

CREATE TABLE IF NOT EXISTS face_enrollments (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id     UUID NOT NULL REFERENCES students (id) ON DELETE CASCADE,
    school_id      UUID NOT NULL REFERENCES schools (id)  ON DELETE CASCADE,

    -- Object key pada storage (lokal / S3-compatible). Bukan blob di DB.
    image_key      VARCHAR(300) NOT NULL,
    image_sha256   BYTEA NOT NULL,
    image_bytes    INTEGER NOT NULL DEFAULT 0,
    mime_type      VARCHAR(40) NOT NULL DEFAULT 'image/jpeg',

    pose           VARCHAR(20) NOT NULL DEFAULT 'frontal'
                   CHECK (pose IN ('frontal','left','right','up','down')),
    quality_score  REAL,          -- 0..1 gabungan blur/pencahayaan/ukuran wajah
    quality_detail JSONB NOT NULL DEFAULT '{}'::jsonb,

    status         VARCHAR(15) NOT NULL DEFAULT 'approved'
                   CHECK (status IN ('pending','approved','rejected','replaced')),
    reject_reason  VARCHAR(200),
    reviewed_by    UUID REFERENCES users (id) ON DELETE SET NULL,
    reviewed_at    TIMESTAMPTZ,

    captured_by    UUID REFERENCES users (id) ON DELETE SET NULL,
    device_id      UUID,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS face_enrollments_student_idx ON face_enrollments (student_id);
CREATE INDEX IF NOT EXISTS face_enrollments_school_status_idx ON face_enrollments (school_id, status);
CREATE UNIQUE INDEX IF NOT EXISTS face_enrollments_dedup
    ON face_enrollments (student_id, image_sha256);

CREATE TABLE IF NOT EXISTS face_embeddings (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id    UUID NOT NULL REFERENCES students (id) ON DELETE CASCADE,
    -- school_id didenormalisasi: pencarian kNN SELALU dibatasi per sekolah
    -- sehingga kandidat mengecil dari 700rb menjadi ratusan.
    school_id     UUID NOT NULL REFERENCES schools (id) ON DELETE CASCADE,
    enrollment_id UUID REFERENCES face_enrollments (id) ON DELETE CASCADE,

    embedding     VECTOR(512) NOT NULL,
    model_version VARCHAR(40) NOT NULL DEFAULT 'mobilefacenet-v1',
    quality_score REAL,
    is_active     BOOLEAN NOT NULL DEFAULT TRUE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS face_embeddings_school_active_idx
    ON face_embeddings (school_id) WHERE is_active;
CREATE INDEX IF NOT EXISTS face_embeddings_student_idx ON face_embeddings (student_id);
CREATE INDEX IF NOT EXISTS face_embeddings_model_idx   ON face_embeddings (model_version);

-- HNSW cosine. Dipakai sebagai jaring pengaman / verifikasi silang; jalur
-- panas sebenarnya memakai cache vektor per-sekolah di dalam proses Rust
-- (lihat src/face/index.rs) yang melakukan pencarian eksak dengan SIMD.
CREATE INDEX IF NOT EXISTS face_embeddings_hnsw
    ON face_embeddings USING hnsw (embedding vector_cosine_ops)
    WITH (m = 16, ef_construction = 64);

-- Menjaga ringkasan pada students tetap sinkron.
CREATE OR REPLACE FUNCTION sync_student_face_summary() RETURNS trigger AS $$
DECLARE
    sid UUID;
    cnt INTEGER;
BEGIN
    sid := COALESCE(NEW.student_id, OLD.student_id);
    SELECT COUNT(*) INTO cnt FROM face_embeddings WHERE student_id = sid AND is_active;
    UPDATE students
       SET face_sample_count = cnt,
           face_enrolled     = (cnt > 0),
           face_enrolled_at  = CASE WHEN cnt > 0 AND face_enrolled_at IS NULL
                                    THEN NOW() ELSE face_enrolled_at END
     WHERE id = sid;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_face_embeddings_sync ON face_embeddings;
CREATE TRIGGER trg_face_embeddings_sync
    AFTER INSERT OR UPDATE OF is_active OR DELETE ON face_embeddings
    FOR EACH ROW EXECUTE FUNCTION sync_student_face_summary();
