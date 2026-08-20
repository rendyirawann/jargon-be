<?php

namespace App\Http\Controllers\Backend\MasterData;

use App\Http\Controllers\Controller;
use App\Models\Attendance;
use App\Models\Classroom;
use App\Models\Student;
use App\Models\StudentGuardian;
use App\Support\Tenant;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Routing\Controllers\HasMiddleware;
use Illuminate\Routing\Controllers\Middleware;
use Illuminate\Support\Carbon;
use Illuminate\Support\Facades\DB;
use Illuminate\View\View;
use Yajra\DataTables\Facades\DataTables;

/**
 * Data siswa & wali murid.
 *
 * Siswa tidak punya akun. Kolom yang dikelola di sini adalah data pokok
 * (NISN/NIS, kelas, orang tua) plus kontak wali yang menjadi tujuan
 * notifikasi absensi. Pendaftaran wajah ada di FaceEnrollmentController.
 */
class StudentController extends Controller implements HasMiddleware
{
    public static function middleware(): array
    {
        return [
            'auth',
            new Middleware('can:view_student', only: ['index', 'data', 'show']),
            new Middleware('can:create_student', only: ['create', 'store']),
            new Middleware('can:update_student', only: ['edit', 'update']),
            new Middleware('can:delete_student', only: ['destroy']),
            new Middleware('can:manage_guardian', only: ['storeGuardian', 'updateGuardian', 'destroyGuardian']),
        ];
    }

    public function index(Request $request): View
    {
        $schoolId = Tenant::currentSchoolId($request->query('school_id'));

        return view('backend.master.student.index', [
            'schoolId' => $schoolId,
            'schools' => Tenant::isProvinceScope() ? Tenant::selectableSchools() : collect(),
            'classrooms' => $this->classroomOptions($schoolId),
            'statuses' => Student::STATUS,
        ]);
    }

    public function data(Request $request)
    {
        $schoolId = Tenant::currentSchoolId($request->query('school_id'));

        $query = Student::query()
            ->leftJoin('classrooms', 'classrooms.id', '=', 'students.current_classroom_id')
            ->whereNull('students.deleted_at')
            ->when($schoolId, fn ($q) => $q->where('students.school_id', $schoolId))
            ->select([
                'students.id', 'students.nisn', 'students.nis', 'students.full_name',
                'students.gender', 'students.status', 'students.face_enrolled',
                'students.face_sample_count', 'classrooms.name as classroom_name',
            ]);

        // Default hanya siswa aktif — daftar yang memuat siswa lulus/pindah
        // hampir selalu bukan yang dicari pengguna.
        $status = $request->input('status', 'aktif');
        if ($status !== 'all') {
            $query->where('students.status', $status);
        }

        $query->when($request->filled('classroom_id'),
            fn ($q) => $q->where('students.current_classroom_id', $request->classroom_id));

        if ($request->filled('face')) {
            $query->where('students.face_enrolled', $request->face === 'sudah');
        }

        return DataTables::of($query)
            ->filterColumn('full_name', function ($q, $keyword) {
                $q->where(function ($sub) use ($keyword) {
                    $sub->where('students.full_name', 'ilike', "%{$keyword}%")
                        ->orWhere('students.nis', 'ilike', "%{$keyword}%")
                        ->orWhere('students.nisn', 'ilike', "%{$keyword}%");
                });
            })
            ->addColumn('biometric', function ($row) {
                if (! $row->face_enrolled) {
                    return '<span class="badge badge-light-danger">Belum terdaftar</span>';
                }
                $complete = $row->face_sample_count >= Student::RECOMMENDED_SAMPLES;

                return '<span class="badge badge-light-'.($complete ? 'success' : 'warning').'">'
                    .$row->face_sample_count.' sampel</span>';
            })
            ->addColumn('gender_label', fn ($row) => match ($row->gender) {
                'L' => 'Laki-laki', 'P' => 'Perempuan', default => '-',
            })
            ->addColumn('action', fn ($row) => view('backend.master.student._action', ['row' => $row])->render())
            ->rawColumns(['biometric', 'action'])
            ->make(true);
    }

    public function create(Request $request): View
    {
        $schoolId = Tenant::currentSchoolId($request->query('school_id'));

        return view('backend.master.student.form', [
            'student' => new Student(['school_id' => $schoolId]),
            'schools' => Tenant::selectableSchools(),
            'classrooms' => $this->classroomOptions($schoolId),
            'guardian' => new StudentGuardian(),
        ]);
    }

    public function store(Request $request): RedirectResponse
    {
        $data = $this->validated($request);
        Tenant::authorizeSchool($data['school_id']);
        $this->assertClassroomBelongsToSchool($data['current_classroom_id'] ?? null, $data['school_id']);

        $guardian = $this->validatedGuardian($request);

        $student = DB::transaction(function () use ($data, $guardian) {
            $student = Student::create($data);

            if ($guardian !== null) {
                // Wali pertama otomatis menjadi kontak utama: tanpa kontak
                // utama, notifikasi absensi tidak punya tujuan yang jelas.
                $student->guardians()->create($guardian + [
                    'school_id' => $student->school_id,
                    'is_primary' => true,
                ]);
            }

            return $student;
        });

        return redirect()
            ->route('students.show', $student)
            ->with('success', "Siswa {$student->full_name} berhasil ditambahkan.");
    }

    public function show(Request $request, Student $student): View
    {
        Tenant::authorizeSchool($student->school_id);

        $from = Carbon::today()->subDays(29);
        $to = Carbon::today();

        return view('backend.master.student.show', [
            'student' => $student->load(['classroom', 'school', 'guardians']),
            'samples' => $student->faceEnrollments()->orderByDesc('created_at')->get(),
            'attendances' => Attendance::query()
                ->betweenDates($from->toDateString(), $to->toDateString())
                ->where('student_id', $student->id)
                ->orderByDesc('attendance_date')
                ->get(),
            'recap' => $this->recap($student, $from, $to),
            'from' => $from,
            'to' => $to,
        ]);
    }

    public function edit(Student $student): View
    {
        Tenant::authorizeSchool($student->school_id);

        return view('backend.master.student.form', [
            'student' => $student,
            'schools' => Tenant::selectableSchools(),
            'classrooms' => $this->classroomOptions($student->school_id),
            'guardian' => new StudentGuardian(),
        ]);
    }

    public function update(Request $request, Student $student): RedirectResponse
    {
        Tenant::authorizeSchool($student->school_id);

        $data = $this->validated($request, $student);
        $this->assertClassroomBelongsToSchool(
            $data['current_classroom_id'] ?? null,
            $student->school_id
        );

        // school_id tidak boleh berubah lewat form ini: memindahkan siswa
        // antar sekolah berarti memindahkan data biometrik dan riwayat
        // absensinya, dan itu harus lewat proses mutasi tersendiri.
        unset($data['school_id']);
        $student->update($data);

        return redirect()
            ->route('students.show', $student)
            ->with('success', 'Data siswa diperbarui.');
    }

    public function destroy(Student $student): RedirectResponse
    {
        Tenant::authorizeSchool($student->school_id);

        // Data biometrik dimusnahkan permanen (kewajiban perlindungan data
        // pribadi), sementara riwayat absensi tetap disimpan sebagai dokumen
        // administrasi. Penghapusan gambar & vektor dilakukan oleh API.
        $name = $student->full_name;
        DB::transaction(function () use ($student) {
            DB::table('face_embeddings')->where('student_id', $student->id)->delete();
            DB::table('face_enrollments')->where('student_id', $student->id)->delete();
            $student->update([
                'status' => 'keluar',
                'face_enrolled' => false,
                'face_sample_count' => 0,
            ]);
            $student->delete();
        });

        return redirect()
            ->route('students.index')
            ->with('success', "Siswa {$name} dihapus dan seluruh data wajahnya dimusnahkan.");
    }

    // =================================================================
    // Wali murid
    // =================================================================

    public function storeGuardian(Request $request, Student $student): RedirectResponse
    {
        Tenant::authorizeSchool($student->school_id);

        $data = $this->validatedGuardian($request, true);

        DB::transaction(function () use ($student, $data) {
            if (! empty($data['is_primary'])) {
                $student->guardians()->update(['is_primary' => false]);
            }
            $student->guardians()->create($data + ['school_id' => $student->school_id]);
        });

        return back()->with('success', 'Wali murid ditambahkan.');
    }

    public function updateGuardian(Request $request, Student $student, StudentGuardian $guardian): RedirectResponse
    {
        Tenant::authorizeSchool($student->school_id);
        abort_unless($guardian->student_id === $student->id, 404);

        $data = $this->validatedGuardian($request, true);

        DB::transaction(function () use ($student, $guardian, $data) {
            if (! empty($data['is_primary'])) {
                $student->guardians()->where('id', '!=', $guardian->id)->update(['is_primary' => false]);
            }
            $guardian->update($data);
        });

        return back()->with('success', 'Data wali murid diperbarui.');
    }

    public function destroyGuardian(Student $student, StudentGuardian $guardian): RedirectResponse
    {
        Tenant::authorizeSchool($student->school_id);
        abort_unless($guardian->student_id === $student->id, 404);

        $guardian->delete();

        return back()->with('success', 'Wali murid dihapus.');
    }

    /**
     * Dropdown kelas mengikuti sekolah terpilih (dipakai form dinamis).
     */
    public function classroomsBySchool(Request $request): JsonResponse
    {
        $schoolId = $request->query('school_id');
        Tenant::authorizeSchool($schoolId);

        return response()->json($this->classroomOptions($schoolId)->map(fn ($c) => [
            'id' => $c->id,
            'text' => $c->name,
        ])->values());
    }

    // =================================================================

    private function classroomOptions(?string $schoolId)
    {
        if (! $schoolId) {
            return collect();
        }

        return Classroom::query()
            ->where('school_id', $schoolId)
            ->where('is_active', true)
            ->currentYear()
            ->orderBy('grade_level')
            ->orderBy('name')
            ->get(['id', 'name', 'grade_level']);
    }

    private function assertClassroomBelongsToSchool(?string $classroomId, ?string $schoolId): void
    {
        if (! $classroomId || ! $schoolId) {
            return;
        }

        $ok = Classroom::withoutTenantScope()
            ->where('id', $classroomId)
            ->where('school_id', $schoolId)
            ->whereNull('deleted_at')
            ->exists();

        abort_unless($ok, 422, 'Kelas yang dipilih bukan milik sekolah ini.');
    }

    /**
     * @return array<string, mixed>
     */
    private function validated(Request $request, ?Student $student = null): array
    {
        $schoolId = $student?->school_id ?? $request->input('school_id');

        $rules = [
            'school_id' => ['required', 'exists:schools,id'],
            'current_classroom_id' => ['nullable', 'exists:classrooms,id'],
            // NISN unik nasional; NIS unik per sekolah.
            'nisn' => [
                'nullable', 'digits:10',
                'unique:students,nisn'.($student ? ",{$student->id}" : ''),
            ],
            'nis' => ['nullable', 'string', 'max:20'],
            'full_name' => ['required', 'string', 'min:2', 'max:150'],
            'gender' => ['nullable', 'in:L,P'],
            'birth_place' => ['nullable', 'string', 'max:100'],
            'birth_date' => ['nullable', 'date', 'before:today'],
            'religion' => ['nullable', 'string', 'max:20'],
            'address' => ['nullable', 'string', 'max:500'],
            'phone' => ['nullable', 'string', 'max:20'],
            'father_name' => ['nullable', 'string', 'max:150'],
            'mother_name' => ['nullable', 'string', 'max:150'],
            'status' => ['nullable', 'in:'.implode(',', Student::STATUS)],
            'entry_year' => ['nullable', 'integer', 'between:1990,2100'],
        ];

        $data = $request->validate($rules, [], [
            'full_name' => 'nama lengkap',
            'nisn' => 'NISN',
            'nis' => 'NIS',
            'current_classroom_id' => 'kelas',
        ]);

        if (! empty($data['nis']) && $schoolId) {
            $duplicate = Student::withoutTenantScope()
                ->where('school_id', $schoolId)
                ->where('nis', $data['nis'])
                ->whereNull('deleted_at')
                ->when($student, fn ($q) => $q->where('id', '!=', $student->id))
                ->exists();

            if ($duplicate) {
                abort(422, "NIS {$data['nis']} sudah dipakai siswa lain di sekolah ini.");
            }
        }

        $data['status'] ??= 'aktif';

        return $data;
    }

    /**
     * @return array<string, mixed>|null
     */
    private function validatedGuardian(Request $request, bool $required = false): ?array
    {
        $prefix = $required ? '' : 'guardian_';

        if (! $required && ! $request->filled($prefix.'full_name')) {
            return null;
        }

        $rules = [
            $prefix.'relation' => ['required', 'in:'.implode(',', StudentGuardian::RELATIONS)],
            $prefix.'full_name' => ['required', 'string', 'min:2', 'max:150'],
            $prefix.'phone' => ['nullable', 'string', 'max:20'],
            $prefix.'whatsapp' => ['nullable', 'string', 'max:20'],
            $prefix.'email' => ['nullable', 'email', 'max:150'],
            $prefix.'telegram_chat_id' => ['nullable', 'string', 'max:40'],
            $prefix.'preferred_channel' => ['required', 'in:'.implode(',', StudentGuardian::CHANNELS)],
            $prefix.'is_primary' => ['nullable', 'boolean'],
            $prefix.'notify_enabled' => ['nullable', 'boolean'],
        ];

        $validated = $request->validate($rules, [], [
            $prefix.'full_name' => 'nama wali',
            $prefix.'preferred_channel' => 'kanal notifikasi',
        ]);

        // Buang prefix agar bisa langsung dipakai untuk mass-assignment.
        $data = [];
        foreach ($validated as $key => $value) {
            $data[$prefix === '' ? $key : substr($key, strlen($prefix))] = $value;
        }

        $data['notify_enabled'] = (bool) ($data['notify_enabled'] ?? true);
        $data['is_primary'] = (bool) ($data['is_primary'] ?? false);

        // Kanal yang dipilih harus punya kontaknya, kalau tidak notifikasi
        // akan selalu gagal dan baru ketahuan saat orang tua mengeluh.
        $missing = match ($data['preferred_channel']) {
            'whatsapp' => empty($data['whatsapp']) && empty($data['phone']),
            'telegram' => empty($data['telegram_chat_id']),
            'email' => empty($data['email']),
            default => false,
        };

        if ($missing) {
            abort(422, "Kanal {$data['preferred_channel']} dipilih, tetapi kontaknya belum diisi.");
        }

        return $data;
    }

    /**
     * @return array<string, int>
     */
    private function recap(Student $student, Carbon $from, Carbon $to): array
    {
        $row = Attendance::query()
            ->betweenDates($from->toDateString(), $to->toDateString())
            ->where('student_id', $student->id)
            ->selectRaw("
                COUNT(*) FILTER (WHERE status = 'hadir')     AS hadir,
                COUNT(*) FILTER (WHERE status = 'terlambat') AS terlambat,
                COUNT(*) FILTER (WHERE status = 'izin')      AS izin,
                COUNT(*) FILTER (WHERE status = 'sakit')     AS sakit,
                COUNT(*) FILTER (WHERE status = 'alfa')      AS alfa,
                COALESCE(SUM(late_minutes), 0)               AS total_late
            ")
            ->first();

        return [
            'hadir' => (int) ($row->hadir ?? 0),
            'terlambat' => (int) ($row->terlambat ?? 0),
            'izin' => (int) ($row->izin ?? 0),
            'sakit' => (int) ($row->sakit ?? 0),
            'alfa' => (int) ($row->alfa ?? 0),
            'total_late' => (int) ($row->total_late ?? 0),
        ];
    }
}
