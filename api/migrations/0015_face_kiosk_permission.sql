-- =====================================================================
-- 0015 : Izin mengoperasikan stasiun absensi wajah
--
-- MENGAPA IZIN TERSENDIRI
--
-- Halaman /admin/biometric/scan melakukan hal yang berbeda dari dua izin
-- yang sudah ada:
--
--   create_face_enrollment  -> MENAMBAH data biometrik siswa
--   override_attendance     -> MENGUBAH absensi yang sudah tercatat
--   operate_face_kiosk      -> MENCATAT kehadiran lewat pemindaian wajah
--
-- Memakai salah satu izin lama akan menggabungkan kewenangan yang
-- sebaiknya dapat diberikan terpisah: guru boleh mendaftarkan wajah di
-- kelasnya tanpa harus boleh menjalankan gerbang absensi, dan operator
-- piket boleh menjalankan gerbang tanpa boleh menyentuh data biometrik.
--
-- Diberikan kepada: superadmin, admin_dinas, kepala_sekolah, staff_tu.
-- TIDAK kepada guru — menjalankan gerbang bukan tugas mengajar, dan
-- membatasi siapa yang boleh mencatat kehadiran mempersempit peluang
-- absensi titipan.
-- =====================================================================

INSERT INTO permissions (name, category, guard_name, created_at, updated_at)
VALUES ('operate_face_kiosk', 'Biometrik', 'web', NOW(), NOW())
ON CONFLICT (name, guard_name) DO UPDATE SET category = EXCLUDED.category;

INSERT INTO role_has_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p ON p.name = 'operate_face_kiosk' AND p.guard_name = 'web'
WHERE r.guard_name = 'web'
  AND r.name IN ('superadmin', 'admin_dinas', 'kepala_sekolah', 'staff_tu')
ON CONFLICT DO NOTHING;
