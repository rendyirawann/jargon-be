-- =====================================================================
-- 0014 : Pengenalan wajah dari BROWSER — embedding 128 dimensi
--
-- MENGAPA DIMENSINYA BERUBAH
--
-- Sampai migrasi ini, satu-satunya klien pengenalan wajah adalah tablet
-- Android dengan MobileFaceNet (512-d) lewat TFLite. Mode kios itu tidak
-- dapat dijalankan di browser: `tflite_flutter` berdiri di atas `dart:ffi`
-- yang tidak bisa dikompilasi ke web sama sekali.
--
-- Untuk pengenalan wajah di dashboard `/admin`, model yang dipakai adalah
-- face-api.js (FaceRecognitionNet) yang berjalan langsung di browser dan
-- menghasilkan **128 dimensi**, bukan 512.
--
-- KONSEKUENSI YANG DITERIMA
--
-- `FACE_EMBEDDING_DIM` adalah satu nilai global, dan kolom di bawah punya
-- satu dimensi tetap. Jadi sistem memakai SATU model pada satu waktu:
-- beralih ke 128-d berarti jalur tablet MobileFaceNet dikesampingkan
-- sampai ada model 128-d untuk TFLite.
--
-- Itu pertukaran yang disengaja: jalur tablet toh belum dapat dipakai
-- (berkas mobilefacenet.tflite tidak disertakan repositori), sedangkan
-- pengenalan dari browser dapat berjalan hari ini. Sebagai imbalan,
-- vektor 128-d juga 4x lebih murah dicocokkan dan 4x lebih hemat memori
-- pada index per-sekolah di dalam proses.
--
-- AMAN DIJALANKAN karena tidak ada data yang hilang: mengubah tipe kolom
-- hanya boleh selama belum ada embedding tersimpan, dan migrasi ini
-- MENOLAK berjalan bila sudah ada — lihat pemeriksaan di bawah.
--
-- Untuk kembali ke 512-d nanti: buat migrasi baru dengan arah sebaliknya,
-- lalu DAFTARKAN ULANG seluruh wajah. Embedding lintas model tidak dapat
-- dikonversi — 128 angka itu bukan ringkasan dari 512 angka yang lain.
-- =====================================================================

-- ------------------------------------------------------------------
-- Penjaga: jangan sampai menghapus data biometrik tanpa sadar.
--
-- Mendaftarkan ulang wajah berarti memanggil siswa satu per satu ke depan
-- kamera. Kalau tabel sudah berisi, migrasi ini berhenti dan operator
-- harus memutuskan sendiri.
-- ------------------------------------------------------------------
DO $$
DECLARE
    n BIGINT;
BEGIN
    SELECT COUNT(*) INTO n FROM face_embeddings;
    IF n > 0 THEN
        RAISE EXCEPTION
            'face_embeddings sudah berisi % baris. Mengubah dimensi embedding '
            'akan membuat semuanya tidak dapat dicocokkan. Kosongkan tabel '
            'secara sadar lebih dulu (dan siapkan pendaftaran ulang wajah), '
            'atau pertahankan 512-d dan jangan jalankan migrasi ini.', n;
    END IF;
END $$;

-- ------------------------------------------------------------------
-- Index HNSW harus dibuang lebih dulu: ia mengikat dimensi kolom.
-- ------------------------------------------------------------------
DROP INDEX IF EXISTS face_embeddings_hnsw;

ALTER TABLE face_embeddings
    ALTER COLUMN embedding TYPE VECTOR(128);

ALTER TABLE face_embeddings
    ALTER COLUMN model_version SET DEFAULT 'faceapi-v1';

-- Index dibuat ulang pada dimensi baru.
--
-- Catatan: pencocokan sehari-hari TIDAK memakai index ini. Pencarian
-- selalu bercakupan satu sekolah — ratusan vektor — sehingga brute force
-- tepat di dalam proses lebih cepat DAN lebih akurat daripada ANN. Index
-- ini disimpan untuk keperluan lain (mis. pemeriksaan duplikat lintas
-- sekolah) dan agar kolomnya tetap punya bentuk yang jelas.
CREATE INDEX IF NOT EXISTS face_embeddings_hnsw
    ON face_embeddings USING hnsw (embedding vector_cosine_ops);

-- ------------------------------------------------------------------
-- Versi model pada pengaturan aplikasi, supaya dashboard menampilkan
-- nilai yang sama dengan yang diperiksa API.
-- ------------------------------------------------------------------
INSERT INTO settings (key, value, created_at, updated_at) VALUES
    ('face_model_version', 'faceapi-v1', NOW(), NOW()),
    ('face_embedding_dim', '128',        NOW(), NOW())
ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW();
