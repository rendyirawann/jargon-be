-- =====================================================================
-- 0010 : Seed — roles, permissions, superadmin, template notifikasi
-- Idempotent: aman dijalankan berulang.
-- =====================================================================

-- ------------------------------------------------------------------
-- Permissions (dikelompokkan lewat kolom `category` agar halaman
-- /admin/roles bisa menampilkannya per grup)
-- ------------------------------------------------------------------
INSERT INTO permissions (name, category, guard_name, created_at, updated_at) VALUES
    ('view_dashboard',              'Dashboard',      'web', NOW(), NOW()),

    ('view_school',                 'Sekolah',        'web', NOW(), NOW()),
    ('create_school',               'Sekolah',        'web', NOW(), NOW()),
    ('update_school',               'Sekolah',        'web', NOW(), NOW()),
    ('delete_school',               'Sekolah',        'web', NOW(), NOW()),

    ('view_classroom',              'Kelas',          'web', NOW(), NOW()),
    ('create_classroom',            'Kelas',          'web', NOW(), NOW()),
    ('update_classroom',            'Kelas',          'web', NOW(), NOW()),
    ('delete_classroom',            'Kelas',          'web', NOW(), NOW()),

    ('view_student',                'Siswa',          'web', NOW(), NOW()),
    ('create_student',              'Siswa',          'web', NOW(), NOW()),
    ('update_student',              'Siswa',          'web', NOW(), NOW()),
    ('delete_student',              'Siswa',          'web', NOW(), NOW()),
    ('import_student',              'Siswa',          'web', NOW(), NOW()),
    ('export_student',              'Siswa',          'web', NOW(), NOW()),
    ('manage_guardian',             'Siswa',          'web', NOW(), NOW()),

    ('view_face_enrollment',        'Biometrik',      'web', NOW(), NOW()),
    ('create_face_enrollment',      'Biometrik',      'web', NOW(), NOW()),
    ('approve_face_enrollment',     'Biometrik',      'web', NOW(), NOW()),
    ('delete_face_enrollment',      'Biometrik',      'web', NOW(), NOW()),

    ('view_attendance',             'Absensi',        'web', NOW(), NOW()),
    ('override_attendance',         'Absensi',        'web', NOW(), NOW()),
    ('export_attendance',           'Absensi',        'web', NOW(), NOW()),
    ('manage_attendance_rule',      'Absensi',        'web', NOW(), NOW()),

    ('view_device',                 'Perangkat',      'web', NOW(), NOW()),
    ('create_device',               'Perangkat',      'web', NOW(), NOW()),
    ('update_device',               'Perangkat',      'web', NOW(), NOW()),
    ('delete_device',               'Perangkat',      'web', NOW(), NOW()),
    ('pair_device',                 'Perangkat',      'web', NOW(), NOW()),

    ('view_notification',           'Notifikasi',     'web', NOW(), NOW()),
    ('send_notification',           'Notifikasi',     'web', NOW(), NOW()),
    ('manage_notification_template','Notifikasi',     'web', NOW(), NOW()),
    ('manage_notification_channel', 'Notifikasi',     'web', NOW(), NOW()),

    ('view_report',                 'Laporan',        'web', NOW(), NOW()),
    ('export_report',               'Laporan',        'web', NOW(), NOW()),

    ('view_resources',              'User Management','web', NOW(), NOW()),
    ('view_user',                   'User Management','web', NOW(), NOW()),
    ('create_user',                 'User Management','web', NOW(), NOW()),
    ('update_user',                 'User Management','web', NOW(), NOW()),
    ('delete_user',                 'User Management','web', NOW(), NOW()),
    ('ban_user',                    'User Management','web', NOW(), NOW()),
    ('view_role',                   'User Management','web', NOW(), NOW()),
    ('create_role',                 'User Management','web', NOW(), NOW()),
    ('update_role',                 'User Management','web', NOW(), NOW()),
    ('delete_role',                 'User Management','web', NOW(), NOW()),

    ('view_help',                   'Help',           'web', NOW(), NOW()),
    ('view_audit_log',              'Help',           'web', NOW(), NOW()),

    ('view_setting',                'Pengaturan',     'web', NOW(), NOW()),
    ('update_setting',              'Pengaturan',     'web', NOW(), NOW())
ON CONFLICT (name, guard_name) DO UPDATE SET category = EXCLUDED.category;

-- ------------------------------------------------------------------
-- Roles
-- ------------------------------------------------------------------
INSERT INTO roles (name, guard_name, created_at, updated_at) VALUES
    ('superadmin',      'web', NOW(), NOW()),
    ('admin_dinas',     'web', NOW(), NOW()),
    ('kepala_sekolah',  'web', NOW(), NOW()),
    ('guru',            'web', NOW(), NOW()),
    ('staff_tu',        'web', NOW(), NOW())
ON CONFLICT (name, guard_name) DO NOTHING;

-- superadmin : semua permission
INSERT INTO role_has_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r CROSS JOIN permissions p
WHERE r.name = 'superadmin' AND r.guard_name = 'web'
ON CONFLICT DO NOTHING;

-- admin_dinas : pantau seluruh provinsi, tanpa hak hapus & tanpa user mgmt
INSERT INTO role_has_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r JOIN permissions p ON p.name IN (
    'view_dashboard','view_school','update_school','view_classroom','view_student',
    'export_student','view_face_enrollment','view_attendance','export_attendance',
    'view_device','view_notification','view_report','export_report','view_help',
    'view_audit_log','view_setting'
) WHERE r.name = 'admin_dinas' AND r.guard_name = 'web'
ON CONFLICT DO NOTHING;

-- kepala_sekolah : pantau + setujui, terbatas pada sekolahnya
INSERT INTO role_has_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r JOIN permissions p ON p.name IN (
    'view_dashboard','view_school','view_classroom','view_student','manage_guardian',
    'view_face_enrollment','approve_face_enrollment','view_attendance',
    'override_attendance','export_attendance','manage_attendance_rule','view_device',
    'view_notification','send_notification','manage_notification_template',
    'view_report','export_report','view_setting'
) WHERE r.name = 'kepala_sekolah' AND r.guard_name = 'web'
ON CONFLICT DO NOTHING;

-- guru : monitoring siswa + daftarkan wajah + kirim notifikasi
INSERT INTO role_has_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r JOIN permissions p ON p.name IN (
    'view_dashboard','view_classroom','view_student','manage_guardian',
    'view_face_enrollment','create_face_enrollment','view_attendance',
    'override_attendance','view_notification','send_notification','view_report'
) WHERE r.name = 'guru' AND r.guard_name = 'web'
ON CONFLICT DO NOTHING;

-- staff_tu : operator data sekolah
INSERT INTO role_has_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r JOIN permissions p ON p.name IN (
    'view_dashboard','view_classroom','create_classroom','update_classroom','delete_classroom',
    'view_student','create_student','update_student','delete_student','import_student',
    'export_student','manage_guardian','view_face_enrollment','create_face_enrollment',
    'delete_face_enrollment','view_attendance','override_attendance','export_attendance',
    'manage_attendance_rule','view_device','create_device','update_device','pair_device',
    'view_notification','send_notification','view_report','export_report'
) WHERE r.name = 'staff_tu' AND r.guard_name = 'web'
ON CONFLICT DO NOTHING;

-- ------------------------------------------------------------------
-- Wilayah & tahun ajaran
-- ------------------------------------------------------------------
INSERT INTO regions (code, name, kind) VALUES
    ('12',   'Sumatera Utara',  'provinsi'),
    ('1275', 'Kota Medan',      'kota'),
    ('1271', 'Kota Sibolga',    'kota'),
    ('1272', 'Kota Tanjungbalai','kota'),
    ('1273', 'Kota Pematangsiantar','kota'),
    ('1274', 'Kota Tebing Tinggi','kota'),
    ('1276', 'Kota Binjai',     'kota'),
    ('1277', 'Kota Padangsidimpuan','kota'),
    ('1278', 'Kota Gunungsitoli','kota'),
    ('1201', 'Kabupaten Tapanuli Tengah','kabupaten'),
    ('1202', 'Kabupaten Tapanuli Utara','kabupaten'),
    ('1203', 'Kabupaten Tapanuli Selatan','kabupaten'),
    ('1204', 'Kabupaten Nias',  'kabupaten'),
    ('1205', 'Kabupaten Langkat','kabupaten'),
    ('1206', 'Kabupaten Karo',  'kabupaten'),
    ('1207', 'Kabupaten Deli Serdang','kabupaten'),
    ('1208', 'Kabupaten Simalungun','kabupaten'),
    ('1209', 'Kabupaten Asahan','kabupaten'),
    ('1210', 'Kabupaten Labuhanbatu','kabupaten'),
    ('1211', 'Kabupaten Dairi', 'kabupaten'),
    ('1212', 'Kabupaten Toba',  'kabupaten'),
    ('1213', 'Kabupaten Mandailing Natal','kabupaten'),
    ('1214', 'Kabupaten Nias Selatan','kabupaten'),
    ('1215', 'Kabupaten Pakpak Bharat','kabupaten'),
    ('1216', 'Kabupaten Humbang Hasundutan','kabupaten'),
    ('1217', 'Kabupaten Samosir','kabupaten'),
    ('1218', 'Kabupaten Serdang Bedagai','kabupaten'),
    ('1219', 'Kabupaten Batu Bara','kabupaten'),
    ('1220', 'Kabupaten Padang Lawas Utara','kabupaten'),
    ('1221', 'Kabupaten Padang Lawas','kabupaten'),
    ('1222', 'Kabupaten Labuhanbatu Selatan','kabupaten'),
    ('1223', 'Kabupaten Labuhanbatu Utara','kabupaten'),
    ('1224', 'Kabupaten Nias Utara','kabupaten'),
    ('1225', 'Kabupaten Nias Barat','kabupaten')
ON CONFLICT (code) DO NOTHING;

UPDATE regions child SET parent_id = prov.id
FROM regions prov
WHERE prov.code = '12' AND child.code <> '12' AND child.parent_id IS NULL;

INSERT INTO academic_years (name, start_date, end_date, is_active) VALUES
    ('2026/2027', '2026-07-14', '2027-06-20', TRUE)
ON CONFLICT (name) DO NOTHING;

INSERT INTO school_terms (academic_year_id, name, start_date, end_date, is_active)
SELECT ay.id, 'Ganjil', '2026-07-14', '2026-12-20', TRUE FROM academic_years ay WHERE ay.name = '2026/2027'
ON CONFLICT (academic_year_id, name) DO NOTHING;
INSERT INTO school_terms (academic_year_id, name, start_date, end_date, is_active)
SELECT ay.id, 'Genap', '2027-01-05', '2027-06-20', FALSE FROM academic_years ay WHERE ay.name = '2026/2027'
ON CONFLICT (academic_year_id, name) DO NOTHING;

-- ------------------------------------------------------------------
-- Superadmin
--   username : superadmin
--   password : Superadmin#2026   <-- WAJIB diganti setelah login pertama
--   Hash bcrypt cost 12, kompatibel dengan Hash::check() Laravel.
-- ------------------------------------------------------------------
INSERT INTO users (name, username, email, password, is_active, email_verified_at,
                   must_change_password, created_at, updated_at)
VALUES ('Super Administrator', 'superadmin', 'superadmin@disdik.sumutprov.go.id',
        '$2y$12$RllO.hebA59eQO9X5OjvAOcJ02/tqDIYNH8VxPskGLD1ahjfqz.4.',
        TRUE, NOW(), TRUE, NOW(), NOW())
ON CONFLICT (username) DO NOTHING;

INSERT INTO model_has_roles (role_id, model_type, model_id)
SELECT r.id, 'App\Models\User', u.id
FROM roles r, users u
WHERE r.name = 'superadmin' AND r.guard_name = 'web' AND u.username = 'superadmin'
ON CONFLICT DO NOTHING;

-- ------------------------------------------------------------------
-- Template notifikasi bawaan
--   Placeholder: {{nama_siswa}} {{kelas}} {{sekolah}} {{tanggal}}
--                {{jam_masuk}} {{jam_pulang}} {{status}} {{menit_terlambat}}
-- ------------------------------------------------------------------
INSERT INTO notification_templates (school_id, key, channel, subject, body) VALUES
(NULL, 'check_in', 'whatsapp', NULL,
 E'*Absensi Masuk*\n\nAnanda *{{nama_siswa}}* ({{kelas}}) telah tiba di {{sekolah}}.\n\n🕐 Jam masuk: *{{jam_masuk}}*\n📅 Tanggal: {{tanggal}}\n✅ Status: {{status}}\n\n_Pesan otomatis dari sistem absensi Dinas Pendidikan Provinsi Sumatera Utara._'),
(NULL, 'late', 'whatsapp', NULL,
 E'*Absensi Terlambat*\n\nAnanda *{{nama_siswa}}* ({{kelas}}) tiba di {{sekolah}} melewati jam masuk.\n\n🕐 Jam masuk: *{{jam_masuk}}*\n⏱️ Terlambat: *{{menit_terlambat}} menit*\n📅 Tanggal: {{tanggal}}\n\nMohon perhatian Bapak/Ibu untuk kedisiplinan ananda.'),
(NULL, 'absent', 'whatsapp', NULL,
 E'*Ananda Tidak Hadir*\n\nHingga batas jam absensi hari ini, ananda *{{nama_siswa}}* ({{kelas}}) belum tercatat hadir di {{sekolah}}.\n\n📅 Tanggal: {{tanggal}}\n\nBila ananda sakit/izin, mohon hubungi wali kelas.'),
(NULL, 'check_out', 'whatsapp', NULL,
 E'*Absensi Pulang*\n\nAnanda *{{nama_siswa}}* ({{kelas}}) telah pulang dari {{sekolah}}.\n\n🕐 Jam pulang: *{{jam_pulang}}*\n📅 Tanggal: {{tanggal}}'),

(NULL, 'check_in', 'telegram', NULL,
 E'<b>Absensi Masuk</b>\n\nAnanda <b>{{nama_siswa}}</b> ({{kelas}}) telah tiba di {{sekolah}}.\n\nJam masuk: <b>{{jam_masuk}}</b>\nTanggal: {{tanggal}}\nStatus: {{status}}'),
(NULL, 'late', 'telegram', NULL,
 E'<b>Absensi Terlambat</b>\n\nAnanda <b>{{nama_siswa}}</b> ({{kelas}}) terlambat {{menit_terlambat}} menit.\n\nJam masuk: <b>{{jam_masuk}}</b>\nTanggal: {{tanggal}}'),
(NULL, 'absent', 'telegram', NULL,
 E'<b>Ananda Tidak Hadir</b>\n\nAnanda <b>{{nama_siswa}}</b> ({{kelas}}) belum tercatat hadir di {{sekolah}} pada {{tanggal}}.'),
(NULL, 'check_out', 'telegram', NULL,
 E'<b>Absensi Pulang</b>\n\nAnanda <b>{{nama_siswa}}</b> ({{kelas}}) pulang pukul <b>{{jam_pulang}}</b> ({{tanggal}}).'),

(NULL, 'check_in', 'email', 'Absensi Masuk - {{nama_siswa}} ({{tanggal}})',
 E'Yang terhormat Bapak/Ibu wali dari {{nama_siswa}},\n\nAnanda {{nama_siswa}} kelas {{kelas}} telah tercatat hadir di {{sekolah}}.\n\nJam masuk : {{jam_masuk}}\nTanggal   : {{tanggal}}\nStatus    : {{status}}\n\nHormat kami,\nDinas Pendidikan Provinsi Sumatera Utara'),
(NULL, 'late', 'email', 'Absensi Terlambat - {{nama_siswa}} ({{tanggal}})',
 E'Yang terhormat Bapak/Ibu wali dari {{nama_siswa}},\n\nAnanda tercatat terlambat {{menit_terlambat}} menit pada {{tanggal}} (jam masuk {{jam_masuk}}).\n\nHormat kami,\nDinas Pendidikan Provinsi Sumatera Utara'),
(NULL, 'absent', 'email', 'Ketidakhadiran - {{nama_siswa}} ({{tanggal}})',
 E'Yang terhormat Bapak/Ibu wali dari {{nama_siswa}},\n\nAnanda belum tercatat hadir di {{sekolah}} pada {{tanggal}}.\n\nHormat kami,\nDinas Pendidikan Provinsi Sumatera Utara'),
(NULL, 'check_out', 'email', 'Absensi Pulang - {{nama_siswa}} ({{tanggal}})',
 E'Yang terhormat Bapak/Ibu wali dari {{nama_siswa}},\n\nAnanda pulang dari {{sekolah}} pukul {{jam_pulang}} pada {{tanggal}}.\n\nHormat kami,\nDinas Pendidikan Provinsi Sumatera Utara')
ON CONFLICT DO NOTHING;

-- ------------------------------------------------------------------
-- Tandai migration Laravel sebagai SUDAH dijalankan.
-- Skema dimiliki oleh sqlx; ini membuat `php artisan migrate` menjadi
-- no-op sehingga tidak ada dua sumber kebenaran yang saling menimpa.
-- ------------------------------------------------------------------
INSERT INTO migrations (migration, batch) VALUES
    ('0001_01_01_000000_create_users_table', 1),
    ('0001_01_01_000001_create_cache_table', 1),
    ('0001_01_01_000002_create_jobs_table', 1),
    ('2026_02_03_083715_create_permission_tables', 1),
    ('2026_03_03_062556_create_activity_log_table', 1),
    ('2026_03_10_130540_add_category_to_permissions_table', 1),
    ('2026_05_06_040000_create_settings_table', 1),
    ('2026_05_06_040001_add_social_columns_to_users_table', 1),
    -- migration yang di-load dari paket vendor
    ('2017_03_04_000000_create_bans_table', 1);

-- ------------------------------------------------------------------
-- Setting dashboard
-- ------------------------------------------------------------------
INSERT INTO settings (key, value, created_at, updated_at) VALUES
    ('site_name', 'Absensi Face Recognition - Disdik Sumut', NOW(), NOW()),
    ('site_logo', 'base-logo.png', NOW(), NOW()),
    ('site_font', 'Plus Jakarta Sans', NOW(), NOW()),
    ('social_google_enabled',   '0', NOW(), NOW()),
    ('social_facebook_enabled', '0', NOW(), NOW()),
    ('social_github_enabled',   '0', NOW(), NOW()),
    ('social_linkedin_enabled', '0', NOW(), NOW())
ON CONFLICT (key) DO NOTHING;
