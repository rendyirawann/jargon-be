<?php

namespace App\Http\Controllers\Backend\Biometric;

use App\Http\Controllers\Controller;
use App\Models\Classroom;
use App\Models\FaceEnrollment;
use App\Models\Student;
use App\Services\AbsensiApi;
use App\Support\Tenant;
use Illuminate\Http\RedirectResponse;
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
            new Middleware('can:delete_face_enrollment', only: ['destroy']),
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
     * Terima gambar + embedding dari browser lalu teruskan ke API.
     */
    public function store(Request $request, Student $student): RedirectResponse
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
            array_map('floatval', $data['embedding']),
            $data['model_version'],
            $data['pose'],
        );

        if (! $result['success']) {
            return back()->withErrors($result['errors'] ?: ['image_base64' => $result['message']]);
        }

        $payload = $result['data'] ?? [];
        $count = $payload['sample_count'] ?? 0;

        return redirect()
            ->route('biometric.capture', $student)
            ->with('success', $result['message'])
            ->with('sample_count', $count);
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

    public function destroy(FaceEnrollment $enrollment): RedirectResponse
    {
        Tenant::authorizeSchool($enrollment->school_id);

        // Penghapusan lewat API supaya gambar di storage, vektor di pgvector,
        // ringkasan pada tabel students, dan cache index tablet ikut bersih.
        $result = AbsensiApi::make()->deleteFaceSample(
            AbsensiApi::tokenFromSession(),
            $enrollment->id
        );

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
