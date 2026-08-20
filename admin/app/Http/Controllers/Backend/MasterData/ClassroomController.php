<?php

namespace App\Http\Controllers\Backend\MasterData;

use App\Http\Controllers\Controller;
use App\Models\AcademicYear;
use App\Models\Classroom;
use App\Models\User;
use App\Support\Tenant;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Routing\Controllers\HasMiddleware;
use Illuminate\Routing\Controllers\Middleware;
use Illuminate\View\View;

/** Rombongan belajar (kelas) beserta wali kelasnya. */
class ClassroomController extends Controller implements HasMiddleware
{
    public static function middleware(): array
    {
        return [
            'auth',
            new Middleware('can:view_classroom', only: ['index']),
            new Middleware('can:create_classroom', only: ['store']),
            new Middleware('can:update_classroom', only: ['update']),
            new Middleware('can:delete_classroom', only: ['destroy']),
        ];
    }

    public function index(Request $request): View
    {
        $schoolId = Tenant::currentSchoolId($request->query('school_id'));
        $activeYear = AcademicYear::active();

        $classrooms = collect();
        if ($schoolId) {
            $classrooms = Classroom::query()
                ->where('school_id', $schoolId)
                ->when($activeYear, fn ($q) => $q->where('academic_year_id', $activeYear->id))
                ->with(['homeroomTeacher:id,name', 'academicYear:id,name'])
                ->withCount(['students' => fn ($q) => $q->where('status', 'aktif')])
                ->orderBy('grade_level')
                ->orderBy('name')
                ->get();
        }

        return view('backend.master.classroom.index', [
            'classrooms' => $classrooms,
            'schoolId' => $schoolId,
            'schools' => Tenant::isProvinceScope() ? Tenant::selectableSchools() : collect(),
            'activeYear' => $activeYear,
            'teachers' => $this->teacherOptions($schoolId),
        ]);
    }

    public function store(Request $request): RedirectResponse
    {
        $data = $this->validated($request);
        Tenant::authorizeSchool($data['school_id']);

        $year = AcademicYear::active();
        abort_if($year === null, 422, 'Belum ada tahun ajaran aktif. Aktifkan terlebih dahulu.');

        $this->assertTeacherInSchool($data['homeroom_teacher_id'] ?? null, $data['school_id']);

        $exists = Classroom::withoutTenantScope()
            ->where('school_id', $data['school_id'])
            ->where('academic_year_id', $year->id)
            ->where('name', $data['name'])
            ->whereNull('deleted_at')
            ->exists();
        abort_if($exists, 422, "Kelas {$data['name']} sudah ada pada tahun ajaran {$year->name}.");

        Classroom::create($data + ['academic_year_id' => $year->id]);

        return back()->with('success', "Kelas {$data['name']} berhasil dibuat.");
    }

    public function update(Request $request, Classroom $classroom): RedirectResponse
    {
        Tenant::authorizeSchool($classroom->school_id);

        $data = $this->validated($request, $classroom);
        $this->assertTeacherInSchool($data['homeroom_teacher_id'] ?? null, $classroom->school_id);

        // school_id tidak boleh berpindah: siswa di dalamnya akan menjadi
        // yatim tenant dan hilang dari daftar kedua sekolah.
        unset($data['school_id']);
        $classroom->update($data);

        return back()->with('success', 'Data kelas diperbarui.');
    }

    public function destroy(Classroom $classroom): RedirectResponse
    {
        Tenant::authorizeSchool($classroom->school_id);

        // Memindahkan siswa adalah keputusan operator, bukan efek samping
        // penghapusan kelas.
        $active = $classroom->students()->where('status', 'aktif')->count();
        abort_if(
            $active > 0,
            422,
            "Kelas {$classroom->name} masih memiliki {$active} siswa aktif. Pindahkan siswa terlebih dahulu."
        );

        $classroom->update(['is_active' => false]);
        $classroom->delete();

        return back()->with('success', "Kelas {$classroom->name} dihapus.");
    }

    /**
     * @return array<string, mixed>
     */
    private function validated(Request $request, ?Classroom $classroom = null): array
    {
        return $request->validate([
            'school_id' => [$classroom ? 'nullable' : 'required', 'exists:schools,id'],
            'name' => ['required', 'string', 'max:60'],
            'grade_level' => ['required', 'integer', 'between:1,13'],
            'major' => ['nullable', 'string', 'max:60'],
            'homeroom_teacher_id' => ['nullable', 'exists:users,id'],
            'capacity' => ['nullable', 'integer', 'between:1,100'],
            'is_active' => ['nullable', 'boolean'],
        ], [], [
            'name' => 'nama kelas',
            'grade_level' => 'tingkat',
            'homeroom_teacher_id' => 'wali kelas',
        ]);
    }

    /**
     * Wali kelas harus pegawai sekolah yang sama — mencegah operator satu
     * sekolah menautkan guru sekolah lain, yang lalu bisa membaca data siswa.
     */
    private function assertTeacherInSchool(?string $teacherId, string $schoolId): void
    {
        if (! $teacherId) {
            return;
        }

        $ok = User::where('id', $teacherId)
            ->whereNull('deleted_at')
            ->where('is_active', true)
            ->where(fn ($q) => $q->where('school_id', $schoolId)
                ->orWhereHas('extraSchools', fn ($s) => $s->where('schools.id', $schoolId)))
            ->exists();

        abort_unless($ok, 422, 'Guru yang dipilih bukan pegawai sekolah ini.');
    }

    private function teacherOptions(?string $schoolId)
    {
        if (! $schoolId) {
            return collect();
        }

        return User::query()
            ->where('school_id', $schoolId)
            ->whereNull('deleted_at')
            ->where('is_active', true)
            ->orderBy('name')
            ->get(['id', 'name']);
    }
}
