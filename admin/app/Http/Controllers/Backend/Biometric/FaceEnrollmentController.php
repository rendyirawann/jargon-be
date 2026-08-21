<?php

namespace App\Http\Controllers\Backend\Biometric;

use App\Http\Controllers\Controller;
use App\Models\Classroom;
use App\Models\FaceEnrollment;
use App\Models\Student;
use App\Services\AbsensiApi;
use App\Support\Tenant;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Routing\Controllers\HasMiddleware;
use Illuminate\Routing\Controllers\Middleware;
use Illuminate\View\View;

/**
 * Pendaftaran wajah siswa.
 *
 * Ini satu-satunya alur di seluruh sistem yang menerima GAMBAR wajah.
 * Absensi harian tidak mengirim gambar sama sekali — hanya vektor embedding
 * yang langsung dibuang setelah dicocokkan.
 *
 * Ekstraksi embedding dilakukan di PERANGKAT (browser lewat TensorFlow.js,
 * atau aplikasi Flutter lewat TFLite), bukan di PHP. Alasannya: versi model
 * yang menghasilkan embedding pendaftaran harus sama persis dengan yang
 * dipakai tablet saat absen, jika tidak vektornya tidak sebanding dan
 * pengenalan menjadi acak. Dengan satu model di sisi klien, konsistensi itu
 * dijamin oleh `model_version` yang divalidasi server.
 *
 * Penyimpanan gambar + vektor dilakukan API Rust, yang juga:
 *   - menghitung ulang kualitas foto (klien tidak dipercaya),
 *   - menolak foto yang terlalu mirip siswa LAIN (salah pilih siswa),
 *   - menolak foto yang terlalu berbeda dari sampel siswa itu sendiri,
 *   - membuang cache index wajah sekolah agar tablet langsung mengenali.
 */
class FaceEnrollmentController extends Controller implements HasMiddleware
{
    public static function middleware(): array
    {
        return [
            'auth',
            new Middleware('can:view_face_enrollment', only: ['index', 'show']),
            new Middleware('can:create_face_enrollment', only: ['capture', 'store']),
            // Izin TERSENDIRI, bukan izin pendaftaran wajah maupun koreksi
            // absensi: halaman itu MENCATAT kehadiran lewat pemindaian, dan
            // itu kewenangan yang sebaiknya bisa diberikan terpisah — guru
            // boleh mendaftarkan wajah tanpa boleh menjalankan gerbang.
            new Middleware('can:operate_face_kiosk', only: ['scan']),
            new Middleware('can:delete_face_enrollment', only: ['destroy', 'reset']),
            // 'reset' menghapus SELURUH sampel satu siswa, jadi izinnya sama
            // dengan hapus satuan — bukan izin pendaftaran. Saat ini dimiliki
            // peran superadmin dan staff_tu.
        ];
    }

    public function index(Request $request): View
    {
        $schoolId = Tenant::currentSchoolId($request->query('school_id'));
        $classroomId = $request->query('classroom_id');
        $filter = $request->query('filter', 'belum');

        $students = collect();
        if ($schoolId) {
            $query = Student::query()
                ->where('school_id', $schoolId)
                ->where('status', 'aktif')
                ->with('classroom:id,name')
                ->when($classroomId, fn ($q) => $q->where('current_classroom_id', $classroomId));

            $query = match ($filter) {
                'belum' => $query->needsFaceEnrollment(),
                'kurang' => $query->underSampled(),
                'lengkap' => $query->where('face_enrolled', true)
                    ->where('face_sample_count', '>=', Student::RECOMMENDED_SAMPLES),
                default => $query,
            };

            $students = $query->orderBy('full_name')->paginate(30)->withQueryString();
        }

        return view('backend.biometric.index', [
            'students' => $students,
            'schoolId' => $schoolId,
            'classroomId' => $classroomId,
            'filter' => $filter,
            'schools' => Tenant::isProvinceScope() ? Tenant::selectableSchools() : collect(),
            'classrooms' => $this->classroomOptions($schoolId),
            'coverage' => $this->coverage($schoolId),
        ]);
    }

    /**
     * Halaman pengambilan foto: kamera + ekstraksi embedding di browser.
     */
    public function capture(Student $student): View
    {
        Tenant::authorizeSchool($student->school_id);

        return view('backend.biometric.capture', [
            'student' => $student->load('classroom', 'school'),
            'samples' => $student->faceEnrollments()->orderByDesc('created_at')->get(),
            'recommended' => Student::RECOMMENDED_SAMPLES,
            'modelVersion' => config('services.absensi_api.face_model_version'),
            'embeddingDim' => (int) config('services.absensi_api.embedding_dim'),
        ]);
    }

    /**
     * Absensi wajah langsung dari browser.
     *
     * Halaman ini bertindak sebagai PERANGKAT KIOS: ia pairing sekali
     * memakai kode 8 digit dari `/admin/devices`, lalu memanggil
     * `POST /v1/kiosk/recognize` yang sudah ada.
     *
     * Sengaja BUKAN endpoint baru khusus dashboard. Endpoint kios sudah
     * memuat seluruh aturan yang teruji — jendela jam masuk/pulang, jeda
     * antar-scan, ambang kemiripan, margin kembar, anti-replay nonce,
     * pencatatan device_id, dan pemicu notifikasi wali. Menyalinnya untuk
     * jalur web berarti dua salinan aturan yang bisa menyimpang, dan
     * absensi yang berbeda hanya karena alatnya berbeda.
     *
     * Controller ini tidak memegang kredensial perangkat apa pun: token
     * pairing lahir dan tinggal di browser. Yang dikirim ke view hanya
     * alamat API publik dan versi model.
     */
    public function scan(): View
    {
        return view('backend.biometric.scan', [
            // Alamat yang dipanggil BROWSER, jadi harus alamat publik —
            // bukan `http://api:8080` yang hanya berarti sesuatu di dalam
            // jaringan container.
            'apiBase' => rtrim((string) config('services.absensi_api.public_url'), '/'),
            'modelVersion' => config('services.absensi_api.face_model_version'),
            'embeddingDim' => (int) config('services.absensi_api.embedding_dim'),
        ]);
    }

    /**
     * Terima gambar + embedding dari browser lalu teruskan ke API.
     */
    public function store(Request $request, Student $student): RedirectResponse|JsonResponse
    {
        Tenant::authorizeSchool($student->school_id);

        $data = $request->validate([
            'image_base64' => ['required', 'string', 'min:100'],
            'embedding' => ['required', 'array'],
            'embedding.*' => ['numeric'],
            'model_version' => ['required', 'string', 'max:40'],
            'pose' => ['required', 'in:'.implode(',', FaceEnrollment::POSES)],
        ], [], [
            'image_base64' => 'gambar wajah',
            'embedding' => 'data biometrik',
        ]);

        $expectedDim = (int) config('services.absensi_api.embedding_dim');
        if (count($data['embedding']) !== $expectedDim) {
            return back()->withErrors([
                'embedding' => "Dimensi embedding harus {$expectedDim}, diterima "
                    .count($data['embedding']).'. Muat ulang halaman agar model terbaru terpakai.',
            ]);
        }

        $result = AbsensiApi::make()->enrollFace(
            AbsensiApi::tokenFromSession(),
            $student->id,
            $data['image_base64'],
            self::urutkanEmbedding($data['embedding']),
            $data['model_version'],
            $data['pose'],
        );

        if (! $result['success']) {
            // Halaman pengambilan menyimpan lewat AJAX agar kamera tidak mati;
            // galat harus kembali sebagai JSON, bukan redirect.
            if ($request->expectsJson()) {
                return response()->json([
                    'success' => false,
                    'message' => $result['message'],
                    'errors' => $result['errors'] ?: [],
                ], 422);
            }

            return back()->withErrors($result['errors'] ?: ['image_base64' => $result['message']]);
        }

        $payload = $result['data'] ?? [];
        $count = $payload['sample_count'] ?? 0;

        if ($request->expectsJson()) {
            return response()->json([
                'success' => true,
                'message' => $result['message'],
                'sample_count' => $count,
                'pose' => $data['pose'],
            ]);
        }

        return redirect()
            ->route('biometric.capture', $student)
            ->with('success', $result['message'])
            ->with('sample_count', $count);
    }

    /**
     * Rapikan embedding menjadi LIST float berurutan indeks.
     *
     * Dua hal yang membuat ini perlu, dan dua-duanya tidak kelihatan dari kode
     * pemanggilnya:
     *
     * 1. Kunci hasil `$request->validate()` untuk aturan `embedding.*` tidak
     *    dijamin urut angka. Bila urutannya jadi urut-string (0,1,10,100,...,11)
     *    atau ada yang bolong, `json_encode` memandangnya OBJEK, bukan array.
     *    API (Rust/serde) lalu menolak dengan
     *    "embedding: invalid type: map, expected a sequence".
     * 2. Urutan angka embedding adalah MAKNANYA. Memakai array_values() saja
     *    tanpa ksort() akan mengirim vektor yang teracak tanpa galat apa pun —
     *    pencocokan wajah jadi salah secara diam-diam. Karena itu ksort dulu.
     *
     * @param  array<int|string, mixed>  $embedding
     * @return list<float>
     */
    private static function urutkanEmbedding(array $embedding): array
    {
        ksort($embedding, SORT_NUMERIC);

        return array_values(array_map('floatval', $embedding));
    }

    public function show(Student $student): View
    {
        Tenant::authorizeSchool($student->school_id);

        return view('backend.biometric.show', [
            'student' => $student->load('classroom', 'school'),
            'samples' => $student->faceEnrollments()
                ->with('capturedBy:id,name')
                ->orderByDesc('created_at')
                ->get(),
        ]);
    }

    /**
     * Hapus SELURUH sampel wajah satu siswa, lalu pendaftaran dimulai dari
     * pose pertama lagi.
     *
     * Dipakai tombol "Ulangi dari awal". Penghapusan lewat API supaya gambar di
     * storage, vektor di pgvector, ringkasan di tabel students, dan cache index
     * tablet ikut bersih — sama seperti destroy() untuk satu sampel.
     */
    public function reset(Request $request, Student $student): RedirectResponse|JsonResponse
    {
        Tenant::authorizeSchool($student->school_id);

        $api = AbsensiApi::make();
        $token = AbsensiApi::tokenFromSession();
        $hapus = 0;
        $gagal = [];

        foreach ($student->faceEnrollments()->get() as $sampel) {
            $r = $api->deleteFaceSample($token, $sampel->id);
            if ($r['success']) {
                $hapus++;
            } else {
                $gagal[] = $r['message'];
            }
        }

        $pesan = $gagal === []
            ? "{$hapus} sampel dihapus. Pendaftaran dimulai dari pose pertama."
            : "{$hapus} sampel dihapus, ".count($gagal).' gagal: '.$gagal[0];

        if ($request->expectsJson()) {
            return response()->json([
                'success' => $gagal === [],
                'message' => $pesan,
                'deleted' => $hapus,
            ], $gagal === [] ? 200 : 422);
        }

        return $gagal === []
            ? redirect()->route('biometric.capture', $student)->with('success', $pesan)
            : back()->withErrors(['sample' => $pesan]);
    }

    /**
     * Simpan SEMUA pose sekaligus — semua berhasil, atau tidak ada yang tersimpan.
     *
     * Halaman pengambilan menahan sampel di browser sampai ketiga pose lengkap,
     * lalu mengirimnya sekali lewat sini. Alasannya: operator yang berhenti di
     * tengah tidak boleh meninggalkan wajah setengah terdaftar — data seperti itu
     * tampak "sudah ada" di dashboard tetapi tidak cukup untuk mengenali siapa
     * pun, dan tidak ada yang memberi tahu bahwa ia belum lengkap.
     *
     * Bila satu pose gagal setelah beberapa berhasil, yang sudah masuk DIHAPUS
     * kembali. API tidak punya transaksi lintas request, jadi pembatalan itu
     * dilakukan di sini secara eksplisit.
     */
    public function storeBatch(Request $request, Student $student): JsonResponse
    {
        Tenant::authorizeSchool($student->school_id);

        $data = $request->validate([
            'model_version' => ['required', 'string', 'max:40'],
            'samples' => ['required', 'array', 'size:3'],
            'samples.*.pose' => ['required', 'string', 'in:frontal,right,left'],
            'samples.*.image_base64' => ['required', 'string'],
            'samples.*.embedding' => ['required', 'array'],
            'samples.*.embedding.*' => ['numeric'],
        ], [], [
            'samples' => 'kumpulan sampel',
        ]);

        $expectedDim = (int) config('services.absensi_api.embedding_dim');
        foreach ($data['samples'] as $i => $s) {
            if (count($s['embedding']) !== $expectedDim) {
                return response()->json([
                    'success' => false,
                    'message' => "Dimensi embedding pose {$s['pose']} harus {$expectedDim}, diterima "
                        .count($s['embedding']).'. Muat ulang halaman agar model terbaru terpakai.',
                ], 422);
            }
        }

        $api = AbsensiApi::make();
        $token = AbsensiApi::tokenFromSession();
        $terpasang = [];
        $count = 0;

        // Id sampel LAMA dicatat lebih dulu: sesi ini menimpa sampel lama, tetapi
        // penghapusannya dilakukan SETELAH ketiga pose baru berhasil masuk. Urutan
        // itu penting — kalau dihapus lebih dulu lalu penyimpanan gagal, siswa
        // kehilangan data lamanya dan tidak mendapat yang baru.
        $idLama = $student->faceEnrollments()->pluck('id')->all();

        foreach ($data['samples'] as $s) {
            $result = $api->enrollFace(
                $token,
                $student->id,
                $s['image_base64'],
                self::urutkanEmbedding($s['embedding']),
                $data['model_version'],
                $s['pose'],
            );

            if (! $result['success']) {
                // Batalkan yang sudah masuk supaya tidak ada sisa separuh jalan.
                foreach ($terpasang as $id) {
                    $api->deleteFaceSample($token, $id);
                }

                return response()->json([
                    'success' => false,
                    'message' => "Pose {$s['pose']} gagal disimpan: ".$result['message']
                        .' Tidak ada sampel yang tersimpan.',
                    'errors' => $result['errors'] ?: [],
                ], 422);
            }

            $payload = $result['data'] ?? [];
            $count = $payload['sample_count'] ?? $count;
            if (! empty($payload['id'])) {
                $terpasang[] = $payload['id'];
            }
        }

        // Ketiga pose baru sudah aman: sekarang sampel lama boleh dibuang.
        $ditimpa = 0;
        foreach ($idLama as $id) {
            if ($api->deleteFaceSample($token, $id)['success']) {
                $ditimpa++;
            }
        }

        return response()->json([
            'success' => true,
            'message' => $ditimpa > 0
                ? "Ketiga pose tersimpan; {$ditimpa} sampel lama ditimpa."
                : 'Ketiga pose tersimpan.',
            'sample_count' => max(0, $count - $ditimpa),
        ]);
    }

    public function destroy(Request $request, FaceEnrollment $enrollment): RedirectResponse|JsonResponse
    {
        Tenant::authorizeSchool($enrollment->school_id);

        // Penghapusan lewat API supaya gambar di storage, vektor di pgvector,
        // ringkasan pada tabel students, dan cache index tablet ikut bersih.
        $result = AbsensiApi::make()->deleteFaceSample(
            AbsensiApi::tokenFromSession(),
            $enrollment->id
        );

        // Halaman pengambilan menghapus lewat AJAX; galat/berhasil harus JSON.
        if ($request->expectsJson()) {
            return response()->json([
                'success' => (bool) $result['success'],
                'message' => $result['success'] ? 'Sampel wajah dihapus.' : $result['message'],
            ], $result['success'] ? 200 : 422);
        }

        return $result['success']
            ? back()->with('success', 'Sampel wajah dihapus.')
            : back()->withErrors(['sample' => $result['message']]);
    }

    // =================================================================

    /**
     * @return array<string, int|float>
     */
    private function coverage(?string $schoolId): array
    {
        $row = Student::query()
            ->when($schoolId, fn ($q) => $q->where('school_id', $schoolId))
            ->where('status', 'aktif')
            ->selectRaw('
                COUNT(*) AS total,
                COUNT(*) FILTER (WHERE face_enrolled) AS enrolled,
                COUNT(*) FILTER (WHERE face_enrolled AND face_sample_count < ?) AS under_sampled
            ', [Student::RECOMMENDED_SAMPLES])
            ->first();

        $total = (int) ($row->total ?? 0);
        $enrolled = (int) ($row->enrolled ?? 0);

        return [
            'total' => $total,
            'enrolled' => $enrolled,
            'not_enrolled' => max(0, $total - $enrolled),
            'under_sampled' => (int) ($row->under_sampled ?? 0),
            'percent' => $total > 0 ? round($enrolled / $total * 100, 1) : 0.0,
        ];
    }

    private function classroomOptions(?string $schoolId)
    {
        if (! $schoolId) {
            return collect();
        }

        return Classroom::where('school_id', $schoolId)
            ->where('is_active', true)
            ->orderBy('grade_level')
            ->orderBy('name')
            ->get(['id', 'name']);
    }
}
