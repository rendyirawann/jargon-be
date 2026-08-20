# Arsitektur Jargon GO

Super Apps Dinas Pendidikan Provinsi Sumatera Utara — 700.000+ siswa, ribuan
sekolah. Tiga layanan di atas satu backend: **absensi** pengenalan wajah,
**Panic Button** (pengaduan anonim), dan **Pemberkasan** kepegawaian.

---

## 1. Gambaran umum

```
      ┌───────────────────┐        ┌───────────────────┐
      │  Tablet di        │        │  Jargon GO        │
      │  gerbang / kelas  │        │  siswa, ortu,     │
      │  (jargon-fe)      │        │  guru (jargon-fe) │
      └────────┬──────────┘        └────────┬──────────┘
               │ Device token               │ Bearer JWT
               │ embedding 512-d            │ (login NIK/NISN)
               ▼                            ▼
      ┌───────────────────────────────────────────────┐
      │        API  —  Rust / Axum  (jargon-be/api)   │
      │                                               │
      │  • pencocokan wajah (index per-sekolah)       │
      │  • aturan jam masuk/pulang                    │
      │  • anonimitas Panic Button + audit unmask     │
      │  • pemberkasan kepegawaian                    │
      │  • outbox notifikasi transaksional            │
      │  • OpenAPI 3.1 + Swagger UI di /docs          │
      └───────┬───────────────────────────────┬───────┘
              │                               │
              │  SQL                          │  server-to-server
              ▼                               │  (X-Api-Key)
      ┌────────────────────────┐              │
      │  PostgreSQL 17         │◀─────────────┼──── SQL (baca)
      │  + pgvector            │              │
      │  + partisi bulanan     │              │
      └────────────────────────┘              │
                                     ┌────────┴──────────────────┐
                                     │  Dashboard /admin         │
                                     │  Laravel 12 + Octane      │
                                     │  (jargon-be/admin)       │
                                     └───────────────────────────┘
                                                │
                                     ┌──────────┴────────────┐
                                     │  Redis                │
                                     │  session, cache,      │
                                     │  rate limit, nonce    │
                                     └───────────────────────┘
```

---

## 2. Mengapa Rust untuk API dan Laravel untuk dashboard

Pertanyaan yang diminta dipertimbangkan: apakah `/admin` sebaiknya juga Rust?

**Rekomendasi: tidak. Pertahankan Laravel Octane untuk `/admin`, Rust untuk API.**

Alasannya bukan preferensi bahasa, melainkan bentuk bebannya:

| | Jalur absensi (API) | Dashboard (/admin) |
|---|---|---|
| Pola beban | ~1,4 juta request terkonsentrasi di 45 menit setiap pagi | puluhan pengguna bersamaan, sepanjang hari |
| Operasi dominan | perkalian vektor 512-d, ribuan kali per detik | SELECT dengan filter + render HTML |
| Yang menentukan biaya | CPU dan latensi p99 | kecepatan pengembangan fitur |
| Konsekuensi lambat | siswa menumpuk di gerbang | halaman terasa berat |

Untuk kolom kiri, Rust memberi keunggulan yang nyata: pencocokan wajah
dilakukan di memori proses tanpa GC pause, satu instance mampu melayani
ribuan tablet, dan konsumsi memori dapat diprediksi.

Untuk kolom kanan, keunggulan itu tidak terpakai. Yang menentukan justru
kecepatan membangun 20+ layar CRUD, RBAC, jejak audit, manajemen pengguna,
dan ekspor laporan — dan **semua itu sudah ada** pada starter Laravel yang
Anda punya (Spatie Permission, Activitylog, DataTables, ban, session login).
Menulis ulang semuanya di Rust berarti mengorbankan pekerjaan yang sudah jadi
untuk keuntungan yang tidak terasa oleh pengguna dashboard.

**Yang membuat kombinasi ini tetap aman adalah pembagian tanggung jawab yang
tegas:**

* Dashboard **membaca** PostgreSQL langsung (lebih cepat, dan DataTables
  server-side memang butuh SQL).
* Dashboard **menulis** melalui API untuk semua hal yang punya aturan domain:
  pendaftaran wajah, koreksi absensi, pairing perangkat, kirim notifikasi.

Konsekuensinya: perhitungan menit keterlambatan, pemilihan template
notifikasi, invalidasi cache index wajah, dan penulisan outbox transaksional
hanya ada di **satu** tempat. Tidak mungkin angka di laporan berbeda hanya
karena absensi berasal dari tablet alih-alih dari koreksi manual guru.

Kapan pindah ke Rust penuh? Bila dashboard mulai menjadi hambatan nyata —
misalnya laporan provinsi yang harus mengagregasi lintas ribuan sekolah
secara real time. Saat itu tiba, endpoint yang bermasalah dipindahkan satu
per satu ke API, bukan seluruh dashboard sekaligus.

---

## 3. Multi-tenancy: satu sekolah = satu tenant

Ini kebutuhan paling berkonsekuensi dalam sistem ini. Guru di SMA N 2 Binjai
tidak boleh bisa melihat data siswa di sekolah lain, walaupun ia menebak UUID.

Penegakannya berlapis:

1. **Kolom `school_id` di setiap tabel domain** — filter tenant selalu bisa
   didorong ke index.
2. **API**: satu fungsi, [`AuthUser::resolve_school`](../api/src/auth/mod.rs),
   yang dilewati setiap endpoint. Peran tingkat sekolah yang menyebut
   `school_id` lain menerima `403`; peran provinsi (`superadmin`,
   `admin_dinas`) boleh memilih atau tidak memfilter.
3. **Dashboard**: `TenantScope` sebagai **global scope** Eloquent. Ini
   disengaja — mengandalkan setiap pengembang untuk ingat menulis
   `->where('school_id', ...)` pasti gagal cepat atau lambat. Dengan global
   scope, yang harus eksplisit justru sebaliknya: `withoutTenantScope()`.
4. **Akun tanpa sekolah gagal aman** — melihat nol baris, bukan semua baris.

Peran yang tersedia:

| Peran | Cakupan | Kemampuan utama |
|---|---|---|
| `superadmin` | provinsi | semuanya, termasuk kelola sekolah & pengguna |
| `admin_dinas` | provinsi | pantau & ekspor, tanpa hak hapus |
| `petugas_pengaduan` | provinsi | moderasi & penanganan Panic Button |
| `kepala_sekolah` | 1 sekolah | pantau, setujui, koreksi absensi, atur jam, verifikasi berkas |
| `guru` | 1 sekolah | pantau, daftarkan wajah, koreksi, kirim notifikasi, unggah berkas |
| `staff_tu` | 1 sekolah | operator data siswa, kelas, dan perangkat |
| `siswa` | dirinya sendiri | baca absensinya sendiri, kirim pengaduan |
| `orang_tua` | anak-anaknya | baca absensi anaknya, kirim pengaduan |

Absensi tetap **tidak memerlukan akun siswa**: identitas operasional siswa
adalah wajahnya di tablet. Akun `siswa` hanya untuk MEMBACA datanya sendiri di
aplikasi — tidak ada endpoint yang mengizinkan akun itu mencatat kehadiran.

### 3.1 Cakupan kedua: siswa, bukan sekolah

Dua peran tidak bisa dijelaskan oleh `school_id`:

* **`siswa`** — cakupannya satu baris, bukan satu sekolah. Seorang siswa tidak
  boleh melihat absensi teman sekelasnya.
* **`orang_tua`** — anak-anaknya bisa bersekolah di tempat berbeda. Mengikat
  akun ke satu `school_id` akan salah dua arah sekaligus: kehilangan akses ke
  anak di sekolah lain, sekaligus memberi akses ke **seluruh** siswa di sekolah
  yang terikat.

Karena itu `AuthUser` punya dua dimensi cakupan yang terpisah:

| Fungsi | Menjawab |
|---|---|
| [`resolve_school`](../api/src/auth/mod.rs) | sekolah mana yang boleh dilihat |
| [`resolve_students`](../api/src/auth/mod.rs) | siswa mana yang boleh dilihat |

Cakupan orang tua diturunkan dari baris `student_guardians.user_id`, bukan dari
kolom sekolah — dan karena daftar itu ikut tertanam di access token, memutus
tautan seorang anak juga **mencabut sesi login** akun tersebut. Tanpa itu,
orang tua yang sudah dilepas masih bisa membaca data anak itu sampai tokennya
kedaluwarsa.

---

## 4. Alur pengenalan wajah

### 4.1 Pembagian kerja perangkat ↔ server

```
TABLET (Flutter)                          SERVER (Rust)
────────────────                          ──────────────
deteksi wajah (ML Kit)
liveness pasif (kedip + gerak)
crop + align 112×112
ekstraksi embedding (TFLite)
L2-normalize
     │
     ├── PENDAFTARAN ──> GAMBAR + embedding ──> simpan keduanya
     │                                          (gambar ke storage,
     │                                           vektor ke pgvector)
     │
     └── ABSEN HARIAN ─> HANYA embedding ─────> cocokkan, catat absensi,
                                                BUANG embedding-nya
```

Embedding diekstraksi di perangkat, bukan di server. Satu request absensi
membawa ~8 KB (512 float sebagai JSON) alih-alih ~200 KB gambar. Untuk ribuan
sekolah yang banyak di antaranya berjaringan lambat, ini perbedaan antara
"instan" dan "tidak bisa dipakai".

Harganya: versi model di tablet harus sama dengan versi pada embedding
tersimpan. Vektor dari model berbeda **tidak sebanding** — mencocokkannya
menghasilkan identifikasi acak, bukan sekadar akurasi lebih rendah. Karena itu
`model_version` disertakan pada setiap request dan divalidasi server.

### 4.2 Mengapa pencocokan tidak memakai kNN pgvector

Populasi total 700.000+, tetapi pencocokan **selalu** dibatasi satu sekolah:
tablet di SMA N 1 Medan tidak pernah perlu membandingkan wajah dengan siswa di
Nias. Satu sekolah rata-rata hanya ratusan sampai ~2.000 embedding.

Untuk ukuran itu, brute-force **eksak** di memori (dot product 512-d,
autovectorized) memakan waktu di bawah satu milidetik — lebih cepat *dan*
lebih akurat daripada ANN melalui jaringan ke database. Implementasinya di
[`face/index.rs`](../api/src/face/index.rs): `DashMap<school_id, SchoolSlice>`
dengan buffer f32 datar yang ramah cache CPU, TTL 5 menit, dan invalidasi
langsung setiap kali ada pendaftaran wajah baru.

pgvector + index HNSW tetap ada sebagai sumber kebenaran dan untuk memuat
cache.

### 4.3 Urutan pemeriksaan pada satu scan

Disusun dari termurah ke termahal, agar payload yang jelas tidak valid ditolak
sebelum menyentuh database:

1. versi model cocok? (banding string)
2. dimensi & isi vektor waras? (memori)
3. liveness lolos? (banding float)
4. jam perangkat masuk akal? (aritmetika — anti replay)
5. nonce belum terpakai? (Redis, 1 RTT)
6. hari sekolah, bukan libur? (Postgres)
7. cocokkan wajah (memori)
8. cooldown & anti-replay embedding (Postgres)
9. tulis absensi + outbox notifikasi (satu transaksi)

Ambang penerimaan ada **dua**, bukan satu:

* `FACE_MATCH_THRESHOLD` (0,62) — skor minimum.
* `FACE_MATCH_MARGIN` (0,04) — selisih minimum terhadap siswa lain terdekat.

Yang kedua melindungi kasus saudara kembar. Skor tinggi tetapi ambigu
**ditolak** dengan pesan "silakan absen manual ke petugas" — mencatat
kehadiran orang yang salah jauh lebih merugikan daripada meminta scan ulang.

---

## 5. Privasi data biometrik

Aturan yang ditegakkan oleh skema, bukan sekadar konvensi:

| | Gambar wajah | Vektor embedding |
|---|---|---|
| Pendaftaran | disimpan (storage) | disimpan (pgvector) |
| Absensi harian | **tidak dikirim** | dikirim, dipakai, **dibuang** |
| Log scan | — | hanya SHA-256-nya (anti-replay) |

Tabel `attendances` dan `attendance_events` **tidak punya kolom** untuk gambar
atau vektor. Isi satu baris absensi: id & nama siswa, id & nama kelas, id &
nama sekolah, jam masuk, jam pulang, status.

Gambar pendaftaran diarsipkan bukan karena kebiasaan, melainkan karena
diperlukan untuk menghitung ulang embedding saat model di-upgrade. Tanpa itu,
700.000 siswa harus difoto ulang satu per satu.

Menghapus siswa memusnahkan seluruh gambar dan vektornya secara permanen.
Riwayat absensi tetap disimpan sebagai dokumen administrasi.

Foto pendaftaran tidak dilayani langsung oleh web server: setiap permintaan
melewati `/files/*` di API yang memverifikasi hak akses terhadap sekolah siswa
tersebut. "URL yang sulit ditebak" bukan kontrol akses.

---

## 6. Skala: 700.000 siswa

| Tabel | Volume/tahun | Strategi |
|---|---|---|
| `attendances` | ~160 juta baris | partisi RANGE per bulan pada `attendance_date` |
| `attendance_events` | ~320 juta baris | partisi RANGE per bulan pada `occurred_at` |
| `notification_outbox` | ~150 juta baris | partisi RANGE per bulan + arsip |
| `face_embeddings` | ~2,1 juta baris | index HNSW + cache per sekolah di proses |

Konsekuensi praktis yang harus diingat setiap pengembang: **setiap query ke
`attendances` wajib membawa filter tanggal.** Tanpa itu, PostgreSQL memindai
seluruh riwayat provinsi. Di Eloquent, gunakan scope `onDate()` atau
`betweenDates()`; di Rust, klausa `attendance_date BETWEEN` selalu ada.

Angka dashboard tidak dihitung dari tabel mentah setiap kali halaman dibuka.
Worker `maintenance` mengisi `attendance_daily_summary` setiap jam, dan juga:

* membuat partisi bulan berikutnya sebelum dibutuhkan;
* menandai siswa yang tidak pernah discan sebagai `alfa` setelah jam
  `absent_notify_after` tiap sekolah — tanpa langkah ini, siswa yang tidak
  masuk tidak punya baris absensi sama sekali dan orang tuanya tidak pernah
  diberi tahu;
* membersihkan token kedaluwarsa, heartbeat lama, dan kode pairing hangus.

---

## 7. Notifikasi ke wali murid

Pola **transactional outbox**:

```
[ transaksi database ]
    catat absensi           ──┐
    masukkan pesan ke outbox ──┴── commit bersama
[ /transaksi ]

[ worker terpisah ]
    ambil batch (FOR UPDATE SKIP LOCKED)
    kirim ke provider
    tandai sent / jadwalkan ulang dengan backoff eksponensial
```

Kalau transaksi batal, pesan tidak pernah ada. Kalau transaksi sukses, pesan
**pasti** tercatat. Dan yang paling penting: provider WhatsApp yang sedang
down hanya memperlambat notifikasi — tidak pernah membuat absensi siswa gagal
tercatat.

Tiga kanal, dipilih per wali murid:

| Kanal | Provider | Catatan |
|---|---|---|
| WhatsApp | `fonnte`, `wablas`, `meta_cloud` | provider lokal untuk pilot; Meta Cloud API untuk skala provinsi |
| Telegram | Bot API | gratis; wali harus menekan `/start` lebih dulu |
| Email | SMTP | paling murah untuk volume besar, tapi paling jarang dibaca |

Nomor tujuan **disamarkan** pada semua respons API dan tampilan dashboard
(`6281*****789`), agar daftar log tidak menjadi sumber ekspor nomor telepon
orang tua.

---

## 8. Keamanan perangkat

Tablet dipasang di ruang publik. Karena itu:

* **Tidak memakai akun guru.** Tablet punya identitas sendiri: device token
  (256-bit acak) + kunci HMAC, diterbitkan sekali saat pairing.
* **Server hanya menyimpan SHA-256 token**, bukan tokennya.
* **Pairing lewat kode 8 digit** yang berlaku 30 menit, sekali pakai,
  dikonsumsi dalam satu `UPDATE` (dua tablet dengan kode sama: hanya satu
  berhasil), dan dilindungi rate limit.
* **Pencabutan berlaku seketika** — cache perangkat dibuang, bukan menunggu
  TTL.
* **Kredensial di perangkat** masuk Keystore/Keychain, bukan SharedPreferences.
* **Anti-replay tiga lapis**: nonce sekali pakai (Redis), jendela toleransi
  jam perangkat (±120 detik), dan hash embedding — embedding wajah asli selalu
  sedikit berbeda tiap frame, jadi hash yang sama persis berarti payload lama
  diputar ulang.

### Catatan jujur tentang liveness

Aplikasi menerapkan liveness **pasif** berbasis isyarat perilaku: kedip mata
(dari `leftEyeOpenProbability` ML Kit) dan gerak kepala mikro. Ini
menghentikan penyalahgunaan yang paling umum di lapangan — mengarahkan foto
cetak atau layar ponsel ke kamera.

Ini **bukan** anti-spoof kelas tinggi. Serangan dengan video wajah bergerak
atau masker cetak 3D masih mungkin. Bila diperlukan jaminan lebih kuat,
tambahkan model anti-spoof khusus atau kamera dengan sensor kedalaman;
titik sisipannya sudah disiapkan di `FaceEngine.analyze()`.

---

## 9. Ketahanan jaringan

Banyak sekolah berada di daerah dengan jaringan tidak stabil. Kalau tablet
menolak absen saat jaringan mati, siswa yang sudah datang tercatat alfa dan
orang tuanya menerima notifikasi yang salah — kegagalan yang lebih buruk
daripada tidak ada sistem.

Karena itu tablet punya antrean lokal (SQLite):

* Pemindaian yang gagal terkirim disimpan, dan siswa diberi tahu "absensi
  tersimpan, akan dikirim otomatis".
* Hanya vektor + waktu tangkap yang disimpan — **tidak ada gambar**. Baris
  dihapus segera setelah server menerimanya.
* Umur maksimum 18 jam: absensi yang tertahan lebih lama tidak lagi bisa
  dipertanggungjawabkan sebagai jam kedatangan, jadi lebih baik dibuang dan
  dikoreksi manual. Jumlah yang dibuang **dilaporkan** ke operator, tidak
  hilang diam-diam.
* Batas 5.000 baris agar penyimpanan tablet tidak habis.

Penolakan yang bersifat aturan (di luar jam, wajah tak dikenal) **tidak**
masuk antrean — mengirim ulangnya tidak akan mengubah apa pun.

---

## 10. Identitas dan akun aplikasi

### 10.1 Satu kolom untuk NIK dan NISN

Login Jargon GO memakai **NISN** (10 digit) untuk siswa dan **NIK** (16 digit)
untuk semua peran lain. Keduanya ditampung satu kolom, `users.identity_number`,
dengan `identity_type` sebagai penandanya.

Alasannya praktis: layar login hanya punya satu kotak isian, dan pencarian akun
saat login harus mengenai **satu** index tunggal. Dua kolom yang di-OR akan
memaksa PostgreSQL memilih antara dua index parsial pada jalur yang dipanggil
setiap kali seseorang membuka aplikasi.

Panjangnya ditegakkan `CHECK` di database, bukan hanya di aplikasi:

```sql
CHECK (identity_number IS NULL
       OR (identity_type = 'nik'  AND identity_number ~ '^[0-9]{16}$')
       OR (identity_type = 'nisn' AND identity_number ~ '^[0-9]{10}$'))
```

Data pokok kependudukan yang salah panjang akan menyulitkan integrasi dengan
sistem Provsu lain nanti — lebih murah ditolak sekarang daripada dibersihkan
setelah 700.000 baris masuk.

### 10.2 Pendaftaran hanya lewat admin

Tidak ada pendaftaran mandiri. Yang boleh melihat absensi seorang siswa hanya
siswa itu dan orang tuanya, dan **tidak ada cara memverifikasi hubungan
orang tua–anak dari sebuah formulir**. Siapa pun bisa mengetik NISN anak orang
lain. Verifikasinya harus dilakukan pihak yang mengenal keluarganya — sekolah —
lewat `/admin/app-accounts`.

Konsekuensi yang diterima: onboarding menjadi pekerjaan operator sekolah. Itu
diringankan dua cara:

* **Akun siswa massal** (`POST /v1/users/students/bulk`) membuat akun untuk
  seluruh siswa aktif satu kelas sekaligus, dan mengembalikan kata sandi awal
  **sekali**. Nilai itu tidak pernah bisa dilihat lagi — halaman `/admin`-nya
  menyediakan cetak dan unduh CSV pada tampilan yang sama.
* Kata sandi awal dibuat **acak**, bukan diturunkan dari NISN atau tanggal
  lahir. Keduanya tercetak di dokumen sekolah dan diketahui teman sekelas;
  memakainya berarti setiap siswa bisa masuk ke akun temannya pada hari
  pertama.

Semua akun baru berstatus `must_change_password = TRUE`.

---

## 11. Panic Button: anonimitas sebagai persyaratan, bukan fitur

Menu ini ada supaya siswa berani melaporkan pungli dan perundungan. Bila
identitas pelapor bisa bocor — sekali saja, ke satu orang yang salah — menu ini
berhenti dipakai dan tidak akan dipercaya lagi. Karena itu anonimitas
diperlakukan sebagai persyaratan yang membentuk skema, bukan tampilan yang
dipasang belakangan.

### 11.1 Yang disimpan dan yang tidak

`panic_reports.author_user_id` **ada** — laporan tanpa penulis berarti tidak
ada cara menindaklanjuti laporan palsu atau menghubungi pelapor lewat balasan
resmi. Yang dijaga adalah aksesnya:

| Lapis | Penjagaan |
|---|---|
| Database | VIEW `panic_reports_feed` sengaja **tidak memuat** kolom itu |
| API | tidak ada endpoint yang mengembalikannya kecuali `POST /v1/panic/reports/{id}/unmask` |
| Dashboard | `PanicReport::$hidden` memuatnya; relasi `author()` **sengaja tidak didefinisikan** |

Relasi yang tidak ada adalah penjagaan yang paling sering terlupakan: begitu
`$report->author` bisa ditulis, cepat atau lambat ada Blade yang menampilkannya.

### 11.2 Handel, bukan nama

| Tempat | Bentuk | Sifat |
|---|---|---|
| Judul laporan | handel acak per laporan | dua laporan dari orang yang sama **tidak dapat dihubungkan** |
| Komentar dalam satu utas | HMAC-SHA256(rahasia, report_id, user_id) | stabil di dalam utas, tidak dapat dicocokkan lintas utas |

Handel komentar harus stabil agar percakapan bisa diikuti ("Anonim-3 menjawab
Anonim-1"), tetapi tidak boleh sama di laporan lain — kalau sama, cukup satu
laporan yang identitasnya terbuka untuk membuka seluruh riwayat orang itu.
Rahasia HMAC-nya adalah kunci server, sehingga tabel yang bocor pun tidak
membalik handel menjadi `user_id`.

### 11.3 Beranda provinsi, nama sekolah dikaburkan

Beranda **tidak** dipisah per sekolah. Di sekolah dengan 90 siswa, laporan
"kelas XI IPA 2, uang seragam" akan menyisakan sedikit sekali kemungkinan
pelapor. Beranda provinsi dengan nama sekolah dikaburkan
([`blur_school_name`](../api/src/services/anonymity.rs)) membuat kumpulan
tersangka menjadi seluruh provinsi.

Sisi buruknya diterima: laporan menjadi kurang spesifik bagi pembaca umum.
Petugas yang menangani tetap melihat sekolah sebenarnya.

### 11.4 Kepala sekolah tidak melihat semuanya

```rust
const SCHOOL_HIDDEN_CATEGORIES: [&str; 3] = ["kekerasan", "pelecehan", "pungli"];
```

Pada tiga kategori itu, kepala sekolah bisa jadi **pihak yang diadukan**.
Laporan semacam itu langsung diteruskan ke Dinas dan tidak pernah muncul di
dashboard sekolah. Laporan di luar cakupan menghasilkan **404, bukan 403** —
403 memberi tahu bahwa laporannya ada.

### 11.5 Membuka identitas

Ada keadaan ketika identitas memang harus dibuka: permintaan penyidik, atau
laporan yang mengancam keselamatan. Jalannya satu, dan berbayar:

1. Butuh izin `unmask_panic_report` (bawaannya hanya Superadmin).
2. Butuh alasan tertulis minimal 20 karakter.
3. API menulis `panic_unmask_logs` **sebelum** mengembalikan identitas.
4. Catatan itu tidak dapat dihapus dari mana pun — model Laravel-nya
   `$fillable` kosong dan tidak ada rute penghapusan.
5. Riwayat pembukaan **ditampilkan pada halaman laporan** kepada semua yang
   boleh melihatnya, dan direkap di `/admin/panic/unmask-logs`.

Identitas hasil pembukaan hanya melewati flash session; dashboard tidak pernah
menyimpannya ke tabel mana pun.

### 11.6 Metadata media

Foto yang diunggah di-**re-encode** untuk membuang EXIF, termasuk koordinat
GPS. Sebuah foto lorong sekolah yang membawa koordinat tepat dan jam pengambilan
adalah cara paling mudah mengidentifikasi pengunggahnya.

---

## 12. Pemberkasan kepegawaian

Bagian paling sederhana dari tiga layanan, dengan dua keputusan yang layak
disebut.

**Daftar periksa disusun dari jenis dokumen, bukan dari berkas yang sudah
diunggah.** `document_types` menyimpan dokumen apa saja yang diminta untuk
tiap keperluan (kenaikan pangkat, sertifikasi, tunjangan, mutasi, pensiun).
Halaman verifikasi menampilkan seluruh daftar itu, bukan hanya baris yang ada
di `document_files`. Konsekuensinya, yang **kurang** ikut terlihat — dan itulah
informasi yang dicari verifikator.

**Penglihatan lebih ketat daripada tenant biasa.** Berkas memuat NIK, nomor
rekening, dan ijazah. Tanpa izin `verify_document_submission`, seseorang hanya
melihat pengajuannya sendiri — bahkan untuk rekan sesekolah. Draf milik orang
lain tidak pernah terlihat verifikator: itu masih coretan, bukan pengajuan.

Pengajuan terkunci setelah berstatus `diajukan`; perubahan berkas baru mungkin
lagi setelah verifikator mengembalikannya ke `revisi`. Setiap perubahan status
melewati API sehingga `document_submission_events` selalu memuat siapa yang
mengubah apa.

---

## 13. Peta berkas

```
jargon-be/
├── api/                       Rust / Axum — API + Swagger + worker
│   ├── migrations/            SUMBER KEBENARAN skema database (sqlx)
│   │   ├── 0001-0010          absensi face recognition
│   │   ├── 0011               identitas Jargon GO (NIK/NISN, akun siswa & ortu)
│   │   ├── 0012               Panic Button (+ panic_unmask_logs)
│   │   └── 0013               Pemberkasan kepegawaian
│   └── src/
│       ├── auth/              JWT, bcrypt, extractor 3 jenis kredensial
│       ├── domain/            DTO + validasi
│       ├── face/              vektor, index per-sekolah, kualitas citra
│       ├── routes/            handler HTTP (tipis, ber-anotasi OpenAPI)
│       │   ├── panic.rs       beranda, moderasi, unmask beraudit
│       │   ├── documents.rs   pemberkasan berbasis daftar periksa
│       │   └── me.rs          data milik pengguna aplikasi
│       ├── services/          aturan domain (rules, recognition, notify)
│       │   └── anonymity.rs   handel acak & HMAC, pengaburan sekolah
│       └── workers/           outbox + pemeliharaan harian
├── admin/                     Laravel 12 + Octane — dashboard /admin
│   ├── app/Support/Tenant.php Penjaga tenant terpusat
│   ├── app/Services/          Klien ke API Rust
│   └── resources/views/backend/
│       ├── panic/             moderasi & penanganan pengaduan
│       ├── document/          verifikasi berkas
│       └── account/           akun aplikasi Jargon GO
├── infra/                     docker-compose, nginx, init PostgreSQL
└── docs/                      dokumen ini + panduan deployment

jargon-fe/                     Flutter — tablet kios + aplikasi Jargon GO
├── web/                       scaffolding debug di browser
├── run-web.bat                jalankan debug web di :5000
└── lib/
    ├── core/
    │   ├── theme/             claymorphism: palet, bayangan, widget dasar
    │   ├── api_routes.dart    SATU-SATUNYA tempat path endpoint ditulis
    │   ├── api_config.dart    alamat server aktif (dapat diubah pengguna)
    │   ├── config, http, storage
    ├── data/                  model, antrean offline, repository
    └── features/
        ├── kiosk/             face_engine (ML Kit + TFLite) + layar kios
        ├── enroll/            pendaftaran wajah
        ├── auth/              login NIK/NISN
        ├── shell/             beranda, profil, setelan alamat server
        ├── absensi/           pemantauan absensi (tanpa tombol absen)
        ├── panic/             beranda, tulis, dan detail pengaduan
        ├── berkas/            pemberkasan kepegawaian
        ├── monitor/           monitoring guru
        └── pairing/           pairing perangkat

docker-compose.yml             seluruh sistem dalam container (yang dibagikan)
.env.example                   nilai yang bisa disetel + alasannya
setup.bat / setup.sh           penyiapan pertama kali (membuat rahasia acak)
dev.bat                        buka semua service di tab Windows Terminal
dev-manual.txt                 perintah tiap tab, bila dijalankan manual
```

### 13.2 Dua cara menjalankan, dan mengapa keduanya ada

| | `docker compose` (akar) | `dev.bat` |
|---|---|---|
| Untuk | membagikan, mendemokan, memasang | **mengubah kode** |
| Dijalankan | semuanya di container | hanya PostgreSQL di container |
| Alamat | satu port, `http://localhost/` | tiga port (:5000, :8080, :8000) |
| Ubah kode | bangun ulang image | hot reload |

Container tidak dipakai untuk pengembangan sehari-hari karena setiap
perubahan berarti membangun ulang image — hot reload Flutter dan `cargo run`
jauh lebih cepat. Sebaliknya `dev.bat` tidak bisa dibagikan: ia
mengandalkan Rust, PHP, dan Flutter terpasang di mesin.

**Satu pintu masuk di mode container.** nginx melayani aplikasi di `/`,
dashboard di `/admin`, dan API di `/v1`, `/docs`, `/files`. Konsekuensi yang
diincar:

* Aplikasi web dan API berada di **origin yang sama**, sehingga CORS tidak
  berlaku sama sekali — tidak ada daftar origin yang harus dijaga sinkron
  dengan port yang sedang dipakai.
* Aplikasi mengambil alamat API dari **origin halamannya sendiri**
  (`API_SAME_ORIGIN`), jadi image yang sama benar di `localhost`, di IP LAN,
  maupun di balik nama domain. Alamat yang dipaku saat build hanya benar di
  satu mesin — dan itu yang membuat sebuah image tidak bisa diserahkan ke
  orang lain.

Harga yang dibayar: satu tabrakan path. Metronic menaruh asetnya di
`/assets/`, dan Flutter web juga. Yang mengalah dashboard — `ASSET_URL=/admin`
membuat `asset()` memancarkan `/admin/assets/...`, dan nginx membuang
awalannya kembali sebelum meneruskan. Satu variabel, tanpa memindahkan
berkas apa pun.

### 13.1 Aplikasi berjalan di dua dunia

Aplikasi yang sama dapat dibangun untuk Android/iOS **dan** untuk browser
(dipakai saat pengembangan, supaya menu Jargon GO bisa diuji tanpa emulator).
Dua bagian tidak bisa dipakai bersama, dan keduanya dipisahkan lewat
conditional import — bukan lewat pengecekan `kIsWeb` di dalam kode:

| Bagian | Perangkat | Web | Mengapa |
|---|---|---|---|
| Antrean offline | SQLite (`sqflite`) | memori | `sqflite` tidak berjalan di web |
| Mode kios | layar kios sungguhan | halaman penjelasan | `tflite_flutter` berdiri di atas `dart:ffi`, yang **tidak dapat dikompilasi ke web sama sekali** |

Perbedaannya penting: `kIsWeb` adalah pemeriksaan saat berjalan, sehingga
kodenya tetap ikut dikompilasi. Untuk `dart:ffi` itu tidak cukup — `flutter
run -d chrome` akan gagal pada tahap build, bukan menampilkan pesan yang rapi.

Konsekuensi yang perlu diingat saat menambah fitur: satu impor baru yang
menyentuh kamera, ML Kit, atau TFLite dari jalur yang terjangkau `main.dart`
akan merusak debug web **tanpa peringatan apa pun dari `flutter analyze`**.
Karena itu `flutter build web` termasuk dalam pemeriksaan sebelum commit.

Menu yang tampil di aplikasi ditentukan **server** lewat `available_menus` pada
`GET /v1/me/home`, dihitung dari izin pengguna. Menambah menu untuk sebuah peran
karenanya tidak memerlukan rilis aplikasi baru — penting untuk rencana
integrasi dengan aplikasi Provsu lainnya.

---

## 14. Batasan yang diketahui

Disebutkan eksplisit agar tidak menjadi kejutan di lapangan:

1. **Model TFLite tidak disertakan** dalam repositori (lisensi + ukuran).
   Lihat `jargon-fe/assets/models/README.md`.
2. **Liveness pasif**, bukan anti-spoof kelas tinggi (§8).
3. **Impor massal siswa** memiliki skema tabel (`import_jobs`) dan endpoint
   penyimpanan, tetapi pemrosesan CSV/XLSX-nya belum diimplementasikan.
   Untuk 700.000 siswa ini akan menjadi pekerjaan berikutnya yang paling
   penting.
4. **Ekspor laporan** saat ini dilakukan di sisi klien (CSV dari tabel yang
   tampil). Tabel `report_exports` sudah disiapkan untuk ekspor asinkron
   berukuran besar.
5. **Storage berkas** memakai filesystem lokal melalui satu abstraksi
   (`services/storage.rs`). Untuk produksi multi-replika, arahkan ke volume
   bersama atau tambahkan backend S3 di berkas itu.
6. **Pendaftaran wajah dari dashboard** memerlukan model TensorFlow.js di
   `admin/public/assets/models/facenet/`. Alternatifnya, gunakan tablet
   bermode `enroll` yang sudah lengkap.
7. **Anonimitas Panic Button berhenti pada akses database.** Siapa pun dengan
   kredensial PostgreSQL dapat membaca `author_user_id` tanpa melewati
   `panic_unmask_logs`. Yang melindungi di lapisan itu adalah pembatasan akun
   database dan audit di sisi infrastruktur, bukan aplikasi ini. Bila
   ancamannya termasuk DBA, langkah berikutnya adalah menyimpan
   `author_user_id` terenkripsi dengan kunci yang hanya dipegang API.
8. **Notifikasi push untuk aplikasi** baru sampai tahap pencatatan perangkat
   (`user_devices`). Pengirimannya belum tersambung ke FCM/APNs; notifikasi
   yang berjalan saat ini adalah WhatsApp/Telegram/Email lewat outbox (§7).
9. **Impor massal orang tua** belum ada. Akun orang tua dibuat satu per satu
   karena tautan ke anak harus diverifikasi manual (§10.2) — untuk sekolah
   besar ini pekerjaan yang nyata dan layak dijadwalkan.
