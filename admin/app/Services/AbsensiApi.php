<?php

namespace App\Services;

use Illuminate\Http\Client\PendingRequest;
use Illuminate\Http\Client\Response;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Log;
use RuntimeException;

/**
 * Klien untuk API Rust (`jargon-be/api`).
 *
 * PEMBAGIAN TANGGUNG JAWAB — ini bagian penting arsitekturnya:
 *
 *   * Untuk MEMBACA daftar & laporan, dashboard query PostgreSQL langsung
 *     lewat Eloquent. Lebih cepat (tanpa hop HTTP), dan DataTables
 *     server-side memang butuh akses SQL.
 *
 *   * Untuk MENULIS hal yang punya aturan domain — pendaftaran wajah,
 *     koreksi absensi, pairing perangkat, kirim notifikasi — dashboard
 *     MEMANGGIL API ini. Aturannya (ambang kemiripan, jendela jam,
 *     invalidasi index wajah, outbox transaksional) hanya ada di satu tempat,
 *     sehingga tidak mungkin berbeda antara tablet dan dashboard.
 *
 * Kredensial: header X-Api-Key / X-Api-Secret dari tabel `api_clients`.
 */
class AbsensiApi
{
    public function __construct(
        private readonly string $baseUrl,
        private readonly ?string $keyId,
        private readonly ?string $secret,
        private readonly int $timeout = 20,
    ) {
    }

    public static function make(): self
    {
        $config = config('services.absensi_api');

        return new self(
            rtrim($config['url'] ?? 'http://127.0.0.1:8080', '/'),
            $config['key_id'] ?? null,
            $config['secret'] ?? null,
            (int) ($config['timeout'] ?? 20),
        );
    }

    public function isConfigured(): bool
    {
        return $this->keyId !== null && $this->secret !== null;
    }

    private function request(): PendingRequest
    {
        return Http::baseUrl($this->baseUrl)
            ->timeout($this->timeout)
            ->connectTimeout(5)
            ->acceptJson()
            ->asJson()
            ->withHeaders([
                'X-Api-Key' => (string) $this->keyId,
                'X-Api-Secret' => (string) $this->secret,
            ]);
    }

    /**
     * Panggil API sebagai pengguna yang sedang login.
     *
     * Access token pengguna diteruskan agar API menerapkan RBAC dan penjagaan
     * tenant yang sama — dashboard tidak boleh menjadi jalan pintas yang
     * melewati pemeriksaan izin.
     */
    private function asUser(string $accessToken): PendingRequest
    {
        return $this->request()->withToken($accessToken);
    }

    /**
     * Panggilan umum atas nama pengguna yang sedang login.
     *
     * Dipakai fitur yang endpoint-nya banyak tetapi bentuknya seragam
     * (Panic Button, Pemberkasan). Menambah satu method khusus untuk tiap
     * endpoint hanya akan menghasilkan pembungkus yang identik.
     *
     * Token pengguna tetap dikirim, sehingga API menerapkan RBAC dan
     * penjagaan cakupan atas nama orang itu — dashboard tidak pernah menjadi
     * jalan pintas yang melewati pemeriksaan izin.
     *
     * @param  array<string, mixed>|null  $payload
     * @return array<string, mixed>
     */
    public function call(string $method, string $path, ?array $payload = null): array
    {
        try {
            $client = $this->asUser(self::tokenFromSession());

            $response = match (strtoupper($method)) {
                'GET' => $client->get($path, $payload ?? []),
                'POST' => $client->post($path, $payload ?? []),
                'PATCH' => $client->patch($path, $payload ?? []),
                'PUT' => $client->put($path, $payload ?? []),
                'DELETE' => $client->delete($path),
                default => throw new RuntimeException("Metode {$method} tidak didukung."),
            };

            return $this->handle($response, "permintaan {$path}");
        } catch (RuntimeException $e) {
            // Sesi API belum ada (mis. API mati saat login). Pesannya sudah
            // menjelaskan langkah perbaikannya kepada operator.
            return ['success' => false, 'message' => $e->getMessage(), 'errors' => []];
        }
    }

    // =================================================================
    // Biometrik
    // =================================================================

    /**
     * Daftarkan satu sampel wajah siswa.
     *
     * @param  string  $imageBase64  gambar wajah ter-crop (JPEG/PNG)
     * @param  array<int, float>  $embedding  vektor 512 dimensi dari perangkat
     * @return array<string, mixed>
     */
    /**
     * Ambil isi berkas tersimpan (foto wajah, berkas pemberkasan, lampiran
     * pengaduan) atas nama pengguna yang sedang login.
     *
     * Berkas tidak pernah dilayani langsung oleh web server, dan browser tidak
     * memegang access token API — jadi permintaan `<img src>` atau tautan unduh
     * harus melewati dashboard, yang menempelkan token sesi lalu meneruskan
     * isinya. Tanpa jalur ini, satu-satunya alternatif adalah membuka
     * `/files/*` tanpa autentikasi, yang berarti siapa pun yang menebak object
     * key dapat mengunduh ijazah atau foto pengaduan.
     *
     * @return array{success: bool, body?: string, mime?: string, status?: int, message?: string}
     */
    public function fetchFile(string $key): array
    {
        try {
            $response = $this->asUser(self::tokenFromSession())
                ->withoutRedirecting()
                ->accept('*/*')
                ->get('/files/'.ltrim($key, '/'));
        } catch (RuntimeException $e) {
            return ['success' => false, 'status' => 503, 'message' => $e->getMessage()];
        }

        if (! $response->successful()) {
            Log::warning('API menolak permintaan berkas', [
                'key' => $key,
                'status' => $response->status(),
            ]);

            return [
                'success' => false,
                'status' => $response->status(),
                'message' => 'Berkas tidak dapat diakses.',
            ];
        }

        return [
            'success' => true,
            'body' => $response->body(),
            'mime' => $response->header('Content-Type') ?: 'application/octet-stream',
        ];
    }

    public function enrollFace(
        string $accessToken,
        string $studentId,
        string $imageBase64,
        array $embedding,
        string $modelVersion,
        string $pose = 'frontal',
    ): array {
        return $this->handle(
            $this->asUser($accessToken)->post("/v1/students/{$studentId}/face", [
                'image_base64' => $imageBase64,
                'embedding' => $embedding,
                'model_version' => $modelVersion,
                'pose' => $pose,
            ]),
            'pendaftaran wajah'
        );
    }

    public function deleteFaceSample(string $accessToken, string $enrollmentId): array
    {
        return $this->handle(
            $this->asUser($accessToken)->delete("/v1/face-enrollments/{$enrollmentId}"),
            'hapus sampel wajah'
        );
    }

    // =================================================================
    // Absensi
    // =================================================================

    /**
     * Koreksi absensi satu siswa.
     *
     * @param  array<string, mixed>  $payload
     * @return array<string, mixed>
     */
    public function manualAttendance(string $accessToken, array $payload): array
    {
        return $this->handle(
            $this->asUser($accessToken)->post('/v1/attendances/manual', $payload),
            'koreksi absensi'
        );
    }

    /**
     * @param  array<string, mixed>  $payload
     * @return array<string, mixed>
     */
    public function bulkAttendance(string $accessToken, array $payload): array
    {
        return $this->handle(
            $this->asUser($accessToken)->post('/v1/attendances/bulk', $payload),
            'koreksi absensi massal'
        );
    }

    // =================================================================
    // Perangkat
    // =================================================================

    /**
     * @param  array<string, mixed>  $payload
     * @return array<string, mixed>
     */
    public function createDevice(string $accessToken, array $payload): array
    {
        return $this->handle(
            $this->asUser($accessToken)->post('/v1/devices', $payload),
            'pembuatan perangkat'
        );
    }

    public function regeneratePairingCode(string $accessToken, string $deviceId): array
    {
        return $this->handle(
            $this->asUser($accessToken)->post("/v1/devices/{$deviceId}/pairing-code"),
            'pembuatan kode pairing'
        );
    }

    public function revokeDevice(string $accessToken, string $deviceId): array
    {
        return $this->handle(
            $this->asUser($accessToken)->post("/v1/devices/{$deviceId}/revoke"),
            'pencabutan token perangkat'
        );
    }

    // =================================================================
    // Notifikasi
    // =================================================================

    /**
     * @param  array<int, string>  $studentIds
     * @return array<string, mixed>
     */
    public function sendNotification(
        string $accessToken,
        array $studentIds,
        string $body,
        ?string $channel = null,
        ?string $subject = null,
    ): array {
        return $this->handle(
            $this->asUser($accessToken)->post('/v1/notifications/send', array_filter([
                'student_ids' => $studentIds,
                'body' => $body,
                'channel' => $channel,
                'subject' => $subject,
            ], fn ($v) => $v !== null)),
            'pengiriman notifikasi'
        );
    }

    public function retryNotification(string $accessToken, string $outboxId): array
    {
        return $this->handle(
            $this->asUser($accessToken)->post("/v1/notifications/outbox/{$outboxId}/retry"),
            'pengiriman ulang notifikasi'
        );
    }

    // =================================================================
    // Autentikasi (menerbitkan access token untuk sesi dashboard)
    // =================================================================

    /**
     * Tukar kredensial pengguna dengan access token API.
     *
     * Dipanggil sekali saat login dashboard; tokennya disimpan di session
     * sehingga aksi tulis berikutnya bisa memakai identitas pengguna itu.
     *
     * @return array{access_token: string, refresh_token: string, expires_at: int}|null
     */
    public function login(string $identifier, string $password): ?array
    {
        try {
            $response = $this->request()->post('/v1/auth/login', [
                'identifier' => $identifier,
                'password' => $password,
                'device_name' => 'Dashboard /admin',
            ]);

            if (! $response->successful()) {
                return null;
            }

            $data = $response->json('data') ?? [];

            return [
                'access_token' => $data['access_token'] ?? '',
                'refresh_token' => $data['refresh_token'] ?? '',
                'expires_at' => (int) ($data['expires_at'] ?? 0),
            ];
        } catch (\Throwable $e) {
            // API sedang tidak tersedia. Login dashboard tetap boleh lanjut
            // (Laravel memverifikasi kata sandi sendiri terhadap tabel users
            // yang sama); hanya aksi tulis yang akan gagal dengan pesan jelas.
            Log::warning('Gagal mengambil access token API', ['error' => $e->getMessage()]);

            return null;
        }
    }

    public function refresh(string $refreshToken): ?array
    {
        try {
            $response = $this->request()->post('/v1/auth/refresh', [
                'refresh_token' => $refreshToken,
            ]);

            if (! $response->successful()) {
                return null;
            }
            $data = $response->json('data') ?? [];

            return [
                'access_token' => $data['access_token'] ?? '',
                'refresh_token' => $data['refresh_token'] ?? '',
                'expires_at' => (int) ($data['expires_at'] ?? 0),
            ];
        } catch (\Throwable $e) {
            Log::warning('Gagal refresh access token API', ['error' => $e->getMessage()]);

            return null;
        }
    }

    /**
     * Status kesehatan API — ditampilkan pada kartu dashboard Superadmin.
     *
     * @return array<string, mixed>|null
     */
    public function health(): ?array
    {
        try {
            $response = Http::baseUrl($this->baseUrl)
                ->timeout(3)
                ->acceptJson()
                ->get('/health');

            return $response->successful() ? ($response->json('data') ?? null) : null;
        } catch (\Throwable) {
            return null;
        }
    }

    /**
     * Ubah respons API menjadi array, atau lempar pesan yang layak dibaca.
     *
     * @return array<string, mixed>
     */
    private function handle(Response $response, string $action): array
    {
        if ($response->successful()) {
            return [
                'success' => true,
                'data' => $response->json('data'),
                'message' => $response->json('message') ?? 'Berhasil',
            ];
        }

        $payload = $response->json() ?? [];
        $message = $payload['message'] ?? "Gagal melakukan {$action}.";

        // Error validasi per-field diteruskan apa adanya agar form dashboard
        // bisa menandai input yang bermasalah.
        $errors = [];
        foreach ($payload['errors'] ?? [] as $error) {
            if (isset($error['field'], $error['message'])) {
                $errors[$error['field']][] = $error['message'];
            }
        }

        Log::warning("API menolak {$action}", [
            'status' => $response->status(),
            'code' => $payload['code'] ?? null,
            'message' => $message,
        ]);

        return [
            'success' => false,
            'status' => $response->status(),
            'code' => $payload['code'] ?? 'error',
            'message' => $message,
            'errors' => $errors,
        ];
    }

    /**
     * Access token pengguna aktif, diperbarui otomatis bila hampir kedaluwarsa.
     *
     * Access token berumur 1 jam sementara sesi dashboard bisa berjam-jam.
     * Tanpa pembaruan di sini, aksi tulis akan gagal di tengah pekerjaan
     * operator dengan pesan "token kedaluwarsa" yang tidak bisa ia perbaiki
     * selain logout.
     */
    public static function tokenFromSession(): string
    {
        $token = session('absensi_api.access_token');
        $expiresAt = (int) session('absensi_api.expires_at', 0);
        $refreshToken = session('absensi_api.refresh_token');

        $needsRefresh = ! is_string($token)
            || $token === ''
            // Diperbarui 2 menit sebelum kedaluwarsa agar tidak kehabisan
            // waktu di tengah request.
            || ($expiresAt > 0 && $expiresAt - 120 <= time());

        if ($needsRefresh && is_string($refreshToken) && $refreshToken !== '') {
            $fresh = self::make()->refresh($refreshToken);

            if ($fresh !== null && $fresh['access_token'] !== '') {
                session()->put('absensi_api', $fresh);

                return $fresh['access_token'];
            }
        }

        if (! is_string($token) || $token === '') {
            throw new RuntimeException(
                'Sesi API tidak tersedia. Silakan keluar lalu masuk kembali agar dashboard '
                .'mendapatkan token baru. Bila berulang, periksa apakah layanan API '
                .'(jargon-be/api) sedang berjalan.'
            );
        }

        return $token;
    }
}
