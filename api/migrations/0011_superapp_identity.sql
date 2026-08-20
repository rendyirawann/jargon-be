-- =====================================================================
-- 0011 : Identitas Super App "Jargon GO"
--
-- PERUBAHAN MENDASAR DARI 0010
--   Sebelumnya siswa tidak punya akun sama sekali. Pada Jargon GO, siswa
--   dan orang tua ikut memakai aplikasi (memantau absensi, mengirim
--   pengaduan), sehingga keduanya butuh akun.
--
--   Ini TIDAK mengubah cara absensi bekerja: tablet tetap mengenali siswa
--   lewat wajah tanpa login. Akun siswa hanya dipakai untuk MEMBACA data
--   dirinya sendiri di aplikasi.
--
-- IDENTITAS LOGIN
--   Pendaftaran akun dilakukan admin, bukan swalayan. Yang dipakai login:
--     * NISN (10 digit) untuk siswa
--     * NIK  (16 digit) untuk guru, staff, kepala sekolah, orang tua, dinas
--
--   Satu kolom `identity_number` menampung keduanya, dengan
--   `identity_type` sebagai penandanya. Alasan memakai satu kolom:
--   layar login hanya punya satu kotak isian, dan pencarian akun harus
--   satu index tunggal — bukan dua kolom yang harus di-OR.
-- =====================================================================

ALTER TABLE users ADD COLUMN IF NOT EXISTS identity_number VARCHAR(16);
ALTER TABLE users ADD COLUMN IF NOT EXISTS identity_type   VARCHAR(10)
    CHECK (identity_type IS NULL OR identity_type IN ('nik', 'nisn'));

-- Untuk akun siswa: menunjuk ke data siswanya. Akun ikut terhapus bila
-- data siswanya dihapus.
ALTER TABLE users ADD COLUMN IF NOT EXISTS student_id UUID
    REFERENCES students (id) ON DELETE CASCADE;

-- Nomor HP untuk pemulihan akun & notifikasi aplikasi.
ALTER TABLE users ADD COLUMN IF NOT EXISTS phone_verified_at TIMESTAMPTZ;

-- Satu identitas = satu akun. Partial index agar akun lama tanpa
-- identity_number (mis. superadmin) tetap valid.
CREATE UNIQUE INDEX IF NOT EXISTS users_identity_unique
    ON users (identity_number)
    WHERE identity_number IS NOT NULL AND deleted_at IS NULL;

-- Satu siswa hanya boleh punya satu akun.
CREATE UNIQUE INDEX IF NOT EXISTS users_student_unique
    ON users (student_id)
    WHERE student_id IS NOT NULL AND deleted_at IS NULL;

-- Panjang identitas harus sesuai jenisnya. Diperiksa di database, bukan
-- hanya di aplikasi: data pokok kependudukan yang salah panjang akan
-- menyulitkan integrasi dengan sistem Provsu lain nanti.
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_identity_length_check;
ALTER TABLE users ADD CONSTRAINT users_identity_length_check CHECK (
    identity_number IS NULL
    OR (identity_type = 'nik'  AND identity_number ~ '^[0-9]{16}$')
    OR (identity_type = 'nisn' AND identity_number ~ '^[0-9]{10}$')
);

-- ------------------------------------------------------------------
-- Tautan akun orang tua -> siswa
--
-- Seorang wali bisa punya beberapa anak, dan anak-anak itu bisa berada di
-- sekolah yang berbeda. Karena itu cakupan akun orang tua TIDAK ditentukan
-- oleh users.school_id, melainkan diturunkan dari baris-baris di sini.
-- ------------------------------------------------------------------
ALTER TABLE student_guardians ADD COLUMN IF NOT EXISTS user_id UUID
    REFERENCES users (id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS student_guardians_user_idx
    ON student_guardians (user_id) WHERE user_id IS NOT NULL;

-- ------------------------------------------------------------------
-- Perangkat aplikasi (untuk notifikasi push)
--
-- Berbeda dari tabel `devices` yang merupakan tablet kios: ini ponsel
-- pribadi pengguna, terikat pada akun dan bisa lebih dari satu.
-- ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS user_devices (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    push_token    VARCHAR(255),
    platform      VARCHAR(10) NOT NULL DEFAULT 'android'
                  CHECK (platform IN ('android', 'ios', 'web')),
    app_version   VARCHAR(30),
    os_version    VARCHAR(60),
    device_model  VARCHAR(120),
    last_seen_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS user_devices_user_idx ON user_devices (user_id);
CREATE UNIQUE INDEX IF NOT EXISTS user_devices_push_unique
    ON user_devices (push_token) WHERE push_token IS NOT NULL;

-- ------------------------------------------------------------------
-- Peran baru
-- ------------------------------------------------------------------
INSERT INTO roles (name, guard_name, created_at, updated_at) VALUES
    ('siswa',     'web', NOW(), NOW()),
    ('orang_tua', 'web', NOW(), NOW()),
    -- Petugas Dinas yang menangani pengaduan Panic Button.
    ('petugas_pengaduan', 'web', NOW(), NOW())
ON CONFLICT (name, guard_name) DO NOTHING;

-- ------------------------------------------------------------------
-- Izin baru
-- ------------------------------------------------------------------
INSERT INTO permissions (name, category, guard_name, created_at, updated_at) VALUES
    -- Aplikasi mobile
    ('use_mobile_app',            'Aplikasi',   'web', NOW(), NOW()),
    ('view_own_attendance',       'Aplikasi',   'web', NOW(), NOW()),
    ('view_children_attendance',  'Aplikasi',   'web', NOW(), NOW()),

    -- Panic Button
    ('view_panic_feed',           'Pengaduan',  'web', NOW(), NOW()),
    ('create_panic_report',       'Pengaduan',  'web', NOW(), NOW()),
    ('comment_panic_report',      'Pengaduan',  'web', NOW(), NOW()),
    ('moderate_panic_report',     'Pengaduan',  'web', NOW(), NOW()),
    ('handle_panic_report',       'Pengaduan',  'web', NOW(), NOW()),
    -- Membuka identitas pelapor. Sengaja dipisah dan hanya untuk Superadmin;
    -- setiap pemakaiannya dicatat di panic_unmask_logs.
    ('unmask_panic_report',       'Pengaduan',  'web', NOW(), NOW()),

    -- Pemberkasan
    ('view_document_submission',  'Pemberkasan','web', NOW(), NOW()),
    ('create_document_submission','Pemberkasan','web', NOW(), NOW()),
    ('verify_document_submission','Pemberkasan','web', NOW(), NOW()),
    ('manage_document_type',      'Pemberkasan','web', NOW(), NOW()),

    -- Manajemen akun aplikasi
    ('manage_app_account',        'User Management', 'web', NOW(), NOW())
ON CONFLICT (name, guard_name) DO UPDATE SET category = EXCLUDED.category;

-- ------------------------------------------------------------------
-- Pemberian izin per peran
-- ------------------------------------------------------------------

-- superadmin: semua izin (termasuk yang baru).
INSERT INTO role_has_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r CROSS JOIN permissions p
WHERE r.name = 'superadmin' AND r.guard_name = 'web'
ON CONFLICT DO NOTHING;

-- siswa: lihat absensi sendiri + ikut Panic Button.
INSERT INTO role_has_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r JOIN permissions p ON p.name IN (
    'use_mobile_app', 'view_own_attendance',
    'view_panic_feed', 'create_panic_report', 'comment_panic_report'
) WHERE r.name = 'siswa' AND r.guard_name = 'web'
ON CONFLICT DO NOTHING;

-- orang_tua: lihat absensi anaknya + ikut Panic Button.
INSERT INTO role_has_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r JOIN permissions p ON p.name IN (
    'use_mobile_app', 'view_children_attendance',
    'view_panic_feed', 'create_panic_report', 'comment_panic_report'
) WHERE r.name = 'orang_tua' AND r.guard_name = 'web'
ON CONFLICT DO NOTHING;

-- guru: tambahan Panic Button (baca saja) + pemberkasan miliknya sendiri.
INSERT INTO role_has_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r JOIN permissions p ON p.name IN (
    'use_mobile_app', 'view_panic_feed',
    'view_document_submission', 'create_document_submission'
) WHERE r.name = 'guru' AND r.guard_name = 'web'
ON CONFLICT DO NOTHING;

INSERT INTO role_has_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r JOIN permissions p ON p.name IN (
    'use_mobile_app', 'view_panic_feed',
    'view_document_submission', 'create_document_submission',
    'manage_app_account'
) WHERE r.name = 'staff_tu' AND r.guard_name = 'web'
ON CONFLICT DO NOTHING;

-- kepala_sekolah: melihat pengaduan yang menyangkut sekolahnya (tanpa
-- identitas pelapor) dan memverifikasi pemberkasan gurunya.
INSERT INTO role_has_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r JOIN permissions p ON p.name IN (
    'use_mobile_app', 'view_panic_feed', 'handle_panic_report',
    'view_document_submission', 'create_document_submission',
    'verify_document_submission', 'manage_app_account'
) WHERE r.name = 'kepala_sekolah' AND r.guard_name = 'web'
ON CONFLICT DO NOTHING;

-- admin_dinas: pantau seluruh provinsi.
INSERT INTO role_has_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r JOIN permissions p ON p.name IN (
    'use_mobile_app', 'view_panic_feed', 'handle_panic_report',
    'view_document_submission', 'verify_document_submission'
) WHERE r.name = 'admin_dinas' AND r.guard_name = 'web'
ON CONFLICT DO NOTHING;

-- petugas_pengaduan: peran khusus penanganan Panic Button di tingkat Dinas.
INSERT INTO role_has_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r JOIN permissions p ON p.name IN (
    'use_mobile_app', 'view_dashboard',
    'view_panic_feed', 'moderate_panic_report', 'handle_panic_report',
    'comment_panic_report', 'view_school'
) WHERE r.name = 'petugas_pengaduan' AND r.guard_name = 'web'
ON CONFLICT DO NOTHING;

-- ------------------------------------------------------------------
-- Superadmin bawaan memakai NIK sebagai identitas login.
-- Nilai contoh; ganti dengan NIK sebenarnya lewat /admin/users.
-- ------------------------------------------------------------------
UPDATE users
   SET identity_number = COALESCE(identity_number, '1275000000000001'),
       identity_type   = COALESCE(identity_type, 'nik')
 WHERE username = 'superadmin'
   AND identity_number IS NULL;

-- ------------------------------------------------------------------
-- Pengaturan aplikasi Jargon GO
-- ------------------------------------------------------------------
INSERT INTO settings (key, value, created_at, updated_at) VALUES
    ('app_name',        'Jargon GO', NOW(), NOW()),
    ('app_tagline',     'Super Apps Dinas Pendidikan Provinsi Sumatera Utara', NOW(), NOW()),
    ('panic_pre_moderation', '1', NOW(), NOW()),
    ('panic_urgent_notify_roles', 'petugas_pengaduan,admin_dinas,superadmin', NOW(), NOW())
ON CONFLICT (key) DO NOTHING;
