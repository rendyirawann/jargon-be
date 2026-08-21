-- =====================================================================
-- Data demo untuk pengujian.
--
-- BUKAN migrasi, dan sengaja TIDAK ditaruh di migrations/. Migrasi
-- dijalankan otomatis di setiap pemasangan termasuk produksi; data ini
-- hanya untuk mencoba aplikasi. Sekolah "SMA Negeri 1 Medan (DEMO)" yang
-- muncul di lingkungan Dinas sebenarnya akan lebih merepotkan daripada
-- membantu.
--
-- Aman dijalankan berulang kali: setiap INSERT memakai ON CONFLICT, dan
-- pengenal barisnya tetap (NPSN, NISN, NIK), bukan UUID acak.
--
-- Menjalankan:
--   docker compose exec -T postgres psql -U absensi -d absensi < jargon-be/api/seeds/demo.sql
-- atau lewat skrip pembungkusnya:
--   scripts\seed-demo.bat
--
-- Kata sandi seluruh akun demo: Demo#2026
-- Hash dibuat pgcrypto dengan bcrypt cost 12 — format yang sama dengan
-- yang dipakai API (Rust) maupun dashboard (Laravel), sehingga satu akun
-- bisa masuk ke keduanya.
-- =====================================================================

BEGIN;

-- ------------------------------------------------------------------
-- Sekolah
-- ------------------------------------------------------------------
INSERT INTO schools (npsn, name, slug, jenjang, status, region_id, address, district, principal_name, phone, email)
SELECT '10259001', 'SMA Negeri 1 Medan (DEMO)', 'sman-1-medan-demo', 'SMA', 'negeri',
       r.id, 'Jl. Teuku Cik Ditiro No. 1', 'Medan Baru', 'Dra. Siti Aminah, M.Pd',
       '0614512345', 'sman1medan.demo@disdik.sumutprov.go.id'
FROM regions r WHERE r.name ILIKE '%Medan%' LIMIT 1
ON CONFLICT (npsn) DO NOTHING;

-- ------------------------------------------------------------------
-- Kelas
-- ------------------------------------------------------------------
INSERT INTO classrooms (school_id, academic_year_id, name, grade_level, major)
SELECT s.id, ay.id, k.name, k.grade, 'IPA'
FROM schools s
CROSS JOIN (SELECT id FROM academic_years WHERE is_active ORDER BY name DESC LIMIT 1) ay
CROSS JOIN (VALUES ('X IPA 1', 10), ('XI IPA 1', 11)) AS k(name, grade)
WHERE s.npsn = '10259001'
ON CONFLICT (school_id, academic_year_id, name) DO NOTHING;

-- ------------------------------------------------------------------
-- Siswa
--
-- NISN 10 digit — panjangnya ditegakkan CHECK di tabel users, jadi NISN
-- yang salah panjang membuat akun siswanya tidak bisa dibuat.
-- ------------------------------------------------------------------
INSERT INTO students (school_id, current_classroom_id, nisn, nis, full_name, gender,
                      birth_place, birth_date, religion, status, entry_year,
                      father_name, mother_name)
SELECT s.id, c.id, d.nisn, d.nis, d.nama, d.jk,
       'Medan', d.lahir::date, 'Islam', 'aktif', 2026, d.ayah, d.ibu
FROM schools s
-- Alias `d` harus diperkenalkan SEBELUM dirujuk pada kondisi JOIN
-- classrooms; urutan terbalik menghasilkan "missing FROM-clause entry".
CROSS JOIN (VALUES
    ('0071234501', '2026001', 'Ahmad Fauzi Nasution',  'L', '2009-03-14', 'X IPA 1',  'Rudi Nasution',   'Siti Halimah'),
    ('0071234502', '2026002', 'Siti Nurhaliza Lubis',  'P', '2009-07-02', 'X IPA 1',  'Amir Lubis',      'Dewi Sartika'),
    ('0071234503', '2026003', 'Budi Santoso Siregar',  'L', '2009-11-25', 'X IPA 1',  'Joko Siregar',    'Ratna Sari'),
    ('0071234504', '2026004', 'Putri Ayu Harahap',     'P', '2008-05-09', 'XI IPA 1', 'Iwan Harahap',    'Nur Aisyah'),
    ('0071234505', '2026005', 'Rizky Pratama Ginting', 'L', '2008-09-18', 'XI IPA 1', 'Bakti Ginting',   'Lestari Br Karo')
) AS d(nisn, nis, nama, jk, lahir, kelas, ayah, ibu)
JOIN classrooms c ON c.school_id = s.id AND c.name = d.kelas
WHERE s.npsn = '10259001'
-- Tanpa target: menangani konflik pada index unik partial students_nisn_unique
-- maupun students_school_nis_unique, sehingga seed aman dijalankan ulang.
ON CONFLICT DO NOTHING;

-- ------------------------------------------------------------------
-- Aturan jam absensi
--
-- Tanpa baris ini, pengenalan wajah menolak semua scan dengan "di luar
-- jam absensi" — dan itu terlihat seperti kerusakan, bukan konfigurasi.
-- ------------------------------------------------------------------
-- Jendela dibuat SANGAT LONGGAR, sengaja.
--
-- Aturan sekolah sebenarnya (06:30-09:00) membuat pengujian hanya bisa
-- dilakukan pada jam itu. Di luar jam tersebut setiap scan dijawab "Di
-- luar jam absensi" - benar menurut aturan, tetapi tidak dapat dibedakan
-- dari kerusakan oleh orang yang sedang mencoba sistemnya pertama kali.
--
--   check_in_due_at 23:58  -> praktis tidak pernah terlambat
--   active_weekdays 127    -> tujuh hari, termasuk Sabtu-Minggu
--   require_check_out FALSE -> satu scan sudah menyelesaikan absensi
--
-- HAPUS baris ini sebelum produksi. Jendela 24 jam berarti siswa dapat
-- tercatat hadir pada jam berapa pun, termasuk tengah malam.
INSERT INTO attendance_rules (school_id, name, check_in_opens_at, check_in_start_at,
                              check_in_due_at, check_in_closes_at,
                              check_out_opens_at, check_out_closes_at,
                              late_grace_minutes, active_weekdays, require_check_out)
SELECT s.id, 'DEMO - tanpa batas jam', '00:00', '00:01', '23:58', '23:59',
       '00:01', '23:59', 0, 127, FALSE
FROM schools s WHERE s.npsn = '10259001'
  AND NOT EXISTS (
      SELECT 1 FROM attendance_rules ar
      WHERE ar.school_id = s.id AND ar.classroom_id IS NULL
  );

-- Bila aturan lama dari jalan sebelumnya masih ada, longgarkan.
UPDATE attendance_rules ar
   SET name = 'DEMO - tanpa batas jam',
       check_in_opens_at = '00:00', check_in_start_at = '00:01',
       check_in_due_at = '23:58', check_in_closes_at = '23:59',
       check_out_opens_at = '00:01', check_out_closes_at = '23:59',
       late_grace_minutes = 0, active_weekdays = 127,
       require_check_out = FALSE, updated_at = NOW()
 FROM schools s
WHERE s.npsn = '10259001' AND ar.school_id = s.id AND ar.classroom_id IS NULL;

COMMIT;

-- =====================================================================
-- AKUN
--
-- Semua kata sandi: Demo#2026
--
-- must_change_password sengaja FALSE untuk akun demo. Di alur sebenarnya
-- nilainya TRUE — tetapi akun uji yang memaksa ganti kata sandi pada
-- login pertama membuat pengujian berulang jadi menyusahkan.
-- =====================================================================

BEGIN;

-- Fungsi bantu: satu tempat untuk membuat akun beserta perannya, supaya
-- tidak ada blok INSERT yang tertinggal saat aturannya berubah.
CREATE OR REPLACE FUNCTION demo_akun(
    p_nama       TEXT,
    p_username   TEXT,
    p_email      TEXT,
    p_identitas  TEXT,
    p_jenis      TEXT,          -- 'nik' | 'nisn'
    p_peran      TEXT,
    p_school_id  UUID,
    p_student_id UUID DEFAULT NULL,
    p_nip        TEXT DEFAULT NULL,
    p_jabatan    TEXT DEFAULT NULL
) RETURNS UUID AS $$
DECLARE
    v_id      UUID;
    v_role_id BIGINT;
BEGIN
    INSERT INTO users (name, username, email, password, school_id, student_id,
                       identity_number, identity_type, employee_no, position,
                       is_active, must_change_password, email_verified_at)
    VALUES (p_nama, p_username, p_email,
            crypt('Demo#2026', gen_salt('bf', 12)),
            p_school_id, p_student_id, p_identitas, p_jenis, p_nip, p_jabatan,
            TRUE, FALSE, NOW())
    ON CONFLICT (username) DO UPDATE
        SET password = crypt('Demo#2026', gen_salt('bf', 12)),
            identity_number = EXCLUDED.identity_number,
            identity_type   = EXCLUDED.identity_type,
            student_id      = EXCLUDED.student_id,
            school_id       = EXCLUDED.school_id,
            must_change_password = FALSE,
            is_active       = TRUE,
            deleted_at      = NULL
    RETURNING id INTO v_id;

    SELECT id INTO v_role_id FROM roles WHERE name = p_peran AND guard_name = 'web';
    IF v_role_id IS NULL THEN
        RAISE EXCEPTION 'peran % belum terdaftar', p_peran;
    END IF;

    INSERT INTO model_has_roles (role_id, model_type, model_id)
    VALUES (v_role_id, 'App\Models\User', v_id)
    ON CONFLICT DO NOTHING;

    RETURN v_id;
END;
$$ LANGUAGE plpgsql;

DO $$
DECLARE
    v_school   UUID;
    v_kepsek   UUID;
    v_guru     UUID;
    v_ortu     UUID;
    v_siswa    RECORD;
    v_anak     UUID;
BEGIN
    SELECT id INTO v_school FROM schools WHERE npsn = '10259001';
    IF v_school IS NULL THEN
        RAISE EXCEPTION 'sekolah demo belum ada - jalankan bagian pertama seed';
    END IF;

    -- ---- Peran tingkat sekolah -----------------------------------
    v_kepsek := demo_akun('Dra. Siti Aminah, M.Pd', 'kepsek.demo',
                          'kepsek.demo@disdik.sumutprov.go.id',
                          '1275010000000001', 'nik', 'kepala_sekolah', v_school,
                          NULL, '196805121994031002', 'Kepala Sekolah');

    v_guru := demo_akun('Budi Hartono, S.Pd', 'guru.demo',
                        'guru.demo@disdik.sumutprov.go.id',
                        '1275010000000002', 'nik', 'guru', v_school,
                        NULL, '198203152009011005', 'Guru Matematika');

    PERFORM demo_akun('Rina Wati', 'staff.demo',
                      'staff.demo@disdik.sumutprov.go.id',
                      '1275010000000003', 'nik', 'staff_tu', v_school,
                      NULL, '199001202015012003', 'Staff Tata Usaha');

    -- Guru demo dijadikan wali kelas X IPA 1 supaya layar "kelas saya"
    -- di aplikasi tidak kosong.
    UPDATE classrooms SET homeroom_teacher_id = v_guru
    WHERE school_id = v_school AND name = 'X IPA 1';

    -- ---- Peran tingkat provinsi ----------------------------------
    PERFORM demo_akun('Petugas Pengaduan Dinas', 'petugas.demo',
                      'petugas.demo@disdik.sumutprov.go.id',
                      '1275020000000001', 'nik', 'petugas_pengaduan', NULL);

    PERFORM demo_akun('Admin Dinas Pendidikan', 'dinas.demo',
                      'dinas.demo@disdik.sumutprov.go.id',
                      '1275020000000002', 'nik', 'admin_dinas', NULL);

    -- ---- Akun siswa ---------------------------------------------
    -- Login memakai NISN. Akun siswa mewarisi sekolah dari data siswanya,
    -- bukan dari input — keduanya harus selalu sama.
    FOR v_siswa IN
        SELECT id, nisn, full_name FROM students
        WHERE school_id = v_school AND nisn IS NOT NULL
        ORDER BY nisn
    LOOP
        PERFORM demo_akun(v_siswa.full_name,
                          'siswa' || v_siswa.nisn,
                          v_siswa.nisn || '@siswa.jargon.local',
                          v_siswa.nisn, 'nisn', 'siswa', v_school, v_siswa.id);
    END LOOP;

    -- ---- Akun orang tua -----------------------------------------
    -- Cakupannya diturunkan dari student_guardians, BUKAN dari school_id:
    -- anak-anak seorang wali bisa bersekolah di tempat berbeda.
    SELECT id INTO v_anak FROM students
    WHERE school_id = v_school AND nisn = '0071234501';

    v_ortu := demo_akun('Rudi Nasution', 'ortu.demo',
                        'ortu.demo@example.id',
                        '1275030000000001', 'nik', 'orang_tua', NULL);

    INSERT INTO student_guardians (student_id, school_id, relation, full_name,
                                   phone, whatsapp, preferred_channel,
                                   is_primary, notify_enabled, user_id)
    VALUES (v_anak, v_school, 'ayah', 'Rudi Nasution',
            '081234567890', '081234567890', 'whatsapp', TRUE, TRUE, v_ortu)
    ON CONFLICT DO NOTHING;

    -- Bila baris wali sudah ada dari jalan sebelumnya, cukup tautkan.
    UPDATE student_guardians
       SET user_id = v_ortu
     WHERE student_id = v_anak AND relation = 'ayah' AND user_id IS NULL;
END $$;

DROP FUNCTION IF EXISTS demo_akun(TEXT,TEXT,TEXT,TEXT,TEXT,TEXT,UUID,UUID,TEXT,TEXT);

COMMIT;

-- =====================================================================
-- Riwayat absensi 14 hari terakhir
--
-- Tanpa ini menu Absensi kosong, dan layar kosong tidak membuktikan
-- apa pun saat pengujian — tidak bisa dibedakan antara "tidak ada data"
-- dan "gagal memuat".
--
-- Statusnya divariasikan (hadir/terlambat/sakit/alfa) supaya setiap
-- cabang tampilan status benar-benar terlihat.
-- =====================================================================

BEGIN;

-- Partisi bulanan harus ada lebih dulu; INSERT ke rentang tanpa partisi
-- akan jatuh ke partisi DEFAULT dan melambat.
SELECT ensure_attendance_partitions(2);

INSERT INTO attendances (
    attendance_date, school_id, student_id, classroom_id, academic_year_id,
    student_name, student_nis, classroom_name, school_name,
    check_in_at, check_out_at, status, late_minutes, duration_minutes,
    check_in_method, check_out_method, notification_status
)
SELECT
    d.hari,
    s.school_id,
    s.id,
    s.current_classroom_id,
    ay.id,
    s.full_name,
    s.nis,
    c.name,
    sc.name,
    -- Jam masuk dibuat menyebar di sekitar jam batas supaya sebagian
    -- terhitung terlambat secara wajar, bukan seragam.
    CASE WHEN v.status IN ('sakit','alfa') THEN NULL
         ELSE (d.hari + TIME '06:45' + (v.geser || ' minutes')::interval)
              AT TIME ZONE 'Asia/Jakarta'
    END,
    CASE WHEN v.status IN ('sakit','alfa') THEN NULL
         ELSE (d.hari + TIME '15:10') AT TIME ZONE 'Asia/Jakarta'
    END,
    v.status,
    GREATEST(0, v.geser - 35),
    CASE WHEN v.status IN ('sakit','alfa') THEN NULL ELSE 505 - v.geser END,
    CASE WHEN v.status IN ('sakit','alfa') THEN NULL ELSE 'face' END,
    CASE WHEN v.status IN ('sakit','alfa') THEN NULL ELSE 'face' END,
    'skipped'
FROM students s
JOIN schools sc      ON sc.id = s.school_id
LEFT JOIN classrooms c ON c.id = s.current_classroom_id
CROSS JOIN (SELECT id FROM academic_years WHERE is_active ORDER BY name DESC LIMIT 1) ay
CROSS JOIN generate_series(CURRENT_DATE - 13, CURRENT_DATE, INTERVAL '1 day') AS d(hari)
CROSS JOIN LATERAL (
    SELECT
        CASE
            -- Pola tetap berdasarkan tanggal + NISN: hasilnya sama setiap
            -- kali seed dijalankan, jadi pengujian bisa diulang.
            WHEN (EXTRACT(DAY FROM d.hari)::int + right(s.nisn, 1)::int) % 11 = 0 THEN 'alfa'
            WHEN (EXTRACT(DAY FROM d.hari)::int + right(s.nisn, 1)::int) % 7  = 0 THEN 'sakit'
            WHEN (EXTRACT(DAY FROM d.hari)::int + right(s.nisn, 1)::int) % 5  = 0 THEN 'terlambat'
            ELSE 'hadir'
        END AS status,
        ((EXTRACT(DAY FROM d.hari)::int * 7 + right(s.nisn, 1)::int * 3) % 55) AS geser
) v
WHERE sc.npsn = '10259001'
  -- Sabtu (6) dan Minggu (0) dilewati: hari tanpa sekolah yang punya baris
  -- absensi akan membuat rekap persentase kehadiran salah.
  AND EXTRACT(DOW FROM d.hari) NOT IN (0, 6)
ON CONFLICT (attendance_date, student_id) DO NOTHING;

COMMIT;
