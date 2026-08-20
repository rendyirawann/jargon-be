-- =====================================================================
-- 0013 : Pemberkasan — unggah berkas kepegawaian guru
--
-- TAHAP INI: unggah dan verifikasi berkas.
--   Guru mengajukan satu "pengajuan" (mis. kenaikan pangkat periode
--   April 2027), mengunggah berkas per jenis dokumen, lalu kepala sekolah
--   dan/atau Dinas memverifikasinya.
--
-- DIRANCANG UNTUK TAHAP BERIKUTNYA
--   Tujuan jangka panjang adalah proses kepegawaian penuh (usulan, nota
--   persetujuan, SK). Karena itu status pengajuan sudah memakai alur yang
--   bisa diperpanjang, dan setiap perubahan status tercatat pada
--   `document_submission_events` — bukan hanya kolom `status` yang ditimpa.
--   Menambah tahap baru nanti tidak akan menghapus riwayat yang sudah ada.
-- =====================================================================

-- ------------------------------------------------------------------
-- Jenis dokumen yang dapat diminta.
--
-- Persyaratan berbeda per keperluan: kenaikan pangkat butuh PAK dan SKP,
-- sertifikasi butuh ijazah dan sertifikat pendidik. Karena itu jenis
-- dokumen ditautkan ke `purpose`, bukan daftar tunggal.
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS document_types (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code          VARCHAR(40) NOT NULL UNIQUE,
    name          VARCHAR(120) NOT NULL,
    description   VARCHAR(300),
    purpose       VARCHAR(30) NOT NULL
                  CHECK (purpose IN ('kenaikan_pangkat', 'sertifikasi', 'tunjangan',
                                     'mutasi', 'pensiun', 'umum')),
    is_required   BOOLEAN NOT NULL DEFAULT TRUE,
    -- Batas ukuran & tipe per jenis dokumen: ijazah hasil pindai wajar
    -- beberapa MB, sementara SK cukup PDF kecil.
    max_bytes     INTEGER NOT NULL DEFAULT 5242880,   -- 5 MB
    allowed_mime  TEXT[] NOT NULL DEFAULT ARRAY['application/pdf', 'image/jpeg', 'image/png'],
    sort_order    SMALLINT NOT NULL DEFAULT 0,
    is_active     BOOLEAN NOT NULL DEFAULT TRUE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS document_types_purpose_idx ON document_types (purpose, sort_order);

-- ------------------------------------------------------------------
-- Pengajuan
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS document_submissions (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    school_id     UUID REFERENCES schools (id) ON DELETE SET NULL,

    purpose       VARCHAR(30) NOT NULL
                  CHECK (purpose IN ('kenaikan_pangkat', 'sertifikasi', 'tunjangan',
                                     'mutasi', 'pensiun', 'umum')),
    -- Periode usulan, mis. "April 2027". Bebas teks karena tiap keperluan
    -- punya penamaan periodenya sendiri.
    period        VARCHAR(40),
    title         VARCHAR(150) NOT NULL,
    note          TEXT,

    -- draft    : masih disusun guru, belum terlihat verifikator
    -- diajukan : menunggu diperiksa
    -- diperiksa: sedang diverifikasi
    -- revisi   : ada berkas yang harus diperbaiki
    -- disetujui / ditolak : final
    status        VARCHAR(15) NOT NULL DEFAULT 'draft'
                  CHECK (status IN ('draft', 'diajukan', 'diperiksa', 'revisi',
                                    'disetujui', 'ditolak')),

    submitted_at  TIMESTAMPTZ,
    reviewed_by   UUID REFERENCES users (id) ON DELETE SET NULL,
    reviewed_at   TIMESTAMPTZ,
    review_note   TEXT,

    -- Ringkasan agar daftar tidak perlu menghitung berkas satu per satu.
    file_count    SMALLINT NOT NULL DEFAULT 0,
    approved_file_count SMALLINT NOT NULL DEFAULT 0,
    rejected_file_count SMALLINT NOT NULL DEFAULT 0,

    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS document_submissions_user_idx   ON document_submissions (user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS document_submissions_school_idx ON document_submissions (school_id, status);
CREATE INDEX IF NOT EXISTS document_submissions_queue_idx
    ON document_submissions (status, submitted_at)
    WHERE status IN ('diajukan', 'diperiksa');

-- ------------------------------------------------------------------
-- Berkas
--
-- Berkas TIDAK disimpan sebagai blob di database, hanya object key menuju
-- storage — sama seperti foto pendaftaran wajah. Ijazah hasil pindai bisa
-- beberapa MB; menaruhnya di kolom akan membuat backup database membengkak
-- dan replikasi melambat.
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS document_files (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    submission_id UUID NOT NULL REFERENCES document_submissions (id) ON DELETE CASCADE,
    document_type_id UUID REFERENCES document_types (id) ON DELETE SET NULL,

    file_key      VARCHAR(300) NOT NULL,
    original_name VARCHAR(200) NOT NULL,
    mime_type     VARCHAR(80) NOT NULL,
    bytes         INTEGER NOT NULL DEFAULT 0,
    -- Deteksi unggahan ganda dan pembuktian keutuhan berkas.
    sha256        BYTEA NOT NULL,

    status        VARCHAR(15) NOT NULL DEFAULT 'menunggu'
                  CHECK (status IN ('menunggu', 'disetujui', 'ditolak')),
    reject_reason VARCHAR(300),
    reviewed_by   UUID REFERENCES users (id) ON DELETE SET NULL,
    reviewed_at   TIMESTAMPTZ,

    uploaded_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS document_files_submission_idx ON document_files (submission_id);
-- Satu jenis dokumen cukup satu berkas per pengajuan; unggahan baru
-- menggantikan yang lama (ditangani aplikasi).
CREATE UNIQUE INDEX IF NOT EXISTS document_files_type_unique
    ON document_files (submission_id, document_type_id)
    WHERE document_type_id IS NOT NULL;

-- ------------------------------------------------------------------
-- Lini masa pengajuan
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS document_submission_events (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v7(),
    submission_id UUID NOT NULL REFERENCES document_submissions (id) ON DELETE CASCADE,
    status        VARCHAR(15) NOT NULL,
    note          VARCHAR(500),
    actor_user_id UUID REFERENCES users (id) ON DELETE SET NULL,
    actor_label   VARCHAR(150),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS document_submission_events_idx
    ON document_submission_events (submission_id, created_at);

-- ------------------------------------------------------------------
-- Penjaga ringkasan berkas
-- ------------------------------------------------------------------
CREATE OR REPLACE FUNCTION sync_document_counts() RETURNS trigger AS $$
DECLARE
    sid UUID;
BEGIN
    sid := COALESCE(NEW.submission_id, OLD.submission_id);
    UPDATE document_submissions s
       SET file_count = (
               SELECT COUNT(*) FROM document_files f WHERE f.submission_id = sid
           ),
           approved_file_count = (
               SELECT COUNT(*) FROM document_files f
                WHERE f.submission_id = sid AND f.status = 'disetujui'
           ),
           rejected_file_count = (
               SELECT COUNT(*) FROM document_files f
                WHERE f.submission_id = sid AND f.status = 'ditolak'
           ),
           updated_at = NOW()
     WHERE s.id = sid;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_document_files_count ON document_files;
CREATE TRIGGER trg_document_files_count
    AFTER INSERT OR UPDATE OF status OR DELETE ON document_files
    FOR EACH ROW EXECUTE FUNCTION sync_document_counts();

DROP TRIGGER IF EXISTS trg_document_submissions_updated ON document_submissions;
CREATE TRIGGER trg_document_submissions_updated BEFORE UPDATE ON document_submissions
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- ------------------------------------------------------------------
-- Jenis dokumen bawaan
-- ------------------------------------------------------------------
INSERT INTO document_types (code, name, description, purpose, is_required, sort_order) VALUES
    -- Kenaikan pangkat
    ('sk_pangkat_terakhir', 'SK Pangkat Terakhir',
     'Salinan SK kenaikan pangkat terakhir yang telah dilegalisir.',
     'kenaikan_pangkat', TRUE, 1),
    ('pak_terakhir', 'PAK Terakhir',
     'Penetapan Angka Kredit terakhir.', 'kenaikan_pangkat', TRUE, 2),
    ('skp_2_tahun', 'SKP 2 Tahun Terakhir',
     'Sasaran Kinerja Pegawai dua tahun terakhir.', 'kenaikan_pangkat', TRUE, 3),
    ('ijazah_terakhir', 'Ijazah Terakhir',
     'Ijazah pendidikan terakhir beserta transkrip.', 'kenaikan_pangkat', TRUE, 4),
    ('karpeg', 'Kartu Pegawai (KARPEG)',
     NULL, 'kenaikan_pangkat', TRUE, 5),
    ('sk_cpns', 'SK CPNS / SK Pengangkatan',
     NULL, 'kenaikan_pangkat', FALSE, 6),
    ('surat_pengantar', 'Surat Pengantar Kepala Sekolah',
     'Surat pengantar dari kepala sekolah.', 'kenaikan_pangkat', TRUE, 7),

    -- Sertifikasi
    ('sertifikat_pendidik', 'Sertifikat Pendidik',
     NULL, 'sertifikasi', TRUE, 1),
    ('sk_pembagian_tugas', 'SK Pembagian Tugas Mengajar',
     'SK pembagian tugas mengajar semester berjalan.', 'sertifikasi', TRUE, 2),
    ('jadwal_mengajar', 'Jadwal Mengajar',
     NULL, 'sertifikasi', TRUE, 3),

    -- Tunjangan
    ('rekening_bank', 'Buku Rekening Bank',
     'Halaman depan buku rekening atas nama sendiri.', 'tunjangan', TRUE, 1),
    ('npwp', 'NPWP', NULL, 'tunjangan', TRUE, 2),

    -- Umum
    ('ktp', 'KTP', NULL, 'umum', TRUE, 1),
    ('kartu_keluarga', 'Kartu Keluarga', NULL, 'umum', FALSE, 2),
    ('dokumen_lain', 'Dokumen Pendukung Lain',
     'Berkas tambahan yang relevan.', 'umum', FALSE, 99)
ON CONFLICT (code) DO NOTHING;
