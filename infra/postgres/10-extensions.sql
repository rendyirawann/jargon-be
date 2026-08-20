-- =====================================================================
-- Dijalankan sekali oleh entrypoint image PostgreSQL, sebelum aplikasi
-- pertama kali terhubung.
--
-- Migrasi sqlx juga membuat ekstensi ini (CREATE EXTENSION IF NOT EXISTS),
-- tetapi melakukannya di sini lebih awal memberi pesan galat yang jauh lebih
-- jelas bila image PostgreSQL yang dipakai ternyata tidak memuat pgvector.
-- =====================================================================

CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE EXTENSION IF NOT EXISTS vector;
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION IF NOT EXISTS btree_gin;
CREATE EXTENSION IF NOT EXISTS unaccent;

-- Verifikasi eksplisit: gagal cepat dengan pesan yang bisa dipahami operator,
-- alih-alih gagal nanti pada migrasi 0005 dengan pesan "type vector does not exist".
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'vector') THEN
        RAISE EXCEPTION
            'Ekstensi pgvector tidak tersedia. Gunakan image pgvector/pgvector:pg17 '
            'atau pasang pgvector pada server PostgreSQL Anda.';
    END IF;
END $$;
