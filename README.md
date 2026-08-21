# jargon-be — Backend Jargon GO

Backend Super Apps **Dinas Pendidikan Provinsi Sumatera Utara** (700.000+
siswa, ribuan sekolah): absensi pengenalan wajah, Panic Button (pengaduan
anonim), dan pemberkasan kepegawaian.

```
jargon-be/
├── api/      API Rust (Axum) + OpenAPI/Swagger + worker latar
├── admin/    Dashboard /admin (Laravel 12 + Octane + Redis)
├── infra/    docker-compose, nginx, init PostgreSQL
└── docs/     ARCHITECTURE.md, DEPLOYMENT.md, UPGRADE.md
```

## Mulai cepat

```bash
export JWT_SECRET=$(openssl rand -base64 48)
export SECRETS_KEY_HEX=$(openssl rand -hex 32)
docker compose -f infra/docker-compose.yml up -d
```

| | Alamat |
|---|---|
| Dashboard | <http://localhost/admin/login> |
| Swagger UI | <http://localhost/docs> |
| Spesifikasi OpenAPI | <http://localhost/api-docs/openapi.json> |

Login pertama: `superadmin` / `Superadmin#2026` — **ganti segera**.

Pemasangan manual, onboarding sekolah, dan penyetelan produksi:
[`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md).

Menaikkan versi server yang **sudah berjalan** — bukan memasang dari nol:
[`docs/UPGRADE.md`](docs/UPGRADE.md). Dipisah karena pembaruan punya
bahaya yang tidak ada pada pemasangan baru: ada data yang bisa hilang.

## Dua service, satu database

| | `api/` (Rust) | `admin/` (Laravel Octane) |
|---|---|---|
| Melayani | tablet kios, aplikasi mobile, dashboard | pengguna dashboard |
| Beban | ~520 req/detik pada 06:30–07:15 | puluhan pengguna, sepanjang hari |
| Tanggung jawab | pencocokan wajah, aturan absensi, outbox notifikasi | CRUD, laporan, RBAC, jejak audit |

Dashboard **membaca** PostgreSQL langsung, tetapi **menulis** melalui API untuk
semua hal yang punya aturan domain. Dengan begitu perhitungan keterlambatan,
pemilihan template notifikasi, dan invalidasi cache wajah hanya ada di satu
tempat — angka pada laporan tidak akan berbeda hanya karena absensi berasal
dari tablet alih-alih dari koreksi manual guru.

Alasan lengkap pemilihan ini (termasuk mengapa `/admin` **tidak** ditulis ulang
dalam Rust): [`docs/ARCHITECTURE.md` §2](docs/ARCHITECTURE.md).

## Skema database

**Migrasi sqlx di `api/migrations/` adalah satu-satunya sumber kebenaran
skema.** Migrasi Laravel sudah ditandai selesai oleh `0010_seed.sql`, sehingga
`php artisan migrate` menjadi no-op yang aman — jangan menjalankannya sebagai
bagian dari alur pemasangan.

```
0001_extensions              pgvector, pgcrypto, pg_trgm, UUID v7
0002_identity_and_laravel_compat  users + tabel infrastruktur Laravel
0003_tenancy                 regions, schools, academic_years, cakupan user
0004_academics               classrooms, students, student_guardians
0005_face                    face_enrollments, face_embeddings (vector 512)
0006_devices                 devices, heartbeats, api_clients
0007_attendance              aturan jam, attendances & events (partisi bulanan)
0008_notifications           templates, policies, outbox (partisi bulanan)
0009_ops                     audit, import, export, idempotency
0010_seed                    roles, permissions, superadmin, 34 wilayah Sumut
0011_superapp_identity       login NIK/NISN, akun siswa & orang tua, izin baru
0012_panic_button            kategori, laporan (partisi), media, unmask logs
0013_pemberkasan             jenis dokumen, pengajuan, berkas, lini masa
```

Tiga migrasi terakhir menambahkan Jargon GO di atas skema absensi yang sudah
ada. Yang perlu diperhatikan pada `0012`: kolom `panic_reports.author_user_id`
adalah **rahasia** — VIEW `panic_reports_feed` sengaja tidak memuatnya, dan
satu-satunya jalan resmi membukanya menulis `panic_unmask_logs` lebih dulu.

## Perintah yang sering dipakai

```bash
# API
cd api
cargo run                     # migrasi jalan otomatis saat start
cargo test                    # 204 pengujian unit
cargo clippy --all-targets

# Dashboard
cd admin
php artisan octane:start --host=0.0.0.0 --port=8000
php artisan view:clear && php artisan config:clear
```

## Privasi data biometrik

| | Gambar wajah | Vektor embedding |
|---|---|---|
| Pendaftaran awal | disimpan | disimpan |
| Absensi harian | **tidak dikirim** | dikirim, dipakai, **dibuang** |
| Log scan | — | hanya SHA-256-nya (anti-replay) |

Tabel `attendances` dan `attendance_events` tidak memiliki kolom untuk gambar
maupun vektor. Menghapus siswa memusnahkan seluruh data biometriknya secara
permanen; riwayat absensi tetap disimpan sebagai dokumen administrasi.

## Anonimitas Panic Button

Identitas pelapor tidak pernah muncul di halaman mana pun. Tiga hal yang
sebaiknya tidak diubah tanpa membaca
[`docs/ARCHITECTURE.md` §11](docs/ARCHITECTURE.md) lebih dulu:

1. `PanicReport` tidak punya relasi `author()`. Bukan kelupaan — begitu relasi
   itu ada, cepat atau lambat ada Blade yang menampilkannya.
2. Kategori `kekerasan`, `pelecehan`, dan `pungli` tidak terlihat kepala
   sekolah; pada laporan semacam itu ia bisa jadi pihak yang diadukan.
3. Pembukaan identitas hanya lewat API, karena API menulis
   `panic_unmask_logs` sebelum mengembalikan datanya. Membaca kolomnya langsung
   dari dashboard akan melewati pencatatan itu.

## Akun aplikasi

Pendaftaran akun Jargon GO dilakukan admin di `/admin/app-accounts` — bukan
swalayan. Siswa login memakai **NISN** (10 digit), peran lain memakai **NIK**
(16 digit). Untuk sekolah besar tersedia pembuatan akun siswa massal per kelas;
kata sandi awalnya ditampilkan **sekali** dan harus dicetak atau diunduh saat
itu juga.

Aplikasi Jargon GO & tablet kios: [`../jargon-fe`](../jargon-fe).
