#!/bin/sh
# =====================================================================
# Penyiapan dashboard saat container start.
#
# Yang dilakukan di sini hanya hal yang BERGANTUNG PADA LINGKUNGAN dan
# karenanya tidak bisa dipanggang ke dalam image: APP_KEY, cache config
# yang memuat nilai environment, dan menunggu database.
# =====================================================================
set -e

cd /var/www/admin

# --- APP_KEY ---------------------------------------------------------
# Laravel menolak jalan tanpa APP_KEY. Bila tidak disetel lewat .env,
# dibuatkan sekali di sini.
#
# Kunci ini mengenkripsi cookie sesi. Kunci yang dibuat saat start berarti
# sesi login hilang setiap container dibangun ulang — merepotkan, tetapi
# jauh lebih baik daripada memanggang satu kunci tetap ke dalam image yang
# dibagikan, karena kunci itu akan sama di setiap pemasangan.
if [ -z "${APP_KEY}" ]; then
    echo "  APP_KEY kosong - membuat kunci sementara."
    echo "  Isi APP_KEY di .env agar sesi login bertahan antar-restart."
    APP_KEY="base64:$(head -c 32 /dev/urandom | base64)"
    export APP_KEY
fi

# --- Menunggu database ----------------------------------------------
# depends_on: service_healthy sudah menjamin PostgreSQL siap, tetapi
# container ini juga bisa dijalankan sendiri (docker compose run admin).
echo "  Menunggu PostgreSQL di ${DB_HOST:-postgres}:${DB_PORT:-5432}..."
i=0
while [ $i -lt 60 ]; do
    if php -r '
        $h = getenv("DB_HOST") ?: "postgres";
        $p = getenv("DB_PORT") ?: "5432";
        exit(@fsockopen($h, (int) $p, $e, $s, 2) ? 0 : 1);
    '; then
        break
    fi
    i=$((i + 1))
    sleep 1
done

# --- Cache ------------------------------------------------------------
# Config dan route di-cache SETELAH environment terbaca, bukan saat build:
# nilai seperti ABSENSI_API_URL berbeda antar-pemasangan, dan cache yang
# dipanggang ke image akan membawa nilai mesin yang membangunnya.
php artisan config:clear >/dev/null 2>&1 || true
php artisan config:cache >/dev/null 2>&1 || true
php artisan route:cache  >/dev/null 2>&1 || true
php artisan view:cache   >/dev/null 2>&1 || true

# Tautan storage publik (avatar pengguna). Diabaikan bila sudah ada.
php artisan storage:link >/dev/null 2>&1 || true

# CATATAN: `php artisan migrate` SENGAJA tidak dijalankan. Skema dimiliki
# migrasi sqlx di jargon-be/api/migrations, yang dijalankan container API
# saat start. Menjalankan migrasi Laravel di sini akan menghasilkan dua
# pemilik skema yang sama.

echo "  Dashboard siap di :8000"
exec php artisan serve --host=0.0.0.0 --port=8000
