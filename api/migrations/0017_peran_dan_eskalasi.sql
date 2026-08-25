-- =====================================================================
-- 0017 : Delapan peran, cakupan Cabang Dinas, dan eskalasi Panic Button
--
-- Tiga hal yang ditambahkan, dan alasan bentuknya seperti ini.
--
-- 1. PERAN
--
-- Peran akhir: Superadmin, DISDIK, CAPDIS, Sekolah, Admin, Guru, Siswa,
-- Orang Tua. Yang sudah ada dipakai apa adanya, TIDAK diganti nama:
--
--   DISDIK      -> `admin_dinas`   (sudah ada sejak 0010)
--   Guru        -> `guru`          (sudah ada sejak 0010)
--   Siswa       -> `siswa`         (sudah ada sejak 0011)
--   Orang Tua   -> `orang_tua`     (sudah ada sejak 0011)
--   Superadmin  -> `superadmin`    (sudah ada sejak 0010)
--
-- Mengganti nama `admin_dinas` menjadi `disdik` akan memutus hibah izin
-- di 0010, setelan `panic_urgent_notify_roles` di 0011, dan setiap
-- pemeriksaan peran di kode Rust maupun Laravel. Nama tampilannya diatur
-- di antarmuka; nama teknisnya dibiarkan stabil.
--
-- Yang benar-benar baru: `capdis`, `sekolah`, `admin`.
--
-- 2. CAKUPAN CABANG DINAS
--
-- Satu Cabang Dinas mencakup BEBERAPA kabupaten/kota. Karena `regions`
-- sudah hierarkis (`parent_id`), cabang dinas dipasang sebagai satu
-- tingkat di antaranya:
--
--   provinsi -> cabang_dinas -> kabupaten/kota -> schools
--
-- Dipilih begitu, bukan tabel pemetaan sekolah-per-sekolah, karena
-- sekolah baru otomatis masuk cakupan capdis yang benar begitu
-- `region_id`-nya diisi. Pemetaan manual akan selalu tertinggal setiap
-- ada sekolah baru, dan yang tertinggal di sini berarti laporan siswa
-- tidak terlihat oleh siapa pun.
--
-- 3. ESKALASI
--
-- DUA tenggat berjalan bersamaan, sesuai keputusan:
--
--   process_deadline_at  = dibuat + 7 hari    -> sekolah harus MULAI
--   resolve_deadline_at  = diproses + 7 hari  -> harus SELESAI
--
-- Tenggat pertama itu yang penting: tanpa dia, sekolah dapat mengubur
-- laporan hanya dengan tidak pernah menyentuhnya — jam eskalasi tidak
-- akan pernah mulai berjalan. Diam bukan alasan untuk berhenti.
-- =====================================================================

-- ------------------------------------------------------------------
-- 1. Cabang Dinas sebagai tingkat wilayah
-- ------------------------------------------------------------------
ALTER TABLE regions DROP CONSTRAINT IF EXISTS regions_kind_check;
ALTER TABLE regions ADD CONSTRAINT regions_kind_check
    CHECK (kind IN ('provinsi', 'cabang_dinas', 'kabupaten', 'kota'));

-- Cakupan wilayah seorang pengguna. Dipakai peran `capdis`.
--
-- Kolom sendiri, bukan baris-baris di `user_school_scopes`: seorang
-- petugas capdis membawahi puluhan sekolah, dan menuliskannya satu per
-- satu berarti daftar itu harus diperbarui setiap kali ada sekolah baru.
ALTER TABLE users ADD COLUMN IF NOT EXISTS region_id UUID
    REFERENCES regions (id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS users_region_idx
    ON users (region_id) WHERE region_id IS NOT NULL AND deleted_at IS NULL;

-- ------------------------------------------------------------------
-- 2. Peran baru
-- ------------------------------------------------------------------
INSERT INTO roles (name, guard_name, created_at, updated_at) VALUES
    -- Petugas Cabang Dinas: memantau sekolah di wilayahnya saja.
    ('capdis',  'web', NOW(), NOW()),
    -- Akun milik SEKOLAH, masuk memakai NPSN. Bukan akun perorangan.
    ('sekolah', 'web', NOW(), NOW()),
    -- Operator yang ditugaskan pada SATU sekolah dan mengelola datanya.
    ('admin',   'web', NOW(), NOW())
ON CONFLICT (name, guard_name) DO NOTHING;

-- ------------------------------------------------------------------
-- 3. Izin baru
-- ------------------------------------------------------------------
INSERT INTO permissions (name, category, guard_name, created_at, updated_at) VALUES
    -- Gerbang login dashboard web. Siswa, orang tua, dan guru TIDAK
    -- memilikinya, sehingga akun mereka tidak dapat masuk ke web sama
    -- sekali — dijaga oleh izin, bukan oleh tidak adanya tombol.
    ('login_dashboard',      'Akses',        'web', NOW(), NOW()),
    -- Melihat laporan Panic Button sesuai cakupan peran.
    ('view_panic_scope',     'Panic Button', 'web', NOW(), NOW()),
    -- Memproses laporan pada tingkat masing-masing.
    ('handle_panic_sekolah', 'Panic Button', 'web', NOW(), NOW()),
    ('handle_panic_capdis',  'Panic Button', 'web', NOW(), NOW()),
    ('handle_panic_disdik',  'Panic Button', 'web', NOW(), NOW()),
    -- Menyatakan sebuah laporan SELESAI. Hanya Disdik, sesuai keputusan.
    ('resolve_panic',        'Panic Button', 'web', NOW(), NOW()),
    -- Membalas laporan siswa (catatan + lampiran opsional).
    ('reply_panic',          'Panic Button', 'web', NOW(), NOW())
ON CONFLICT (name, guard_name) DO NOTHING;

-- superadmin: seluruh izin, termasuk yang baru di atas.
INSERT INTO role_has_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r CROSS JOIN permissions p
WHERE r.name = 'superadmin' AND r.guard_name = 'web'
ON CONFLICT DO NOTHING;

-- Yang boleh masuk dashboard web: capdis, disdik, sekolah, admin,
-- superadmin. Guru, siswa, dan orang tua sengaja TIDAK termasuk.
INSERT INTO role_has_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r JOIN permissions p ON p.name = 'login_dashboard'
WHERE r.guard_name = 'web'
  AND r.name IN ('capdis', 'admin_dinas', 'sekolah', 'admin', 'superadmin')
ON CONFLICT DO NOTHING;

-- capdis: memantau wilayahnya, menangani laporan yang naik kepadanya.
INSERT INTO role_has_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r JOIN permissions p ON p.name IN (
    'view_dashboard', 'view_school', 'view_classroom', 'view_student',
    'view_attendance', 'export_attendance', 'view_report', 'export_report',
    'view_panic_scope', 'handle_panic_capdis', 'reply_panic'
) WHERE r.name = 'capdis' AND r.guard_name = 'web'
ON CONFLICT DO NOTHING;

-- sekolah: seluruh muridnya, dan laporan dari muridnya.
INSERT INTO role_has_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r JOIN permissions p ON p.name IN (
    'view_dashboard', 'view_school', 'view_classroom', 'view_student',
    'view_attendance', 'export_attendance', 'view_report',
    'view_panic_scope', 'handle_panic_sekolah', 'reply_panic'
) WHERE r.name = 'sekolah' AND r.guard_name = 'web'
ON CONFLICT DO NOTHING;

-- admin: mengelola data SATU sekolah. Diberi hak ubah, bukan hanya baca.
INSERT INTO role_has_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r JOIN permissions p ON p.name IN (
    'view_dashboard', 'view_school', 'update_school',
    'view_classroom', 'create_classroom', 'update_classroom',
    'view_student', 'create_student', 'update_student', 'export_student',
    'view_face_enrollment', 'create_face_enrollment', 'approve_face_enrollment',
    'operate_face_kiosk', 'view_attendance', 'override_attendance',
    'export_attendance', 'manage_attendance_rule', 'view_device',
    'view_notification', 'view_report', 'export_report',
    'view_panic_scope', 'handle_panic_sekolah', 'reply_panic'
) WHERE r.name = 'admin' AND r.guard_name = 'web'
ON CONFLICT DO NOTHING;

-- DISDIK: satu-satunya yang boleh menyatakan laporan SELESAI.
INSERT INTO role_has_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r JOIN permissions p ON p.name IN (
    'view_panic_scope', 'handle_panic_disdik', 'resolve_panic', 'reply_panic'
) WHERE r.name = 'admin_dinas' AND r.guard_name = 'web'
ON CONFLICT DO NOTHING;

-- ------------------------------------------------------------------
-- 4. Eskalasi pada panic_reports
--
-- `escalation_level` menyatakan SIAPA yang sedang bertanggung jawab,
-- terpisah dari `status` yang menyatakan SEJAUH MANA laporan berjalan.
-- Dua hal itu memang berbeda: laporan bisa berstatus 'ditindaklanjuti'
-- di tingkat capdis setelah sekolah membiarkannya lewat tenggat.
-- ------------------------------------------------------------------
ALTER TABLE panic_reports
    ADD COLUMN IF NOT EXISTS escalation_level VARCHAR(10) NOT NULL DEFAULT 'sekolah',
    ADD COLUMN IF NOT EXISTS process_deadline_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS resolve_deadline_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS escalated_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS escalation_note VARCHAR(200);

ALTER TABLE panic_reports DROP CONSTRAINT IF EXISTS panic_reports_escalation_check;
ALTER TABLE panic_reports ADD CONSTRAINT panic_reports_escalation_check
    CHECK (escalation_level IN ('sekolah', 'capdis', 'disdik'));

-- Tenggat pertama untuk laporan yang sudah ada sebelum migrasi ini.
-- Tanpa backfill, laporan lama tidak akan pernah dilihat pekerja
-- eskalasi karena tenggatnya NULL.
UPDATE panic_reports
   SET process_deadline_at = created_at + INTERVAL '7 days'
 WHERE process_deadline_at IS NULL;

-- Index yang dipakai pekerja eskalasi: cari laporan yang tenggatnya
-- sudah lewat dan belum naik tingkat. Partial supaya tetap kecil —
-- laporan selesai tidak pernah dicari lagi.
CREATE INDEX IF NOT EXISTS panic_reports_eskalasi_idx
    ON panic_reports (escalation_level, process_deadline_at)
    WHERE status NOT IN ('selesai', 'ditolak');

CREATE INDEX IF NOT EXISTS panic_reports_eskalasi_selesai_idx
    ON panic_reports (escalation_level, resolve_deadline_at)
    WHERE resolve_deadline_at IS NOT NULL AND status NOT IN ('selesai', 'ditolak');

-- ------------------------------------------------------------------
-- 5. Satu laporan aktif per siswa
--
-- MENGAPA TABEL TERPISAH, BUKAN UNIQUE INDEX
--
-- `panic_reports` dipartisi RANGE per `created_at`. PostgreSQL menuntut
-- setiap unique constraint pada tabel terpartisi MENYERTAKAN kolom
-- partisinya — sehingga `UNIQUE (author_user_id)` mustahil dibuat di
-- sana: yang bisa dibuat hanyalah `UNIQUE (author_user_id, created_at)`,
-- dan itu justru mengizinkan banyak laporan asalkan waktunya berbeda,
-- yaitu tepat yang ingin dicegah.
--
-- Tabel kecil ini memegang satu baris per siswa yang punya laporan
-- terbuka. Primary key pada author_user_id membuat aturannya dijaga
-- BASIS DATA, bukan dijaga niat baik kode pemanggil — dua permintaan
-- yang tiba bersamaan tidak bisa dua-duanya lolos.
--
-- Barisnya dihapus ketika laporan selesai atau ditolak, dan sejak itu
-- siswa boleh melapor lagi. Berlaku untuk kategori apa pun: satu siswa,
-- satu laporan terbuka.
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS panic_active_reports (
    author_user_id    UUID PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    report_id         UUID NOT NULL,
    report_created_at TIMESTAMPTZ NOT NULL,
    school_id         UUID NOT NULL REFERENCES schools (id) ON DELETE CASCADE,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS panic_active_reports_school_idx
    ON panic_active_reports (school_id);

-- Isi dari laporan terbuka yang sudah ada. DISTINCT ON menjaga satu
-- baris per siswa bila ternyata ada yang sudah punya lebih dari satu —
-- yang terbaru dianggap yang berlaku.
--
-- Tanpa filter `deleted_at`: `panic_reports` memang tidak punya kolom
-- itu. Penghapusan lunak ada pada `panic_comments`, bukan pada
-- laporannya sendiri — laporan hanya berpindah status.
INSERT INTO panic_active_reports (author_user_id, report_id, report_created_at, school_id)
SELECT DISTINCT ON (author_user_id)
       author_user_id, id, created_at, school_id
  FROM panic_reports
 WHERE status NOT IN ('selesai', 'ditolak')
 ORDER BY author_user_id, created_at DESC
ON CONFLICT (author_user_id) DO NOTHING;

-- ------------------------------------------------------------------
-- 6. Kategori sesuai spesifikasi
--
-- Empat kategori yang diminta ditambahkan. Kategori lama TIDAK dihapus,
-- dan satu di antaranya sengaja dipertahankan: 'pelecehan' (pelecehan
-- seksual). Menghapusnya berarti laporan pelecehan harus dimasukkan ke
-- kategori lain yang lebih ringan, dan tingkat kedaruratannya ikut
-- turun. Itu keputusan yang seharusnya diambil sadar, bukan sebagai
-- efek samping penyesuaian daftar.
-- ------------------------------------------------------------------
UPDATE panic_categories
   SET name = 'Bullying / Perundungan'
 WHERE code = 'perundungan';

INSERT INTO panic_categories (code, name, description, icon, default_severity, sort_order) VALUES
    ('pelanggaran_sekolah', 'Pelanggaran Sekolah',
     'Pelanggaran aturan atau kebijakan oleh pihak sekolah, termasuk pungutan di luar ketentuan.',
     'building-off', 'tinggi', 5),
    ('pelanggaran_guru', 'Pelanggaran Guru',
     'Pelanggaran yang dilakukan guru atau pegawai sekolah.',
     'user-x', 'tinggi', 6),
    ('pelanggaran_siswa', 'Pelanggaran Siswa',
     'Pelanggaran tata tertib oleh siswa lain.',
     'users-x', 'sedang', 7)
ON CONFLICT (code) DO NOTHING;

-- ------------------------------------------------------------------
-- 7. Setelan tenggat, agar tidak dipaku di kode
-- ------------------------------------------------------------------
INSERT INTO settings (key, value, created_at, updated_at) VALUES
    ('panic_process_deadline_hours', '168', NOW(), NOW()),
    ('panic_resolve_deadline_hours', '168', NOW(), NOW()),
    -- Timeline siswa dibatasi sekolahnya sendiri. Siswa dari sekolah
    -- lain tidak pernah melihat laporan ini.
    ('panic_feed_scope', 'sekolah', NOW(), NOW())
ON CONFLICT (key) DO NOTHING;

ANALYZE panic_reports;
ANALYZE panic_active_reports;
