<?php

namespace App\Http\Controllers\Backend;

use App\Http\Controllers\Controller;
use App\Services\AbsensiApi;
use Illuminate\Http\Request;
use Illuminate\Routing\Controllers\HasMiddleware;
use Symfony\Component\HttpFoundation\Response;

/**
 * Penerus berkas tersimpan dari API ke browser.
 *
 * Foto wajah, berkas pemberkasan, dan lampiran pengaduan tidak pernah dilayani
 * langsung oleh web server — object key-nya memuat UUID acak, tetapi "URL yang
 * sulit ditebak" bukan kontrol akses. Setiap permintaan harus melewati
 * pemeriksaan izin di API.
 *
 * Masalahnya, browser tidak memegang access token API: yang dipegangnya adalah
 * cookie sesi dashboard. Karena itu `<img src>` dan tautan unduh diarahkan ke
 * sini, dan controller ini menempelkan token sesi pengguna sebelum meneruskan
 * isinya. Otorisasinya tetap milik API — dashboard tidak memutuskan apa pun,
 * hanya menyampaikan identitas orang yang sedang login.
 *
 * Konsekuensi yang diterima: isi berkas melewati memori PHP alih-alih
 * di-stream langsung. Untuk ijazah beberapa MB yang dibuka sesekali oleh
 * verifikator, itu bukan jalur panas. Bila kelak menjadi masalah, jawabannya
 * adalah URL bertanda tangan berumur pendek dari API, bukan membuka `/files/*`
 * tanpa autentikasi.
 */
class FileProxyController extends Controller implements HasMiddleware
{
    public static function middleware(): array
    {
        return ['auth'];
    }

    public function __invoke(Request $request, string $key): Response
    {
        // Object key datang dari URL. Ia hanya diteruskan ke API — yang
        // memeriksa izinnya — tetapi `..` tetap ditolak di sini agar tidak ada
        // permintaan aneh yang sampai ke sana sama sekali.
        if (str_contains($key, '..')) {
            abort(404);
        }

        $result = AbsensiApi::make()->fetchFile($key);

        if (! $result['success']) {
            abort($result['status'] === 403 ? 403 : 404, $result['message'] ?? 'Berkas tidak ditemukan.');
        }

        return response($result['body'], 200, [
            'Content-Type' => $result['mime'],
            // Data pribadi: tidak boleh disimpan proxy bersama.
            'Cache-Control' => 'private, max-age=300',
            'Content-Disposition' => 'inline',
            'X-Content-Type-Options' => 'nosniff',
        ]);
    }
}
