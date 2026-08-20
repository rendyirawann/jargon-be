-- =====================================================================
-- 0012 : Panic Button — kanal pengaduan anonim
--
-- APA INI
--   Platform bergaya media sosial tempat siswa (dan warga sekolah lain)
--   melaporkan pungli, perundungan, kekerasan, atau masalah lain di
--   sekolahnya. Postingan tampil ANONIM.
--
-- MENGAPA ANONIMITASNYA KRITIS
--   Pelapor perundungan dan pungli hampir selalu berada pada posisi lemah
--   terhadap pihak yang dilaporkan — kadang gurunya sendiri, kadang kepala
--   sekolahnya. Bila identitas pelapor bisa dilihat pihak sekolah, platform
--   ini bukan sekadar tidak berguna: ia menjadi alat untuk menemukan siapa
--   yang berani bicara. Karena itu:
--
--     1. `author_user_id` TIDAK PERNAH dikembalikan API kepada peran tingkat
--        sekolah. View `panic_reports_feed` sengaja tidak memuat kolom itu.
--     2. Hanya peran provinsi dengan izin `unmask_panic_report` yang dapat
--        membukanya, dan SETIAP pembukaan wajib menyertakan alasan serta
--        tercatat permanen di `panic_unmask_logs`.
--     3. Nama tampilan berupa `anonymous_handle` yang tidak dapat dibalik
--        menjadi identitas.
--
--   Konsekuensi yang diterima: penyalahgunaan (laporan palsu) ditangani
--   lewat moderasi dan rekam jejak akun, bukan dengan membuka identitas.
-- =====================================================================

CREATE TABLE IF NOT EXISTS panic_categories (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code         VARCHAR(30) NOT NULL UNIQUE,
    name         VARCHAR(80) NOT NULL,
    description  VARCHAR(300),
    icon         VARCHAR(40),
    -- Kategori tertentu (mis. kekerasan fisik) langsung diperlakukan mendesak.
    default_severity VARCHAR(10) NOT NULL DEFAULT 'sedang'
                 CHECK (default_severity IN ('rendah', 'sedang', 'tinggi', 'darurat')),
    sort_order   SMALLINT NOT NULL DEFAULT 0,
    is_active    BOOLEAN NOT NULL DEFAULT TRUE
);

-- ------------------------------------------------------------------
-- Laporan
--
-- Dipartisi per bulan seperti tabel bervolume tinggi lainnya: pada 700rb
-- siswa, feed pengaduan bisa tumbuh cepat dan query feed selalu membawa
-- batas waktu.
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS panic_reports (
    id            UUID NOT NULL DEFAULT uuid_generate_v7(),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Sekolah yang dilaporkan. Diambil dari akun pelapor, tidak diisi manual,
    -- agar seorang siswa tidak bisa mengarang laporan atas nama sekolah lain.
    school_id     UUID NOT NULL,
    category_id   UUID NOT NULL REFERENCES panic_categories (id) ON DELETE RESTRICT,

    -- ========== RAHASIA ==========
    -- Kolom ini tidak boleh keluar lewat API mana pun kecuali endpoint
    -- unmask yang dijaga izin khusus dan dicatat.
    author_user_id UUID NOT NULL,
    author_role    VARCHAR(30) NOT NULL,
    -- =============================

    -- Nama tampilan, mis. "Siswa#4F7A". Dibuat acak saat laporan dibuat dan
    -- tidak dapat dibalik menjadi identitas.
    anonymous_handle VARCHAR(24) NOT NULL,

    title         VARCHAR(150) NOT NULL,
    body          TEXT NOT NULL,

    severity      VARCHAR(10) NOT NULL DEFAULT 'sedang'
                  CHECK (severity IN ('rendah', 'sedang', 'tinggi', 'darurat')),

    -- Alur penanganan.
    status        VARCHAR(20) NOT NULL DEFAULT 'baru'
                  CHECK (status IN ('baru', 'diverifikasi', 'ditindaklanjuti', 'selesai', 'ditolak')),

    -- Moderasi tampil-di-feed. Laporan yang belum disetujui tetap terlihat
    -- oleh pelapornya sendiri dan oleh Dinas — moderasi hanya menahan
    -- tampilnya di feed publik, bukan menahan penanganannya.
    moderation_status VARCHAR(15) NOT NULL DEFAULT 'pending'
                  CHECK (moderation_status IN ('pending', 'approved', 'rejected')),
    moderation_note VARCHAR(300),
    moderated_by  UUID,
    moderated_at  TIMESTAMPTZ,

    -- `terbatas` = tidak pernah tampil di feed publik, hanya untuk Dinas.
    -- Dipilih pelapor untuk kasus yang sangat sensitif.
    visibility    VARCHAR(15) NOT NULL DEFAULT 'publik'
                  CHECK (visibility IN ('publik', 'terbatas')),

    -- Statistik feed, di-denormalisasi agar daftar tidak perlu COUNT bersarang.
    support_count SMALLINT NOT NULL DEFAULT 0,
    comment_count SMALLINT NOT NULL DEFAULT 0,

    handled_by    UUID,
    handled_at    TIMESTAMPTZ,
    resolution    TEXT,
    resolved_at   TIMESTAMPTZ,

    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (created_at, id)
) PARTITION BY RANGE (created_at);

CREATE INDEX IF NOT EXISTS panic_reports_feed_idx
    ON panic_reports (created_at DESC)
    WHERE moderation_status = 'approved' AND visibility = 'publik';
CREATE INDEX IF NOT EXISTS panic_reports_school_idx   ON panic_reports (school_id, created_at DESC);
CREATE INDEX IF NOT EXISTS panic_reports_status_idx   ON panic_reports (status, created_at DESC);
CREATE INDEX IF NOT EXISTS panic_reports_author_idx   ON panic_reports (author_user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS panic_reports_severity_idx ON panic_reports (severity, created_at DESC)
    WHERE severity IN ('tinggi', 'darurat');

-- ------------------------------------------------------------------
-- Lampiran (foto bukti)
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS panic_report_media (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    report_id     UUID NOT NULL,
    report_created_at TIMESTAMPTZ NOT NULL,
    file_key      VARCHAR(300) NOT NULL,
    mime_type     VARCHAR(40) NOT NULL,
    bytes         INTEGER NOT NULL DEFAULT 0,
    -- Metadata EXIF dibuang oleh API sebelum disimpan: koordinat GPS pada
    -- foto akan membocorkan lokasi pelapor, yang mematahkan seluruh tujuan
    -- anonimitas.
    exif_stripped BOOLEAN NOT NULL DEFAULT TRUE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS panic_report_media_report_idx ON panic_report_media (report_id);

-- ------------------------------------------------------------------
-- Komentar
--
-- Komentar juga anonim, KECUALI bila ditulis petugas resmi — balasan resmi
-- justru harus terlihat resmi agar pelapor tahu laporannya ditangani.
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS panic_comments (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v7(),
    report_id     UUID NOT NULL,
    report_created_at TIMESTAMPTZ NOT NULL,

    author_user_id UUID NOT NULL,
    -- Handle stabil per (laporan, penulis): komentator yang sama terlihat
    -- konsisten dalam satu utas, tetapi tidak dapat dilacak antar-utas.
    anonymous_handle VARCHAR(24) NOT NULL,

    -- Bila TRUE, nama & jabatan penulis DITAMPILKAN. Hanya untuk petugas.
    is_official   BOOLEAN NOT NULL DEFAULT FALSE,
    official_name VARCHAR(150),
    official_title VARCHAR(100),

    body          TEXT NOT NULL,
    moderation_status VARCHAR(15) NOT NULL DEFAULT 'approved'
                  CHECK (moderation_status IN ('pending', 'approved', 'rejected')),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at    TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS panic_comments_report_idx
    ON panic_comments (report_id, created_at) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS panic_comments_author_idx ON panic_comments (author_user_id);

-- ------------------------------------------------------------------
-- Dukungan ("saya mengalami hal serupa")
--
-- Bukan "like". Pada konteks pengaduan, jumlah orang yang mengalami hal
-- sama adalah sinyal prioritas penanganan yang paling berguna.
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS panic_supports (
    report_id  UUID NOT NULL,
    user_id    UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (report_id, user_id)
);

-- ------------------------------------------------------------------
-- Riwayat penanganan — terlihat oleh pelapor sebagai lini masa.
--
-- Pelapor yang tidak pernah tahu laporannya diapakan akan berhenti
-- melapor. Lini masa ini adalah yang membuat platform tetap dipercaya.
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS panic_report_events (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v7(),
    report_id     UUID NOT NULL,
    report_created_at TIMESTAMPTZ NOT NULL,
    status        VARCHAR(20) NOT NULL,
    note          VARCHAR(500),
    actor_user_id UUID,
    actor_label   VARCHAR(150),
    is_public     BOOLEAN NOT NULL DEFAULT TRUE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS panic_report_events_report_idx
    ON panic_report_events (report_id, created_at);

-- ------------------------------------------------------------------
-- Jejak pembukaan identitas
--
-- Tabel ini adalah pengaman terakhir anonimitas. Tanpa catatan yang tidak
-- bisa dihapus, izin `unmask_panic_report` hanyalah janji.
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS panic_unmask_logs (
    id             UUID PRIMARY KEY DEFAULT uuid_generate_v7(),
    report_id      UUID NOT NULL,
    actor_user_id  UUID NOT NULL REFERENCES users (id) ON DELETE RESTRICT,
    actor_label    VARCHAR(150) NOT NULL,
    reason         VARCHAR(500) NOT NULL,
    ip_address     VARCHAR(45),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS panic_unmask_logs_report_idx ON panic_unmask_logs (report_id);
CREATE INDEX IF NOT EXISTS panic_unmask_logs_actor_idx  ON panic_unmask_logs (actor_user_id, created_at DESC);

-- ------------------------------------------------------------------
-- Partisi
-- ------------------------------------------------------------------
CREATE OR REPLACE FUNCTION ensure_panic_partitions(months_ahead INTEGER DEFAULT 3)
RETURNS VOID AS $$
DECLARE
    i INTEGER;
    d DATE;
BEGIN
    FOR i IN -1..months_ahead LOOP
        d := (date_trunc('month', CURRENT_DATE) + (i || ' month')::interval)::date;
        PERFORM ensure_monthly_partition('panic_reports', d);
    END LOOP;
END;
$$ LANGUAGE plpgsql;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'panic_reports_default') THEN
        CREATE TABLE panic_reports_default PARTITION OF panic_reports DEFAULT;
    END IF;
END $$;

SELECT ensure_panic_partitions(6);

-- ------------------------------------------------------------------
-- View feed: TIDAK memuat author_user_id.
--
-- Ini lapisan pertahanan tambahan, bukan pengganti pemeriksaan di aplikasi.
-- Query yang tidak sengaja memakai `SELECT *` pada view ini tetap aman.
-- ------------------------------------------------------------------
CREATE OR REPLACE VIEW panic_reports_feed AS
SELECT
    r.id,
    r.created_at,
    r.school_id,
    s.name           AS school_name,
    s.jenjang        AS school_jenjang,
    r.category_id,
    c.code           AS category_code,
    c.name           AS category_name,
    c.icon           AS category_icon,
    r.anonymous_handle,
    r.author_role,
    r.title,
    r.body,
    r.severity,
    r.status,
    r.moderation_status,
    r.visibility,
    r.support_count,
    r.comment_count,
    r.handled_at,
    r.resolved_at,
    r.updated_at
FROM panic_reports r
JOIN panic_categories c ON c.id = r.category_id
JOIN schools s          ON s.id = r.school_id;

-- ------------------------------------------------------------------
-- Penjaga hitungan denormalisasi
-- ------------------------------------------------------------------
CREATE OR REPLACE FUNCTION sync_panic_support_count() RETURNS trigger AS $$
DECLARE
    rid UUID;
    n   INTEGER;
BEGIN
    rid := COALESCE(NEW.report_id, OLD.report_id);
    SELECT COUNT(*) INTO n FROM panic_supports WHERE report_id = rid;
    UPDATE panic_reports SET support_count = LEAST(n, 32767) WHERE id = rid;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_panic_supports_count ON panic_supports;
CREATE TRIGGER trg_panic_supports_count
    AFTER INSERT OR DELETE ON panic_supports
    FOR EACH ROW EXECUTE FUNCTION sync_panic_support_count();

CREATE OR REPLACE FUNCTION sync_panic_comment_count() RETURNS trigger AS $$
DECLARE
    rid UUID;
    n   INTEGER;
BEGIN
    rid := COALESCE(NEW.report_id, OLD.report_id);
    SELECT COUNT(*) INTO n FROM panic_comments
     WHERE report_id = rid AND deleted_at IS NULL AND moderation_status = 'approved';
    UPDATE panic_reports SET comment_count = LEAST(n, 32767) WHERE id = rid;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_panic_comments_count ON panic_comments;
CREATE TRIGGER trg_panic_comments_count
    AFTER INSERT OR UPDATE OF deleted_at, moderation_status OR DELETE ON panic_comments
    FOR EACH ROW EXECUTE FUNCTION sync_panic_comment_count();

-- ------------------------------------------------------------------
-- Kategori bawaan
-- ------------------------------------------------------------------
INSERT INTO panic_categories (code, name, description, icon, default_severity, sort_order) VALUES
    ('perundungan', 'Perundungan / Bullying',
     'Kekerasan fisik, verbal, atau perundungan siber antar siswa.',
     'shield-alert', 'tinggi', 1),
    ('pungli', 'Pungutan Liar',
     'Permintaan uang di luar ketentuan resmi sekolah.',
     'cash-off', 'tinggi', 2),
    ('kekerasan', 'Kekerasan oleh Tenaga Pendidik',
     'Kekerasan fisik atau verbal yang dilakukan guru maupun pegawai sekolah.',
     'alert-octagon', 'darurat', 3),
    ('pelecehan', 'Pelecehan Seksual',
     'Segala bentuk pelecehan seksual di lingkungan sekolah.',
     'alert-triangle', 'darurat', 4),
    ('sarana', 'Sarana & Prasarana',
     'Fasilitas rusak atau tidak layak yang membahayakan siswa.',
     'tools', 'sedang', 5),
    ('pembelajaran', 'Proses Pembelajaran',
     'Guru sering tidak hadir, jam kosong, atau masalah kegiatan belajar.',
     'book', 'sedang', 6),
    ('narkoba', 'Narkoba & Zat Terlarang',
     'Peredaran atau penggunaan zat terlarang di lingkungan sekolah.',
     'pill', 'darurat', 7),
    ('lainnya', 'Lainnya',
     'Aduan lain yang berkaitan dengan sekolah.',
     'dots', 'rendah', 99)
ON CONFLICT (code) DO NOTHING;
