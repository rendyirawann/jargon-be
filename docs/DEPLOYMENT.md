# Panduan Pemasangan & Operasional

---

## 1. Prasyarat

| Komponen | Versi minimum | Catatan |
|---|---|---|
| PostgreSQL | 15 (disarankan 17) | **wajib** dengan ekstensi `pgvector` |
| Redis | 6 | opsional tapi sangat dianjurkan |
| Rust | 1.82 | untuk membangun API |
| PHP | 8.3 | dashboard; ekstensi `pdo_pgsql`, `redis`, `intl`, `zip` |
| Flutter | 3.35+ | untuk membangun aplikasi tablet |
| Node.js | 20 | aset dashboard (Vite) |

---

## 2. Urutan pemasangan yang benar

> **Penting:** skema database dimiliki oleh migrasi **sqlx** di
> `jargon-be/api/migrations`, bukan oleh migrasi Laravel. Migrasi Laravel
> sudah ditandai selesai oleh `0010_seed.sql`, sehingga
> `php artisan migrate` menjadi no-op yang aman. **Jangan** menjalankannya
> sebagai bagian dari alur pemasangan.

### 2.1 Database

```bash
createdb absensi
psql absensi -c 'CREATE EXTENSION IF NOT EXISTS vector;'
```

Bila memakai Docker, ekstensi sudah terpasang oleh
`infra/postgres/10-extensions.sql`.

### 2.2 API (Rust)

```bash
cd jargon-be/api
cp .env.example .env

# Isi dua nilai WAJIB:
openssl rand -base64 48   # -> JWT_SECRET
openssl rand -hex 32      # -> SECRETS_KEY_HEX

cargo run --release
```

Migrasi dijalankan otomatis saat start. sqlx memakai advisory lock, jadi aman
walau beberapa replika start bersamaan.

Verifikasi:

```bash
curl http://localhost:8080/health        # status komponen
open  http://localhost:8080/docs         # Swagger UI
```

### 2.3 Dashboard (Laravel)

```bash
cd jargon-be/admin
cp .env.example .env
composer install
php artisan key:generate
npm install && npm run build

# Octane (produksi)
php artisan octane:install --server=roadrunner
php artisan octane:start --host=0.0.0.0 --port=8000

# atau untuk pengembangan
php artisan serve
```

### 2.4 Kredensial layanan untuk dashboard

Dashboard memanggil API untuk semua operasi tulis. Buat satu klien layanan:

```sql
-- key_id dan secret dihasilkan sendiri; hanya SHA-256 secret yang disimpan.
INSERT INTO api_clients (name, key_id, secret_hash, scopes)
VALUES (
  'Dashboard Admin',
  'dashboard-admin',
  digest('GANTI-DENGAN-SECRET-ACAK-PANJANG', 'sha256'),
  ARRAY['*']
);
```

Lalu isi pada `admin/.env`:

```ini
ABSENSI_API_KEY_ID=dashboard-admin
ABSENSI_API_SECRET=GANTI-DENGAN-SECRET-ACAK-PANJANG
```

Nama variabelnya `ABSENSI_API_*` — itu yang dibaca `config/services.php`.

### 2.5 Login pertama

```
URL      : http://localhost:8000/admin/login
Username : superadmin
Password : Superadmin#2026
```

**Ganti kata sandi ini segera.** Akun ditandai `must_change_password`.

### 2.6 Aplikasi tablet (Flutter)

```bash
cd jargon-fe

# Salin model TFLite ke assets/models/mobilefacenet.tflite
# (lihat assets/models/README.md untuk persyaratannya)

flutter pub get
flutter build apk --release \
  --dart-define=API_BASE_URL=https://absensi.disdik.sumutprov.go.id \
  --dart-define=FACE_MODEL_VERSION=mobilefacenet-v1 \
  --dart-define=FACE_EMBEDDING_DIM=512
```

`FACE_MODEL_VERSION` dan `FACE_EMBEDDING_DIM` **harus sama** dengan nilai di
`api/.env`. Bila berbeda, server menolak setiap request dengan pesan
"Perbarui aplikasi" — itu memang perilaku yang diinginkan, karena embedding
lintas versi model tidak sebanding.

### 2.7 Semuanya dengan Docker

Jauh lebih singkat daripada §2.1–2.6, dan inilah cara yang dianjurkan untuk
mencoba maupun mendemokan sistem.

```bash
git clone https://github.com/rendyirawann/jargon-be.git
git clone https://github.com/rendyirawann/jargon-fe.git   # harus berdampingan
cd jargon-be
setup.bat          # Windows
./setup.sh         # Linux/macOS
```

`setup` membuat `.env` berisi rahasia acak, lalu membangun dan menjalankan
semuanya. Berikutnya cukup `docker compose up -d`.

Semuanya dilayani satu nginx di `HTTP_PORT` (bawaan 80):

| | Alamat |
|---|---|
| Aplikasi Jargon GO | <http://localhost/> |
| Dashboard | <http://localhost/admin/login> |
| Swagger UI | <http://localhost/docs> |

**Di server, jangan bangun dari sumber.** Pakai image siap pakai — tidak
perlu meng-clone `jargon-fe`, dan tidak perlu toolchain apa pun:

```bash
docker compose -f docker-compose.prod.yml pull
docker compose -f docker-compose.prod.yml up -d
```

Yang perlu dikirim ke server hanya `docker-compose.prod.yml`, `.env`, dan
folder `infra/`.

> `infra/docker-compose.yml` **bukan** untuk ini — isinya hanya PostgreSQL
> dan Redis, dipakai `dev.bat` saat API/dashboard/aplikasi dijalankan
> langsung di mesin dengan hot reload.

---

## 2.8 Deploy ke server TANPA Docker

Semua yang di atas berjalan langsung di sistem — tidak ada container. Ini
jalur yang dipakai bila server sudah punya PostgreSQL sendiri, atau bila
kebijakan tidak mengizinkan Docker.

Contoh di bawah memakai **Ubuntu 24.04**. Di Linux pgvector jauh lebih
mudah daripada di Windows: ada paket apt resminya, tidak perlu kompilasi.

### 1. Paket sistem

```bash
sudo apt update
sudo apt install -y postgresql-17 postgresql-17-pgvector redis-server nginx \
                    php8.3-cli php8.3-pgsql php8.3-zip php8.3-intl php8.3-gd \
                    php8.3-mbstring php8.3-xml php8.3-curl php8.3-redis \
                    php8.3-sockets unzip git curl
```

`postgresql-17-pgvector` adalah bagian yang paling sering terlewat. Tanpa
ekstensi `vector`, migrasi `0001` berhenti dan API tidak akan start.

### 2. Database

```bash
sudo -u postgres psql <<'SQL'
CREATE ROLE absensi LOGIN PASSWORD 'ganti-kata-sandi-ini';
CREATE DATABASE absensi OWNER absensi;
\c absensi
CREATE EXTENSION IF NOT EXISTS vector;
SQL
```

Setel juga `max_locks_per_transaction = 256` di `postgresql.conf`. Nilai
bawaannya terlalu kecil: `attendances` dipartisi bulanan, dan satu query
rekap menyentuh banyak partisi sekaligus.

### 3. API (Rust)

Kompilasi di mesin build, bukan di server produksi — server tidak perlu
punya toolchain Rust.

```bash
# di mesin build
git clone https://github.com/rendyirawann/jargon-be.git
cd jargon-be/api
cargo build --release --locked      # butuh rustc >= 1.88

# kirim ke server
scp target/release/jargon-api  server:/opt/jargon/
scp -r migrations              server:/opt/jargon/
```

Di server:

```bash
sudo useradd --system --home /opt/jargon --shell /usr/sbin/nologin jargon
sudo mkdir -p /opt/jargon/storage && sudo chown -R jargon:jargon /opt/jargon
```

`/opt/jargon/.env` — minimal:

```ini
APP_ENV=production
BIND_ADDR=127.0.0.1:8080
PUBLIC_URL=https://absensi.disdik.sumutprov.go.id
DATABASE_URL=postgres://absensi:kata-sandi@127.0.0.1:5432/absensi
REDIS_URL=redis://127.0.0.1:6379/2
JWT_SECRET=<openssl rand -base64 48>
SECRETS_KEY_HEX=<openssl rand -hex 32>
STORAGE_ROOT=/opt/jargon/storage
CORS_ALLOWED_ORIGINS=https://absensi.disdik.sumutprov.go.id
ENABLE_SWAGGER=false
```

`BIND_ADDR` sengaja `127.0.0.1`, bukan `0.0.0.0`: API hanya boleh
dijangkau lewat nginx, sehingga TLS dan rate limit tidak bisa dilewati
dengan memanggil port 8080 langsung.

`/etc/systemd/system/jargon-api.service`:

```ini
[Unit]
Description=Jargon GO API
After=network-online.target postgresql.service redis-server.service
Wants=postgresql.service

[Service]
Type=simple
User=jargon
WorkingDirectory=/opt/jargon
EnvironmentFile=/opt/jargon/.env
ExecStart=/opt/jargon/jargon-api
Restart=always
RestartSec=5

# Migrasi dijalankan saat start dan memakai advisory lock, jadi beberapa
# instance yang start bersamaan tetap aman.

NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/jargon/storage

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable --now jargon-api
sudo systemctl status jargon-api
curl -fsS http://127.0.0.1:8080/health
```

### 4. Dashboard (Laravel)

```bash
sudo git clone https://github.com/rendyirawann/jargon-be.git /var/www/jargon
cd /var/www/jargon/admin
sudo composer install --no-dev --optimize-autoloader
sudo cp .env.example .env
sudo php artisan key:generate
```

Sunting `/var/www/jargon/admin/.env`:

```ini
APP_ENV=production
APP_DEBUG=false
APP_URL=https://absensi.disdik.sumutprov.go.id
ASSET_URL=/admin
DB_CONNECTION=pgsql
DB_HOST=127.0.0.1
DB_DATABASE=absensi
DB_USERNAME=absensi
DB_PASSWORD=kata-sandi
REDIS_HOST=127.0.0.1
SESSION_DRIVER=redis
CACHE_STORE=redis
ABSENSI_API_URL=http://127.0.0.1:8080
ABSENSI_API_PUBLIC_URL=https://absensi.disdik.sumutprov.go.id
```

`ASSET_URL=/admin` **wajib** bila aplikasi Jargon GO dilayani di `/` pada
host yang sama: Metronic menaruh asetnya di `/assets/` dan Flutter web
juga. Tanpa awalan ini keduanya bertabrakan dan salah satu tampil rusak.

```bash
sudo php artisan config:cache route:cache view:cache
sudo chown -R www-data:www-data storage bootstrap/cache
```

**JANGAN** menjalankan `php artisan migrate`. Skema dimiliki migrasi sqlx
di `api/migrations`, yang sudah dijalankan API saat start; migrasi Laravel
sudah ditandai selesai oleh seed.

Pakai Octane untuk produksi:

```bash
sudo composer require laravel/octane spiral/roadrunner-cli
sudo php artisan octane:install --server=roadrunner
```

`/etc/systemd/system/jargon-admin.service`:

```ini
[Unit]
Description=Jargon GO Dashboard (Octane)
After=network-online.target postgresql.service

[Service]
Type=simple
User=www-data
WorkingDirectory=/var/www/jargon/admin
ExecStart=/usr/bin/php artisan octane:start --server=roadrunner --host=127.0.0.1 --port=8000
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### 5. Aplikasi web (Flutter)

Dibangun di mesin build; server hanya menerima berkas statis.

```bash
git clone https://github.com/rendyirawann/jargon-fe.git
cd jargon-fe
flutter build web --release --dart-define=API_SAME_ORIGIN=true
scp -r build/web/* server:/var/www/jargon-web/
```

`API_SAME_ORIGIN=true` membuat aplikasi mengambil alamat API dari origin
halamannya sendiri. Itu sebabnya bundel yang sama benar untuk domain apa
pun — tidak ada alamat yang dipaku, jadi tidak perlu dibangun ulang saat
domain berubah.

### 6. nginx

Salin `infra/nginx/absensi.conf` sebagai titik awal, lalu ganti tiga blok
`upstream`/root menjadi:

```nginx
upstream absensi_api   { server 127.0.0.1:8080; keepalive 32; }
upstream absensi_admin { server 127.0.0.1:8000; keepalive 16; }

# Aplikasi web dilayani nginx langsung, bukan lewat container `web`.
# Ganti blok `location /` yang memakai proxy_pass http://jargon_web
# menjadi:
location / {
    root /var/www/jargon-web;
    try_files $uri $uri/ /index.html;
}
```

Sisa berkasnya — pembagian `/admin`, `/v1`, `/docs`, `/files`, penulisan
ulang `/admin/assets/`, dan batas waktu ketat untuk `/v1/kiosk/` — dipakai
apa adanya.

```bash
sudo nginx -t && sudo systemctl reload nginx
sudo certbot --nginx -d absensi.disdik.sumutprov.go.id
```

HTTPS bukan opsional: tablet mengirim data biometrik, dan aplikasi Flutter
build release menolak HTTP tanpa TLS.

### 7. Yang HILANG dibanding container

| | Container | Tanpa Docker |
|---|---|---|
| Pembaruan | `docker compose pull && up -d` | salin biner + `systemctl restart` |
| Rollback | ganti `IMAGE_TAG` | simpan biner versi sebelumnya sendiri |
| Kesamaan lingkungan | dijamin image | bergantung paket sistem tiap server |
| pgvector | sudah di dalam image | paket apt (Linux) / kompilasi (Windows) |

Yang paling terasa adalah rollback. Dengan container, versi sebelumnya
masih ada sebagai image. Tanpa Docker, biner lama harus disimpan sendiri —
`/opt/jargon/jargon-api.v1` dan seterusnya — kalau tidak, satu-satunya
jalan mundur adalah mengompilasi ulang dari commit lama.

---

## 2.9 Akun untuk pengujian

Pemasangan baru hanya berisi **satu** akun — `superadmin` dari seed — dan
nol sekolah, kelas, maupun siswa. Aplikasi mobile akan tampak kosong, dan
layar kosong tidak membuktikan apa pun saat pengujian: tidak bisa
dibedakan antara "tidak ada data" dan "gagal memuat".

Untuk mengisinya:

```bat
scripts\seed-demo.bat
```

Atau langsung:

```bash
docker compose exec -T postgres psql -U absensi -d absensi < api/seeds/demo.sql
```

Aman dijalankan berulang: setiap `INSERT` memakai `ON CONFLICT` dan
pengenal barisnya tetap (NPSN, NISN, NIK), bukan UUID acak.

### Akun bawaan

| Peran | Login | Kata sandi | Masuk ke |
|---|---|---|---|
| **Superadmin** | `superadmin` | `Superadmin#2026` | dashboard + aplikasi |
| Kepala Sekolah | `1275010000000001` | `Demo#2026` | dashboard + aplikasi |
| Guru | `1275010000000002` | `Demo#2026` | dashboard + aplikasi |
| Staff TU | `1275010000000003` | `Demo#2026` | dashboard + aplikasi |
| Petugas Pengaduan | `1275020000000001` | `Demo#2026` | dashboard |
| Admin Dinas | `1275020000000002` | `Demo#2026` | dashboard + aplikasi |
| **Siswa** | `0071234501` | `Demo#2026` | aplikasi |
| **Orang Tua** | `1275030000000001` | `Demo#2026` | aplikasi |

Siswa login memakai **NISN** (10 digit); peran lain memakai **NIK** (16
digit). Superadmin juga menerima username karena akun itu dibuat sebelum
identitas Jargon GO ada.

Akun siswa untuk keempat siswa lainnya juga dibuat, dengan pola yang sama:
NISN `0071234502` … `0071234505`.

`must_change_password` disetel `FALSE` untuk akun demo. Di alur sebenarnya
nilainya `TRUE`, tetapi akun uji yang memaksa ganti kata sandi pada login
pertama membuat pengujian berulang menyusahkan.

### Yang bisa dilihat masing-masing

Cakupan data adalah bagian sistem yang paling berkonsekuensi, jadi inilah
yang sebaiknya diuji lebih dulu:

| Login sebagai | Menu yang muncul | Data absensi yang terlihat |
|---|---|---|
| Siswa `0071234501` | Absensi, Panic Button | **hanya dirinya** (Ahmad Fauzi) |
| Orang Tua | Absensi, Panic Button | **hanya anaknya** (Ahmad Fauzi) |
| Guru | Absensi, Panic Button, Pemberkasan | 5 siswa di sekolahnya |
| Petugas Pengaduan | **hanya** Panic Button | — |

Menu ditentukan **server** lewat `available_menus` pada `GET /v1/me/home`,
dihitung dari izin peran. Karena itu perbedaan di tabel atas bukan
tampilan yang disembunyikan di klien — endpoint-nya memang tidak
mengembalikan data yang tidak boleh dilihat.

### Data yang ikut dibuat

* 1 sekolah — SMA Negeri 1 Medan (DEMO), NPSN `10259001`
* 2 kelas — X IPA 1, XI IPA 1 (guru demo menjadi wali kelas X IPA 1)
* 5 siswa dengan NISN yang sah
* 1 aturan jam absensi — tanpa ini, pengenalan wajah menolak semua scan
  dengan "di luar jam absensi", yang terlihat seperti kerusakan
* Riwayat absensi **14 hari** dengan status bervariasi
  (hadir/terlambat/sakit/alfa), Sabtu–Minggu dilewati

Statusnya divariasikan dengan pola tetap berdasarkan tanggal + NISN, bukan
acak: hasilnya sama setiap kali seed dijalankan, sehingga pengujian bisa
diulang dan dibandingkan.

### Yang TIDAK dibuat

**Wajah siswa belum terdaftar** (`face_enrolled = false`), jadi mode kios
akan menolak semua scan dengan "wajah tidak dikenali". Itu benar — data
biometrik tidak bisa dibuat dari SQL, harus melalui pendaftaran wajah
sungguhan lewat tablet atau `/admin/biometric`.

Baris absensi pada seed dibuat dengan `check_in_method = 'face'` seolah
berasal dari tablet, supaya tampilan riwayatnya realistis.

---

## 2.10 Memasang di subpath domain (`/jargon-be`)

Pemasangan yang dipakai saat ini:

```
https://beoulve-dev.biz.id/jargon-be/            aplikasi Jargon GO
https://beoulve-dev.biz.id/jargon-be/admin/      dashboard
https://beoulve-dev.biz.id/jargon-be/v1/...      API
https://beoulve-dev.biz.id/jargon-be/docs        Swagger UI
```

Seluruh sistem berada di **bawah satu awalan path**, bukan di akar domain
— karena domain itu juga melayani hal lain. Aplikasi mobile diarahkan ke
`https://beoulve-dev.biz.id/jargon-be`, dan awalan itu tersimpan sebagai
`AppConfig.productionApiBaseUrl` di `jargon-fe/lib/core/config.dart`.

### Yang perlu disesuaikan

Awalan path bukan sekadar tambahan di URL — tiga hal harus tahu tentangnya,
kalau tidak akan menghasilkan 404 yang membingungkan.

**1. nginx: buang awalannya sebelum meneruskan.**

Container di belakangnya tidak tahu apa-apa soal `/jargon-be`; API
melayani `/v1/...` di akarnya sendiri. Jadi awalan itu dibuang saat
proxy:

```nginx
# --- API + Swagger + berkas -------------------------------------------
location ~ ^/jargon-be/(v1|docs|api-docs|files|health) {
    rewrite ^/jargon-be(/.*)$ $1 break;

    proxy_pass http://absensi_api;
    proxy_http_version 1.1;
    proxy_set_header Host              $host;
    proxy_set_header X-Real-IP         $remote_addr;
    proxy_set_header X-Forwarded-For   $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
    proxy_set_header X-Forwarded-Prefix /jargon-be;
    proxy_set_header Connection        "";
}

# --- Jalur panas absensi: batas waktu KETAT ---------------------------
location ^~ /jargon-be/v1/kiosk/ {
    rewrite ^/jargon-be(/.*)$ $1 break;

    proxy_pass http://absensi_api;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header Connection "";
    proxy_connect_timeout 5s;
    proxy_send_timeout    10s;
    proxy_read_timeout    10s;
    proxy_buffering off;
}

# --- Berkas statis dashboard ------------------------------------------
location ~ ^/jargon-be/admin/(assets|build|storage)/ {
    rewrite ^/jargon-be/admin(/.*)$ $1 break;
    proxy_pass http://absensi_admin;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header Connection "";
    expires 7d;
}

# --- Dashboard --------------------------------------------------------
location ^~ /jargon-be/admin {
    rewrite ^/jargon-be(/.*)$ $1 break;
    proxy_pass http://absensi_admin;
    proxy_http_version 1.1;
    proxy_set_header Host              $host;
    proxy_set_header X-Forwarded-For   $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
    proxy_set_header Connection        "";
    proxy_read_timeout 120s;
}

# --- Aplikasi Jargon GO ----------------------------------------------
location ^~ /jargon-be/ {
    rewrite ^/jargon-be(/.*)$ $1 break;
    proxy_pass http://jargon_web;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header Connection "";
}

location = /jargon-be {
    return 301 /jargon-be/;
}
```

Urutan `location` tidak menentukan di sini — nginx memilih yang paling
spesifik — tetapi `^~` pada blok kios **wajib**: tanpa itu, blok regex
`^/jargon-be/(v1|...)` akan menang dan absensi kehilangan batas waktu
ketatnya.

**2. Flutter web: `--base-href`.**

Bundel web memuat asetnya lewat path absolut. Bila dilayani di subpath
tanpa penyesuaian, ia meminta `/assets/...` di akar domain dan
mendapat 404 — halaman putih tanpa pesan galat apa pun.

```bash
flutter build web --release \
  --base-href=/jargon-be/ \
  --dart-define=API_SAME_ORIGIN=true
```

Garis miring di ujung `--base-href` wajib; Flutter menolak build tanpa itu.

Untuk build container, setel di `.env`:

```ini
WEB_BASE_HREF=/jargon-be/
```

**3. Laravel: `APP_URL` dan `ASSET_URL` ikut awalan.**

```ini
APP_URL=https://beoulve-dev.biz.id/jargon-be
ASSET_URL=/jargon-be/admin
```

`ASSET_URL` tetap perlu bagian `/admin` — itu yang mencegah aset Metronic
bertabrakan dengan aset Flutter (§2.8), dan sekarang ditambah awalan
subpathnya.

### Yang TIDAK perlu disesuaikan

**Aplikasi mobile** tidak butuh apa pun selain alamat yang benar. Ia
menyambung path endpoint ke `baseUrl` sebagai teks, jadi awalan `/jargon-be`
ikut terbawa dengan sendirinya.

**Aplikasi web** juga tidak: `API_SAME_ORIGIN=true` membuatnya mengambil
alamat API dari origin halaman. Namun perhatikan — `Uri.base.origin` hanya
memuat skema+host+port, **tanpa** awalan path. Jadi bila aplikasi web
dilayani di subpath, `API_SAME_ORIGIN` saja tidak cukup dan alamat API
harus diisi eksplisit saat build:

```bash
flutter build web --release \
  --base-href=/jargon-be/ \
  --dart-define=API_BASE_URL=https://beoulve-dev.biz.id/jargon-be
```

Itu mengorbankan sifat "jalan di host mana pun" — tetapi pemasangan di
subpath memang sudah terikat pada satu alamat.

---

## 3. Onboarding satu sekolah

Urutannya penting — melangkahi satu tahap membuat tahap berikutnya gagal.

1. **Buat sekolah** — `/admin/schools` (Superadmin).
2. **Buat tahun ajaran aktif** bila belum ada (tabel `academic_years`;
   sudah terisi `2026/2027` oleh seed).
3. **Buat kelas** — `/admin/classrooms`.
4. **Buat akun sekolah** — `/admin/users`: satu `kepala_sekolah`, satu atau
   lebih `staff_tu` dan `guru`. Peran tingkat sekolah **wajib** ditautkan ke
   `school_id`.
5. **Tetapkan wali kelas** pada tiap kelas.
6. **Masukkan data siswa** beserta **kontak wali murid**. Tanpa kontak wali,
   notifikasi tidak punya tujuan dan sistem akan tampak "tidak mengirim apa-apa".
7. **Atur jam masuk/pulang** — `/admin/attendance-rules`.
8. **Atur kebijakan notifikasi** — `/admin/notifications`.
9. **Daftarkan perangkat** — `/admin/devices`, catat kode pairing 8 digit.
10. **Pasangkan tablet**: buka aplikasi → Pasangkan Perangkat → masukkan kode.
11. **Daftarkan wajah siswa** — tablet bermode `enroll`, atau lewat
    `/admin/biometric` bila model TensorFlow.js sudah dipasang.
12. **Uji satu siswa** sebelum hari pertama pemakaian.

Sampai tahap 12, sekolah sudah bisa berabsensi. Akun aplikasi Jargon GO
(tahap berikutnya) **tidak** menjadi syarat absensi berjalan — sengaja
demikian, supaya sekolah tidak perlu menunggu 700 akun selesai dibuat sebelum
tablet bisa dipakai.

---

## 3.1 Onboarding akun Jargon GO

Setelah absensi berjalan, buka aplikasi untuk warga sekolah lewat
`/admin/app-accounts`.

**Akun guru, staf, dan kepala sekolah** — dibuat satu per satu di
**Buat Akun**. Identitas loginnya **NIK 16 digit**. Akun dashboard yang sudah
ada tidak otomatis bisa login aplikasi: isi `identity_number`-nya lebih dulu.

**Akun siswa** — pakai **Akun Siswa Massal**:

1. Pilih sekolah, lalu **satu kelas** (bukan seluruh sekolah — hasilnya jauh
   lebih mudah dibagikan wali kelas).
2. Halaman menampilkan siswa aktif yang belum punya akun. Siswa dengan NISN
   kosong atau bukan 10 digit ditandai dan **akan dilewati** — perbaiki NISN-nya
   di `/admin/students` lebih dulu.
3. Tekan **Buat Akun**. Kata sandi awal muncul **satu kali**.
4. **Cetak atau unduh CSV sekarang juga.** Kata sandi disimpan sebagai hash;
   tidak ada cara menampilkannya ulang. Bila halaman ditutup, satu-satunya
   jalan adalah mereset per siswa.
5. Bagikan lewat wali kelas. Semua akun berstatus wajib ganti kata sandi saat
   login pertama.

**Akun orang tua** — dibuat satu per satu, karena tautan ke anak harus
diverifikasi orang yang mengenal keluarganya:

1. **Buat Akun** → peran `Orang Tua` → NIK 16 digit.
2. Cari anaknya (nama atau NISN) — pencarian selalu dibatasi cakupan sekolah
   Anda — lalu pilih hubungannya (ayah/ibu/wali).
3. Anak berikutnya ditambahkan dari halaman detail akun, kapan saja. Anak-anak
   di sekolah berbeda boleh ditautkan pada akun yang sama.

Memutus tautan anak **mengakhiri sesi login** akun orang tua tersebut; ia harus
login ulang, dan setelah itu tidak lagi melihat data anak yang dilepas.

**Petugas pengaduan Panic Button** — peran `petugas_pengaduan` bercakupan
provinsi dan hanya bisa dibuat Superadmin. Tunjuk minimal dua orang sebelum
menu Panic Button diumumkan ke siswa: laporan kategori kekerasan dan pelecehan
diteruskan langsung ke Dinas tanpa menunggu moderasi sekolah, dan laporan yang
menumpuk tanpa penanganan lebih merusak kepercayaan daripada tidak ada menunya
sama sekali.

**Jenis dokumen pemberkasan** — periksa `/admin/documents/types`. Seed
menyediakan 15 jenis dokumen untuk enam keperluan; sesuaikan dengan ketentuan
kepegawaian yang berlaku sebelum guru mulai mengunggah, karena daftar itulah
yang menjadi daftar periksa di aplikasi.

---

## 4. Konfigurasi provider notifikasi

### WhatsApp — Fonnte / Wablas (pilot, beberapa sekolah)

```
WA_PROVIDER=fonnte
WA_TOKEN=<token dari dashboard provider>
```

### WhatsApp — Meta Cloud API (skala provinsi)

```
WA_PROVIDER=meta_cloud
WA_TOKEN=<permanent access token>
WA_PHONE_NUMBER_ID=<phone number id>
WA_BASE_URL=https://graph.facebook.com/v21.0
```

Meta mewajibkan **message template** yang disetujui untuk pesan yang diprakarsai
bisnis. Ajukan template yang sepadan dengan `notification_templates` bawaan.

### Telegram (paling murah)

```
TELEGRAM_BOT_TOKEN=<token dari @BotFather>
```

Wali murid harus menekan `/start` pada bot sekolah lebih dulu; `chat_id` yang
diperoleh lalu diisikan pada data wali di `/admin/students`.

### Email

```
SMTP_HOST=smtp.example.go.id
SMTP_PORT=587
SMTP_USERNAME=...
SMTP_PASSWORD=...
SMTP_STARTTLS=true
```

**Saran urutan penggelaran:** mulai dengan Telegram + Email (gratis) untuk
pilot, lalu tambahkan WhatsApp setelah anggaran dan volume nyata diketahui.
WhatsApp adalah kanal yang paling dibaca orang tua di Indonesia, tetapi juga
satu-satunya yang berbiaya per pesan — pada 700.000 siswa, biaya itu perlu
dihitung sebelum diaktifkan menyeluruh.

---

## 5. Operasional harian

### Yang berjalan otomatis

Worker di dalam binary API menangani, tanpa cron eksternal:

| Tugas | Frekuensi | Akibat bila gagal |
|---|---|---|
| Kirim notifikasi outbox | tiap 5 detik | orang tua tidak diberi tahu |
| Tandai siswa alfa | setelah `absent_notify_after` per sekolah | siswa absen tidak punya baris absensi |
| Rollup `attendance_daily_summary` | tiap jam | dashboard melambat |
| Buat partisi bulan berikutnya | tengah malam | INSERT jatuh ke partisi DEFAULT dan melambat |
| Bersih-bersih token & heartbeat | tengah malam | tabel membengkak |

Semua aman dijalankan pada beberapa replika sekaligus (klaim pekerjaan memakai
`FOR UPDATE SKIP LOCKED`, operasi idempotent).

Bila ingin memisahkan replika web dan worker, setel `WORKERS_ENABLED=false`
pada replika yang hanya melayani HTTP.

### Pemantauan

```bash
curl http://localhost:8080/health/live    # proses hidup (tanpa sentuh DB)
curl http://localhost:8080/health/ready   # siap menerima trafik
curl http://localhost:8080/health         # status rinci semua komponen
```

Yang perlu dipantau, dengan urutan prioritas:

1. `/health/ready` gagal → absensi dari seluruh tablet berhenti.
2. `notification_outbox` dengan `status='failed'` menumpuk → orang tua tidak
   menerima kabar; kemungkinan kredensial provider kedaluwarsa.
3. Perangkat dengan `last_seen_at` lebih dari 30 menit pada jam sekolah →
   tablet mati atau jaringan sekolah putus.
4. Baris `attendance_events` dengan `event_type='unknown'` melonjak di satu
   sekolah → banyak siswa belum terdaftar wajahnya, atau pencahayaan di lokasi
   tablet buruk.
5. Sekolah yang belum melapor absensi setelah jam masuk (lihat kartu
   "Pelaporan Hari Ini" pada dashboard Superadmin).
6. **Laporan Panic Button berkategori darurat yang belum ditangani** —
   `/admin/panic` menampilkannya sebagai spanduk merah. Kategori kekerasan dan
   pelecehan diteruskan langsung ke Dinas tanpa menunggu moderasi; laporan
   semacam itu yang menganggur berhari-hari adalah kegagalan yang paling mahal
   di seluruh sistem ini.
7. **Baris baru di `panic_unmask_logs`.** Setiap barisnya berarti anonimitas
   seorang pelapor dibuka. Jadikan ini alert, bukan sekadar laporan bulanan:
   satu pembukaan tanpa dasar cukup untuk menghentikan siswa melapor di
   sekolah itu selamanya.

### Backup

```bash
# Basis data — yang paling penting.
pg_dump -Fc absensi > absensi-$(date +%F).dump

# Foto pendaftaran wajah. Data biometrik: enkripsi arsipnya.
tar czf faces-$(date +%F).tar.gz -C /app/storage faces

# Berkas pemberkasan & media pengaduan. Keduanya memuat data pribadi
# (ijazah, NIK, rekening; foto lokasi kejadian) — enkripsi arsipnya juga.
tar czf documents-$(date +%F).tar.gz -C /app/storage documents
tar czf panic-$(date +%F).tar.gz    -C /app/storage panic
```

Foto pendaftaran tidak dapat dibuat ulang tanpa memanggil 700.000 siswa untuk
difoto lagi. Perlakukan backup-nya setara dengan backup database.

Arsip `panic/` perlu perlakuan khusus: memulihkannya ke lingkungan pengujian
yang aksesnya longgar sama saja dengan membocorkan bukti pengaduan. Bila butuh
data untuk pengujian, gunakan basis data tanpa lampiran.

---

## 6. Penyetelan untuk skala provinsi

### PostgreSQL

```conf
shared_buffers = 8GB                  # ~25% RAM
effective_cache_size = 24GB           # ~75% RAM
work_mem = 64MB
maintenance_work_mem = 2GB
max_locks_per_transaction = 512       # partisi bulanan butuh ini
random_page_cost = 1.1                # untuk SSD/NVMe
max_connections = 300
timezone = 'Asia/Jakarta'
```

Gunakan **PgBouncer** dalam mode transaction bila jumlah replika API besar.

Pertimbangkan **replika baca** untuk dashboard: laporan provinsi tidak boleh
bersaing dengan jalur absensi pada jam 06:30.

### API

Jam puncak: ~1,4 juta request dalam 45 menit ≈ 520 req/detik rata-rata, dengan
puncak sesaat beberapa kali lipat.

* Mulai dengan 4 replika, masing-masing 2 vCPU / 4 GB.
* Skala berdasarkan latensi p99 endpoint `/v1/kiosk/recognize`, bukan CPU
  rata-rata.
* Memori index wajah: 512 dim × 4 byte × 3 sampel ≈ 6 KB per siswa. Satu
  instance yang melayani 200 sekolah × 500 siswa ≈ 600 MB. TTL 5 menit dan
  eviction 6 jam menjaga angka ini stabil.

### Kapasitas penyimpanan

| Data | Estimasi |
|---|---|
| Foto pendaftaran | 700.000 × 3 × 60 KB ≈ **126 GB** |
| `attendances` 1 tahun | ~160 juta baris ≈ **48 GB** dengan index |
| `attendance_events` 1 tahun | ~320 juta baris ≈ **60 GB** |
| `notification_outbox` 1 tahun | ~150 juta baris ≈ **90 GB** |

Partisi bulanan memungkinkan `DETACH PARTITION` untuk mengarsipkan data lama
ke penyimpanan murah tanpa mengganggu operasional.

---

## 7. Menyetel ambang pengenalan

Jangan mengubah `FACE_MATCH_THRESHOLD` berdasarkan tebakan. Prosedurnya:

1. Kumpulkan data dari lapangan:

```sql
-- Sebaran skor kemiripan pada scan yang diterima, per sekolah.
SELECT school_id,
       COUNT(*)                                    AS total,
       ROUND(AVG(similarity)::numeric, 3)          AS rata2,
       ROUND(MIN(similarity)::numeric, 3)          AS terendah,
       COUNT(*) FILTER (WHERE reason = 'below_threshold')  AS ditolak_ambang,
       COUNT(*) FILTER (WHERE reason = 'ambiguous_match')  AS ambigu
FROM attendance_events
WHERE occurred_at > NOW() - INTERVAL '7 days'
  AND similarity IS NOT NULL
GROUP BY school_id
ORDER BY ditolak_ambang DESC;
```

2. Banyak `ditolak_ambang` **dan** siswa mengeluh gagal absen → periksa dulu
   kualitas foto pendaftaran (`face_enrollments.quality_score`). Foto buruk
   lebih sering menjadi penyebabnya daripada ambang yang terlalu tinggi.
3. Turunkan ambang hanya lewat kolom `schools.face_match_threshold` untuk
   sekolah tertentu, jangan global. Batas aman: 0,55.
4. Bila `ambigu` tinggi, biasanya ada saudara kembar. Itu **bukan** bug —
   siswa tersebut sebaiknya absen manual ke petugas.

---

## 8. Pemecahan masalah

| Gejala | Penyebab yang paling sering | Tindakan |
|---|---|---|
| `type "vector" does not exist` | pgvector tidak terpasang | `CREATE EXTENSION vector;` atau pakai image `pgvector/pgvector` |
| Tablet: "Versi model berbeda" | `--dart-define` tidak sama dengan `.env` API | selaraskan `FACE_MODEL_VERSION` |
| Tablet: "token tidak dikenal" | token dicabut atau perangkat dihapus | buat kode pairing baru di `/admin/devices` |
| Semua wajah "tidak dikenali" | belum ada siswa terdaftar di sekolah itu | cek `/admin/biometric`, cakupan pendaftaran |
| Dashboard: "Sesi API tidak tersedia" | API mati saat login, atau kredensial salah | periksa `jargon_api_*`, lalu logout-login |
| Notifikasi berstatus `failed` | token provider kedaluwarsa | cek `last_error` di `/admin/notifications/outbox` |
| Absensi lambat pada jam 06:30 | partisi bulan berjalan belum ada | `SELECT ensure_attendance_partitions(3);` |
| `php artisan migrate` error | migrasi Laravel dijalankan padahal skema milik sqlx | jangan jalankan; lihat §2 |
| Dashboard: komponen `input-label` tidak ada | `auth/register.blade.php` memakai komponen yang belum dibuat (bawaan starter, tidak dipakai alur ini) | hapus berkas itu atau buat komponennya bila registrasi publik memang diperlukan |
| Aplikasi: "NISN tidak terdaftar" padahal siswanya ada | data siswa ada, tetapi **akunnya** belum dibuat | `/admin/app-accounts/bulk` untuk kelas tersebut |
| Aplikasi: login gagal untuk guru yang bisa masuk dashboard | akun dashboard lama belum punya `identity_number` | isi NIK-nya di `/admin/app-accounts` |
| Akun siswa massal melewati banyak siswa | NISN kosong atau bukan 10 digit | perbaiki di `/admin/students`, lalu ulangi — yang sudah punya akun otomatis dilewati |
| Orang tua tidak melihat data anaknya | tautan `student_guardians.user_id` belum dibuat | tambahkan anak dari halaman detail akun |
| Orang tua masih melihat anak yang sudah dilepas | sesi lama masih memegang token berisi daftar anak | seharusnya otomatis dicabut; bila berulang, periksa `refresh_tokens` akun itu |
| Menu tertentu tidak muncul di aplikasi | menu ditentukan izin peran, dihitung server | periksa izin peran di `/admin/roles`; tidak perlu rilis aplikasi baru |
| Pengaduan baru tidak tampil di beranda aplikasi | menunggu moderasi (`panic_pre_moderation=1`) | setujui di `/admin/panic`, atau nonaktifkan pra-moderasi di `/admin/settings` |
| Kepala sekolah tidak melihat sebagian laporan | **disengaja** — kategori kekerasan/pelecehan/pungli disembunyikan darinya | tangani lewat peran `petugas_pengaduan` atau `admin_dinas` |
| Guru tidak bisa mengganti berkas | pengajuan terkunci setelah berstatus `diajukan` | verifikator mengembalikannya ke `revisi` |

---

## 9. Daftar periksa sebelum produksi

**Keamanan**

- [ ] `JWT_SECRET` acak minimal 32 karakter, berbeda dari staging
- [ ] `SECRETS_KEY_HEX` terisi (32 byte hex)
- [ ] Kata sandi `superadmin` bawaan sudah diganti
- [ ] `CORS_ALLOWED_ORIGINS` bukan `*`
- [ ] `ENABLE_SWAGGER=false` bila dokumentasi tidak perlu publik
- [ ] HTTPS aktif — aplikasi Flutter release menolak HTTP tanpa TLS
- [ ] APK ditandatangani dengan keystore produksi, bukan kunci debug
      (`android/app/build.gradle.kts`)
- [ ] Redis aktif (tanpanya rate limit & anti-replay nonce mati)

**Data**

- [ ] pgvector terpasang, `SELECT ensure_attendance_partitions(6);` dijalankan
- [ ] `SELECT ensure_panic_partitions(6);` dijalankan
- [ ] Backup otomatis database **dan** direktori `faces/`, `documents/`, `panic/`
- [ ] Backup sudah pernah diuji dipulihkan

**Panic Button**

- [ ] Izin `unmask_panic_report` hanya dimiliki akun yang benar-benar perlu —
      periksa `/admin/roles`, jangan diwariskan ke peran sekolah
- [ ] Minimal dua `petugas_pengaduan` ditunjuk dan tahu cara memakainya
- [ ] Alarm untuk baris baru di `panic_unmask_logs`
- [ ] Akun database aplikasi bukan superuser — anonimitas berhenti pada akses
      langsung ke tabel (lihat ARCHITECTURE.md §14 butir 7)

**Operasional**

- [ ] `/health/ready` terhubung ke load balancer
- [ ] Alarm untuk `notification_outbox` gagal menumpuk
- [ ] Alarm untuk perangkat offline pada jam sekolah
- [ ] Alarm untuk laporan darurat yang belum ditangani lebih dari 24 jam
- [ ] Zona waktu `Asia/Jakarta` konsisten di API, PostgreSQL, dan Laravel
