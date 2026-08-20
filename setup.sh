#!/usr/bin/env bash
# =====================================================================
#  Jargon GO — penyiapan pertama kali (Linux / macOS).
#
#  Membuat .env berisi rahasia acak, lalu membangun dan menjalankan
#  seluruh sistem. Cukup dijalankan SEKALI; berikutnya pakai:
#
#      docker compose up -d
#
#  Pemakaian:
#    ./setup.sh          siapkan .env, bangun, jalankan
#    ./setup.sh env      siapkan .env saja, jangan jalankan
# =====================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

echo
echo "  Jargon GO — penyiapan"
echo "  ====================="
echo

# --- jargon-fe harus ada di sebelah -----------------------------------
# Service `web` dibangun dari ../jargon-fe (repositori terpisah). Diperiksa
# di sini karena galat bawaan Docker untuk build context yang hilang tidak
# menyebutkan sama sekali bahwa yang kurang adalah sebuah repositori.
if [ ! -f ../jargon-fe/pubspec.yaml ]; then
    echo "  [x] Repositori jargon-fe tidak ditemukan di sebelah repositori ini."
    echo
    echo "      Diharapkan: $(cd .. && pwd)/jargon-fe"
    echo
    echo "      Clone keduanya berdampingan:"
    echo "        git clone https://github.com/rendyirawann/jargon-be.git"
    echo "        git clone https://github.com/rendyirawann/jargon-fe.git"
    echo
    echo "      Tidak ingin membangun dari sumber? Pakai image siap pakai:"
    echo "        docker compose -f docker-compose.prod.yml up -d"
    echo
    exit 1
fi

# --- .env -------------------------------------------------------------
if [ ! -f .env.example ]; then
    echo "  [x] .env.example tidak ditemukan."
    exit 1
fi

if [ ! -f .env ]; then
    cp .env.example .env
    echo "  .env dibuat dari .env.example"
fi

# Rahasia DIBANGKITKAN, bukan diminta diketik. Yang paling sering terjadi
# bukan lupa mengisi JWT_SECRET, melainkan mengisinya dengan sesuatu yang
# mudah diketik — dan JWT_SECRET yang bisa ditebak berarti siapa pun dapat
# menempa token akses untuk peran apa pun, termasuk superadmin.
current() { sed -n "s/^[[:space:]]*$1[[:space:]]*=//p" .env | head -n1 | tr -d '[:space:]'; }

set_value() {
    local key="$1" value="$2"
    if grep -qE "^[[:space:]]*${key}[[:space:]]*=" .env; then
        # Pembatas | dipakai karena nilai base64 dapat memuat garis miring.
        sed -i.bak -E "s|^[[:space:]]*${key}[[:space:]]*=.*|${key}=${value}|" .env
        rm -f .env.bak
    else
        printf '%s=%s\n' "$key" "$value" >> .env
    fi
    CHANGED="${CHANGED}${key} "
}

CHANGED=""

jwt_now="$(current JWT_SECRET)"
if [ "${#jwt_now}" -lt 32 ]; then
    set_value JWT_SECRET "$(openssl rand -base64 48 | tr -d '\n')"
fi

if ! printf '%s' "$(current SECRETS_KEY_HEX)" | grep -qE '^[0-9a-fA-F]{64}$'; then
    set_value SECRETS_KEY_HEX "$(openssl rand -hex 32)"
fi

if [ -z "$(current APP_KEY)" ]; then
    set_value APP_KEY "base64:$(openssl rand -base64 32 | tr -d '\n')"
fi

if [ -n "$CHANGED" ]; then
    echo "  Rahasia dibangkitkan: ${CHANGED}"
else
    echo "  .env sudah lengkap — tidak ada yang diubah."
fi

[ "${1:-}" = "env" ] && { echo; exit 0; }

# --- Docker -----------------------------------------------------------
if ! command -v docker >/dev/null 2>&1; then
    echo
    echo "  [x] Docker tidak ditemukan. Pasang Docker lebih dulu."
    exit 1
fi

if ! docker info >/dev/null 2>&1; then
    echo
    echo "  [x] Docker terpasang tetapi daemonnya belum jalan."
    exit 1
fi

echo
echo "  Membangun image. Yang pertama memakan 10–20 menit: Rust dan Flutter"
echo "  dikompilasi dari nol. Build berikutnya jauh lebih cepat karena layer"
echo "  dependensinya di-cache."
echo

docker compose up -d --build

# --- Tunggu benar-benar melayani --------------------------------------
# `docker compose up -d` kembali segera setelah container dijalankan,
# padahal API masih menjalankan migrasi. Tanpa penantian ini, pesan
# "Selesai" muncul sementara membuka halamannya masih menghasilkan 502.
PORT="$(current HTTP_PORT)"; PORT="${PORT:-80}"
BASE="http://127.0.0.1:${PORT}"

echo
echo "  Menunggu sistem siap..."
for target in "API:${BASE}/health/live" "Aplikasi:${BASE}/" "Dashboard:${BASE}/admin/login"; do
    name="${target%%:*}"; url="${target#*:}"
    for _ in $(seq 1 90); do
        if curl -fsS -o /dev/null --max-time 5 "$url" 2>/dev/null; then
            printf '  [ok] %-10s %s\n' "$name" "$url"
            continue 2
        fi
        sleep 2
    done
    printf '  [!]  %-10s belum menjawab\n' "$name"
    echo   "       Lihat sebabnya:  docker compose logs --tail=50"
done

cat <<EOF

  Selesai.

    Jargon GO   ${BASE}/
    Dashboard   ${BASE}/admin/login
    Swagger     ${BASE}/docs

    Login pertama: superadmin / Superadmin#2026  (ganti segera)

  Perintah harian:
    docker compose up -d          jalankan
    docker compose logs -f api    lihat log API
    docker compose down           hentikan
    docker compose down -v        hentikan + HAPUS seluruh data

EOF
