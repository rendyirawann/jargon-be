# BACA SEBELUM MENARUH MODEL DI SINI

Halaman **Pendaftaran Wajah** di dashboard (`/jargon-be/admin`, view
`resources/views/backend/biometric/capture.blade.php`) memuat:

* `public/assets/vendor/tfjs/tf.min.js`            (baris 162)
* `public/assets/models/facenet/model.json` + shard (baris 205)

Keduanya **tidak ada di repo** dan harus dipasok Dinas.

## Bahaya yang tidak dijaga oleh kode mana pun

Aplikasi tablet memakai model **berbeda** dengan halaman ini:

| | Dashboard (halaman ini) | Tablet (jargon-fe) |
|---|---|---|
| Format | TensorFlow.js GraphModel | TensorFlow Lite |
| Berkas | `models/facenet/model.json` | `assets/models/mobilefacenet.tflite` |
| Input | **160 x 160** | **112 x 112** |
| Praproses | crop persegi tengah, tanpa flip (`capture.blade.php:214-222`) | crop + margin 25% + flip horizontal (`face_engine.dart:347-352,402-403`) |
| Output | 512-d | 512-d |

Server **tidak bisa membedakan keduanya**. Ia hanya membandingkan string:
`api/src/services/recognition.rs:94` dan `api/src/services/enrollment.rs:42`.
Lebih jauh, `enrollment.rs:260` menyimpan `model_version` dari **konfigurasi
server**, bukan dari yang dikirim klien. Jadi bila dashboard memakai FaceNet-160
dan tablet memakai MobileFaceNet-112 — dua-duanya 512 dimensi, dua-duanya
berlabel `mobilefacenet-v1` — **semua penjaga lolos** dan embedding dari dua
ruang vektor berbeda tercampur di satu tabel.

Akibatnya bukan "akurasi menurun", melainkan **identifikasi acak**: siswa A
dikenali sebagai siswa B. Lihat `jargon-fe/assets/models/README.md`.

## Aturan yang harus dipegang

1. Pilih **satu** arsitektur + satu berkas bobot untuk SELURUH sistem.
2. Bila dashboard dan tablet harus memakai runtime berbeda (TF.js vs TFLite),
   keduanya wajib merupakan **konversi dari bobot yang sama**, dengan ukuran
   input dan praproses yang **disamakan lebih dulu di kode**.
3. Bila tidak bisa dijamin: **jangan pakai pendaftaran dari dashboard.** Pakai
   tablet bermode `enroll` (disebut `jargon-be/docs/ARCHITECTURE.md:670-672`
   sebagai alternatif yang "sudah lengkap"), sehingga hanya ada satu model.
4. Naikkan `FACE_MODEL_VERSION` di `api/.env` setiap kali model berganti, dan
   hitung ulang embedding dari gambar pendaftaran yang tersimpan.

Saat ini tabel `face_embeddings` **kosong**, jadi keputusan ini masih bebas
biaya. Setelah pendaftaran berjalan, mengganti model berarti menghitung ulang
seluruh embedding.

Alat bantu: `/var/www/html/face-recognition/tools/check-face-model.sh`
memeriksa berkas `.tflite` sebelum dipasang.
