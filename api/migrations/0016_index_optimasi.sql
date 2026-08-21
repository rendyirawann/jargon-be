-- =====================================================================
-- 0016 : Penajaman index berdasarkan query yang BENAR-BENAR dijalankan
--
-- Bukan menambah index untuk setiap foreign key. Basis data ini punya 23
-- foreign key tanpa index, dan meng-index semuanya akan memperlambat
-- setiap INSERT tanpa mempercepat satu query pun yang nyata: pada jam
-- puncak, `attendance_events` menerima ~520 baris per detik, dan setiap
-- index tambahan adalah pekerjaan tulis tambahan pada setiap baris itu.
--
-- Yang di bawah dipilih dari membaca query di kode, bukan dari daftar
-- foreign key. Untuk masing-masing dicatat query mana yang dilayaninya.
--
-- CATATAN UNTUK DATABASE YANG SUDAH BESAR
--
-- CREATE INDEX di sini TANPA CONCURRENTLY, karena migrasi sqlx berjalan di
-- dalam transaksi dan CONCURRENTLY tidak diizinkan di dalam transaksi.
-- Pada tabel yang sudah berisi puluhan juta baris, perintah ini MENGUNCI
-- tulis selama index dibangun. Bila itu terjadi di produksi yang sedang
-- melayani absensi pagi, jalankan versi CONCURRENTLY-nya secara manual di
-- luar jam sibuk lebih dulu, lalu migrasi ini menjadi no-op karena memakai
-- IF NOT EXISTS.
-- =====================================================================

-- ------------------------------------------------------------------
-- 1. Pemuatan index wajah per sekolah  (jalur terpanas)
--
-- Query di src/face/index.rs:
--
--   SELECT ... FROM face_embeddings fe JOIN students s ...
--   WHERE fe.school_id = $1 AND fe.is_active AND s.status = 'aktif'
--   ORDER BY fe.school_id, fe.student_id, fe.created_at
--
-- Index lama hanya (school_id), sehingga ORDER BY memaksa tahap SORT atas
-- seluruh vektor sekolah itu. Pekerjaan itu berulang setiap kali TTL cache
-- habis — untuk ribuan sekolah, terus-menerus.
--
-- Index baru memuat ketiga kolom urut, jadi hasilnya keluar dalam urutan
-- yang diminta tanpa sort. Index lama DIBUANG, bukan dibiarkan: kolom
-- depannya sama (school_id) dan predikat partialnya sama, jadi ia menjadi
-- duplikat yang hanya menambah biaya tulis.
-- ------------------------------------------------------------------
CREATE INDEX IF NOT EXISTS face_embeddings_school_student_idx
    ON face_embeddings (school_id, student_id, created_at)
    WHERE is_active;

DROP INDEX IF EXISTS face_embeddings_school_active_idx;

-- ------------------------------------------------------------------
-- 2. Pemeriksaan anti-replay embedding  (dijalankan setiap scan)
--
-- Query di src/services/recognition.rs:
--
--   SELECT 1 FROM attendance_events
--   WHERE device_id = $1 AND embedding_hash = $2
--     AND occurred_at > NOW() - INTERVAL '10 minutes'
--   LIMIT 1
--
-- Index lama hanya (embedding_hash): baris yang cocok masih harus diambil
-- dari heap untuk memeriksa device_id dan occurred_at. Index baru memuat
-- ketiganya, sehingga pemeriksaan selesai di dalam index.
--
-- Jumlah index pada tabel ini TIDAK bertambah — yang lama dibuang. Itu
-- disengaja: tabel ini yang paling banyak menerima INSERT di seluruh
-- sistem, dan setiap index tambahan terasa pada setiap absensi.
-- ------------------------------------------------------------------
CREATE INDEX IF NOT EXISTS attendance_events_replay2_idx
    ON attendance_events (embedding_hash, device_id, occurred_at DESC);

DROP INDEX IF EXISTS attendance_events_replay_idx;

-- ------------------------------------------------------------------
-- 3. face_embeddings.enrollment_id
--
-- Menghapus satu sampel wajah (/admin/biometric, hapus sampel) memicu
-- ON DELETE CASCADE dari face_enrollments ke face_embeddings. Tanpa index
-- pada kolom penunjuknya, PostgreSQL memindai SELURUH face_embeddings
-- untuk mencari baris anak.
--
-- Pada skala provinsi tabel itu berisi ~2,1 juta baris (700.000 siswa x 3
-- sampel), sehingga menghapus satu foto salah ambil menjadi operasi yang
-- memindai jutaan baris.
-- ------------------------------------------------------------------
CREATE INDEX IF NOT EXISTS face_embeddings_enrollment_idx
    ON face_embeddings (enrollment_id)
    WHERE enrollment_id IS NOT NULL;

-- ------------------------------------------------------------------
-- 4. panic_reports.category_id
--
-- Beranda pengaduan menyaring per kategori (`c.code = $3` setelah JOIN ke
-- panic_categories). Tanpa index ini, penyaringan kategori memindai
-- laporan menurut waktu lalu membuang yang tidak cocok.
--
-- Disertakan created_at DESC karena feed SELALU diurutkan terbaru dulu —
-- kategori tanpa urutan hanya memindahkan pekerjaan ke tahap sort.
-- ------------------------------------------------------------------
CREATE INDEX IF NOT EXISTS panic_reports_category_idx
    ON panic_reports (category_id, created_at DESC);

-- ------------------------------------------------------------------
-- 5. document_files.document_type_id
--
-- Daftar periksa pemberkasan mencocokkan berkas terunggah dengan jenis
-- dokumen. Index unik yang ada, (submission_id, document_type_id),
-- berkolom depan submission_id sehingga tidak dapat melayani pencarian
-- berdasarkan jenis dokumen saja — termasuk saat sebuah jenis dokumen
-- dinonaktifkan atau dihapus dari /admin/documents/types.
-- ------------------------------------------------------------------
CREATE INDEX IF NOT EXISTS document_files_type_idx
    ON document_files (document_type_id)
    WHERE document_type_id IS NOT NULL;

-- ------------------------------------------------------------------
-- YANG SENGAJA TIDAK DI-INDEX
--
--   face_enrollments.captured_by / reviewed_by / device_id
--     Hanya diperlukan saat menghapus pengguna atau perangkat — tindakan
--     administratif yang jarang. Tiga index tambahan pada tabel berisi
--     jutaan baris akan membebani setiap pendaftaran wajah, demi
--     mempercepat operasi yang terjadi beberapa kali setahun.
--
--   panic_supports.user_id
--     Query yang ada memakai (report_id, user_id) dan itu SUDAH primary
--     key tabel ini.
--
--   student_guardians.school_id
--     Penyaringan cakupan orang tua bermula dari user_id (ter-index), lalu
--     school_id hanya menyaring beberapa baris hasilnya.
--
--   classrooms.academic_year_id, regions.parent_id, dan FK audit lainnya
--     Tabel kecil, atau hanya disentuh operasi administratif langka.
-- ------------------------------------------------------------------

-- Statistik disegarkan agar perencana query langsung memakai index baru,
-- bukan menunggu autovacuum. Tanpa ini, query pertama setelah migrasi
-- masih bisa memilih rencana lama.
ANALYZE face_embeddings;
ANALYZE attendance_events;
ANALYZE panic_reports;
ANALYZE document_files;
