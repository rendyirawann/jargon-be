# Memperbarui server Jargon GO yang sudah berjalan

Berkas ini untuk pemasangan yang **sudah hidup dan sedang melayani**, dan
ingin dinaikkan ke versi terbaru.

Untuk pemasangan **baru** dari nol, jangan pakai berkas ini — pakai
[DEPLOYMENT.md](DEPLOYMENT.md). Keduanya dipisah karena dibaca dalam
keadaan yang sangat berbeda: pemasangan baru tidak punya apa pun yang bisa
hilang, sedangkan pembaruan punya data absensi dan wajah terdaftar yang
harus dijaga.

---

## Baca dulu sebelum `git pull`

Pembaruan ke versi ini mengubah **dimensi embedding wajah dari 512 menjadi
128**, karena pengenalan wajah kini berjalan di browser dengan face-api.js.

Akibatnya, bila server Anda sudah punya wajah terdaftar, migrasi `0014`
akan **berhenti dengan sengaja**. Ia tidak merusak apa pun — tetapi Anda
harus memutuskan lebih dulu apa yang dilakukan atas data itu, bukan
menemukan masalahnya di tengah pembaruan.

Bila server belum pernah dipakai mendaftarkan wajah, seluruh peringatan di
atas tidak berlaku dan pembaruan ini biasa saja.

**Perkiraan waktu:** 10–20 menit bila tidak ada wajah terdaftar. Lebih lama
bila wajah harus didaftarkan ulang, dan itu pekerjaan operator sekolah,
bukan pekerjaan perintah.

---

## Langkah 1 — periksa apa yang terdampak

```bash
cd /path/ke/jargon-be
docker compose exec -T postgres psql -U absensi -d absensi -c \
  "SELECT count(*) AS wajah_terdaftar FROM face_embeddings"
```

**Hasil 0** — aman, lanjut ke langkah 2.

**Ada baris** — pilih satu, sadar konsekuensinya:

* **Daftarkan ulang.** Kosongkan tabelnya, lalu daftarkan ulang lewat
  `/admin/biometric`. Embedding 512-d dan 128-d tidak bisa dikonversi: 128
  angka itu bukan ringkasan dari 512 angka yang lain, melainkan keluaran
  model yang sama sekali berbeda. Tidak ada jalan tengah di sini.

  ```bash
  docker compose exec -T postgres psql -U absensi -d absensi -c \
    "TRUNCATE face_embeddings; \
     UPDATE students SET face_enrolled = FALSE, face_sample_count = 0"
  ```

  Absensi yang sudah tercatat **tidak** ikut terhapus — tabel `attendances`
  tidak menyimpan data wajah sama sekali, hanya waktu, siswa, kelas, dan
  sekolah. Yang hilang hanya kemampuan mengenali, bukan riwayat kehadiran.

* **Tetap di 512-d.** Jangan tarik pembaruan ini. Pengenalan wajah lewat
  browser memang belum bisa dipakai, tetapi jalur tablet tetap utuh dan
  tidak ada yang rusak.

---

## Langkah 2 — cadangkan

Sekali perintah, sebelum apa pun berubah.

```bash
docker compose exec -T postgres pg_dump -U absensi -Fc absensi \
  > ~/absensi-sebelum-upgrade-$(date +%F).dump
```

Migrasi `0014` mengubah tipe kolom, dan sqlx **tidak** punya migrasi turun.
Mundur dari pembaruan ini berarti memulihkan cadangan — jadi langkah ini
bukan kehati-hatian berlebih, ia satu-satunya jalan pulang.

Pastikan berkasnya benar-benar berisi sebelum melanjutkan:

```bash
ls -lh ~/absensi-sebelum-upgrade-*.dump
```

---

## Langkah 3 — tarik kode

```bash
git pull origin main
```

Bila ditolak karena ada perubahan lokal di server:

```bash
git status                 # lihat dulu apa yang berubah, jangan langsung buang
git stash                  # simpan sementara, atau
git checkout -- <berkas>   # buang bila memang tidak sengaja
```

`.env` **tidak** akan tertimpa — berkas itu ada di `.gitignore`. Itu
disengaja, dan justru karena itu langkah berikutnya wajib.

---

## Langkah 4 — sesuaikan `.env` DENGAN TANGAN

Langkah yang paling sering terlewat, dan gejalanya menyesatkan: setiap
pemindaian dijawab *"Versi model di perangkat berbeda dengan server"*,
padahal yang salah hanya tiga baris di `.env`.

`git pull` memperbarui `.env.example`, **bukan** `.env` Anda.

```ini
FACE_EMBEDDING_DIM=128
FACE_MODEL_VERSION=faceapi-v1
FACE_MATCH_THRESHOLD=0.82
```

`0.82`, bukan `0.62`. Ambang itu bergantung model — `0.62` disetel untuk
MobileFaceNet, sementara face-api.js butuh `0.82` (euclidean 0.6 setara
cosine 0.82). Penjelasan lengkapnya di DEPLOYMENT.md §2.12.

Ini bukan soal akurasi yang kurang enak. Ambang yang terlalu rendah berarti
**wajah orang lain bisa tercatat sebagai siswa**, dan tidak ada di layar
mana pun yang menunjukkan itu sedang terjadi.

Untuk melihat kunci baru lain yang mungkin perlu diisi:

```bash
diff <(grep -oE '^[A-Z_]+' .env         | sort -u) \
     <(grep -oE '^[A-Z_]+' .env.example | sort -u)
```

---

## Langkah 5 — bangun ulang, migrasi, jalankan

```bash
# Dari image siap pakai
docker compose -f docker-compose.prod.yml pull
docker compose -f docker-compose.prod.yml run --rm \
    --entrypoint /usr/local/bin/jargon-api api migrate
docker compose -f docker-compose.prod.yml up -d
```

```bash
# Atau membangun dari sumber (butuh ../jargon-fe berdampingan)
docker compose build
docker compose run --rm --entrypoint /usr/local/bin/jargon-api api migrate
docker compose up -d
```

Migrasi dijalankan terpisah **bukan** karena wajib — API menerapkannya saat
start — melainkan supaya kegagalan migrasi terlihat sebagai kegagalan
migrasi, bukan sebagai "API tidak mau menyala". Pada pembaruan yang
mengubah tipe kolom, selisih itu menentukan berapa lama Anda mencari.

Tiga migrasi yang baru:

| Migrasi | Isi |
|---|---|
| `0014` | Vektor wajah dari 512-d menjadi 128-d, HNSW dibangun ulang |
| `0015` | Izin `operate_face_kiosk` untuk halaman absensi web |
| `0016` | Penajaman index sesuai query yang benar-benar dijalankan |

**Untuk basis data yang sudah besar:** `0016` memakai `CREATE INDEX` tanpa
`CONCURRENTLY`, karena sqlx menjalankan migrasi di dalam transaksi dan
`CONCURRENTLY` tidak diizinkan di sana. Pada tabel berisi puluhan juta
baris, perintah itu **mengunci tulis** selama index dibangun. Bila itu
jatuh pada jam absensi pagi, jalankan versi `CONCURRENTLY`-nya secara
manual di luar jam sibuk lebih dulu — migrasinya lalu menjadi no-op karena
memakai `IF NOT EXISTS`. Perintahnya ada di kepala
`api/migrations/0016_index_optimasi.sql`.

---

## Langkah 6 — restart nginx

```bash
docker compose restart nginx
```

Dua alasan berbeda:

1. **Konfigurasinya berubah.** `proxy_set_header Host` sekarang
   `$http_host`, bukan `$host`. `$host` membuang nomor port, sehingga pada
   `HTTP_PORT` selain 80 seluruh redirect Laravel kehilangan portnya dan
   mendarat di server lain di mesin yang sama. Gejalanya: login tampak
   gagal padahal kata sandinya benar.
2. **nginx bisa memegang alamat IP upstream yang lama.** Container `api`
   yang baru dibangun kadang mendapat IP baru; bila nginx masih memakai
   yang lama, semua endpoint API menjawab `502 Bad Gateway` meski API-nya
   sehat. Tidak selalu terjadi — bergantung apakah Docker memberi IP yang
   sama — dan itulah masalahnya: gejalanya muncul sesekali dan terlihat
   seperti API yang rusak. Restart menghilangkan kemungkinan itu dengan
   biaya beberapa detik.

---

## Langkah 7 — periksa

```bash
docker compose ps
curl -fsS http://127.0.0.1:${HTTP_PORT:-80}/health/live
docker compose exec -T postgres psql -U absensi -d absensi -tAc \
  "SELECT version, description FROM _sqlx_migrations ORDER BY version DESC LIMIT 3"
docker compose exec -T api sh -c \
  'echo $FACE_EMBEDDING_DIM $FACE_MODEL_VERSION $FACE_MATCH_THRESHOLD'
```

Yang diharapkan:

```
16 index optimasi        <- migrasi teratas
128 faceapi-v1 0.82      <- konfigurasi wajah
```

Lalu satu uji yang sebenarnya, karena empat perintah di atas bisa semuanya
hijau sementara absensi tetap tidak jalan:

1. Buka `/admin/biometric` dan daftarkan satu wajah — tiga langkah: hadap
   depan, menoleh kanan, menoleh kiri.
2. Buka `/admin/devices`, ambil kode pemasangan 8 angka.
3. Buka `/admin/biometric/scan`, pasangkan dengan kode itu, lalu pindai
   wajah yang baru didaftarkan sampai **tercatat hadir**.

Selesai di titik itu, bukan di titik `health/live` menjawab 200.

---

## Bila harus mundur

```bash
docker compose down
docker compose up -d postgres
docker compose exec -T postgres psql -U absensi -d absensi \
  -c "DROP SCHEMA public CASCADE; CREATE SCHEMA public;"
docker compose exec -T postgres pg_restore -U absensi -d absensi \
  --clean --if-exists < ~/absensi-sebelum-upgrade-YYYY-MM-DD.dump
git checkout <commit-sebelumnya>
docker compose up -d --build
```

Mundur berarti memulihkan cadangan, bukan menjalankan migrasi balik —
itulah sebabnya langkah 2 tidak boleh dilewati.

---

## Pembaruan rutin berikutnya

Pembaruan yang **tidak** mengubah skema jauh lebih ringkas. Langkah 1, 2,
dan 4 hanya diperlukan bila catatan rilis menyebut perubahan skema atau
kunci `.env` baru:

```bash
cd /path/ke/jargon-be
git pull origin main
docker compose -f docker-compose.prod.yml pull
docker compose -f docker-compose.prod.yml run --rm \
    --entrypoint /usr/local/bin/jargon-api api migrate
docker compose -f docker-compose.prod.yml up -d
docker compose restart nginx
curl -fsS http://127.0.0.1:${HTTP_PORT:-80}/health/live
```

Kebiasaan yang layak dipertahankan: `pg_dump` di langkah 2 tetap dijalankan
walau skemanya tidak berubah. Biayanya beberapa detik, dan satu kali saja
ia dibutuhkan, ia membayar seluruh biaya itu sekaligus.
