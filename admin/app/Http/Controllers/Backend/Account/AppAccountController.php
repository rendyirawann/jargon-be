<?php

namespace App\Http\Controllers\Backend\Account;

use App\Http\Controllers\Controller;
use App\Models\Classroom;
use App\Models\Student;
use App\Models\User;
use App\Services\AbsensiApi;
use App\Support\Tenant;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Routing\Controllers\HasMiddleware;
use Illuminate\Routing\Controllers\Middleware;
use Illuminate\View\View;
use Spatie\Permission\Models\Role;

/**
 * Akun aplikasi Jargon GO.
 *
 * Pendaftaran akun TIDAK swalayan — seluruhnya dilakukan admin dari sini.
 * Alasannya sederhana: yang boleh melihat absensi seorang siswa hanya siswa
 * itu dan orang tuanya, dan tidak ada cara memverifikasi hubungan itu dari
 * formulir pendaftaran mandiri. Verifikasinya dilakukan sekolah, di sini.
 *
 * Halaman ini terpisah dari `/admin/users` bawaan starter. Yang di sana
 * mengelola akun dashboard (username + email); yang di sini mengelola akun
 * APLIKASI, yang identitas loginnya NIK atau NISN, dan yang untuk orang tua
 * harus ditautkan ke anaknya. Menggabungkan keduanya menghasilkan satu
 * formulir dengan separuh isian yang selalu tidak relevan.
 *
 * Semua penulisan diteruskan ke API: aturan panjang NIK/NISN, pewarisan
 * sekolah dari data siswa, dan pencabutan sesi saat tautan diputus hanya ada
 * di satu tempat.
 */
class AppAccountController extends Controller implements HasMiddleware
{
    /** Peran yang memakai aplikasi Jargon GO. */
    public const APP_ROLES = [
        'siswa' => 'Siswa',
        'orang_tua' => 'Orang Tua',
        'guru' => 'Guru',
        'staff_tu' => 'Staff TU',
        'kepala_sekolah' => 'Kepala Sekolah',
        'petugas_pengaduan' => 'Petugas Pengaduan',
    ];

    public const RELATIONS = ['ayah' => 'Ayah', 'ibu' => 'Ibu', 'wali' => 'Wali'];

    public static function middleware(): array
    {
        return [
            'auth',
            new Middleware('can:manage_app_account'),
        ];
    }

    public function index(Request $request): View
    {
        $query = User::query()
            ->whereNull('deleted_at')
            ->whereNotNull('identity_number')
            ->with(['roles:id,name', 'school:id,name', 'student:id,full_name']);

        // Cakupan tenant. Akun orang tua tidak punya school_id (anaknya bisa
        // beda sekolah), jadi disaring lewat tautan anaknya.
        $allowed = Tenant::schoolIds();
        if ($allowed !== null) {
            $ids = $allowed ?: ['00000000-0000-0000-0000-000000000000'];
            $query->where(function ($q) use ($ids) {
                $q->whereIn('users.school_id', $ids)
                    ->orWhereExists(function ($sub) use ($ids) {
                        $sub->selectRaw('1')
                            ->from('student_guardians as g')
                            ->whereColumn('g.user_id', 'users.id')
                            ->whereIn('g.school_id', $ids);
                    });
            });
        }

        $query->when($request->filled('role'), fn ($q) => $q->whereHas(
            'roles',
            fn ($r) => $r->where('name', $request->role)
        ));

        $query->when($request->filled('q'), function ($q) use ($request) {
            $term = trim($request->q);
            $q->where(function ($sub) use ($term) {
                $sub->where('name', 'ILIKE', "%{$term}%")
                    ->orWhere('identity_number', 'ILIKE', "%{$term}%");
            });
        });

        $query->when(
            $request->boolean('belum_ganti_sandi'),
            fn ($q) => $q->where('must_change_password', true)
        );

        return view('backend.account.index', [
            'accounts' => $query->orderByDesc('created_at')->paginate(25)->withQueryString(),
            'roles' => self::APP_ROLES,
            'stats' => $this->stats($allowed),
        ]);
    }

    public function create(): View
    {
        return view('backend.account.create', [
            'roles' => $this->assignableRoles(),
            'schools' => Tenant::selectableSchools(),
            'relations' => self::RELATIONS,
        ]);
    }

    public function store(Request $request): RedirectResponse
    {
        $data = $request->validate([
            'name' => ['required', 'string', 'min:3', 'max:150'],
            'role' => ['required', 'in:'.implode(',', array_keys(self::APP_ROLES))],
            'identity_number' => ['required', 'string', 'regex:/^[0-9]+$/'],
            'email' => ['required', 'email', 'max:255'],
            'password' => ['required', 'string', 'min:8'],
            'school_id' => ['nullable', 'uuid'],
            'student_ids' => ['array'],
            'student_ids.*' => ['uuid'],
            'guardian_relation' => ['nullable', 'in:'.implode(',', array_keys(self::RELATIONS))],
            'employee_no' => ['nullable', 'string', 'max:30'],
            'position' => ['nullable', 'string', 'max:100'],
            'phone' => ['nullable', 'string', 'max:15'],
        ], [], [
            'identity_number' => 'NIK/NISN',
            'student_ids' => 'tautan siswa',
        ]);

        // Panjang identitas diperiksa di sini juga agar operator mendapat
        // pesan sebelum permintaan menyeberang ke API — bukan pengganti
        // pemeriksaan di sana, tetapi umpan balik yang lebih cepat.
        $expected = $data['role'] === 'siswa' ? 10 : 16;
        $label = $expected === 10 ? 'NISN' : 'NIK';
        if (strlen($data['identity_number']) !== $expected) {
            return back()->withInput()->withErrors([
                'identity_number' => "Peran {$data['role']} login memakai {$label} ({$expected} digit).",
            ]);
        }

        Tenant::authorizeSchool($data['school_id'] ?? null);

        $payload = [
            'name' => $data['name'],
            // Username teknis; pengguna aplikasi tetap login memakai NIK/NISN.
            'username' => $this->technicalUsername($data['role'], $data['identity_number']),
            'email' => $data['email'],
            'password' => $data['password'],
            'identity_number' => $data['identity_number'],
            'role' => $data['role'],
            'school_id' => $data['school_id'] ?: null,
            'student_ids' => array_values($data['student_ids'] ?? []),
            'guardian_relation' => $data['guardian_relation'] ?? null,
            'employee_no' => $data['employee_no'] ?? null,
            'position' => $data['position'] ?? null,
            'phone' => $data['phone'] ?? null,
        ];

        $result = AbsensiApi::make()->call('POST', '/v1/users', $payload);

        if (! $result['success']) {
            return back()->withInput()->withErrors(
                $result['errors'] ?: ['name' => $result['message']]
            );
        }

        return redirect()
            ->route('app-accounts.index')
            ->with('success', $result['message']);
    }

    public function show(Request $request, string $id): View
    {
        $account = User::query()
            ->whereNull('deleted_at')
            ->with([
                'roles:id,name',
                'school:id,name',
                'student.classroom:id,name',
                'children.school:id,name',
                'children.classroom:id,name',
            ])
            ->findOrFail($id);

        $this->authorizeAccount($account);

        return view('backend.account.show', [
            'account' => $account,
            'relations' => self::RELATIONS,
            'isParent' => $account->hasRole('orang_tua'),
        ]);
    }

    /** Tautkan seorang anak ke akun orang tua. */
    public function linkChild(Request $request, string $id): RedirectResponse
    {
        $data = $request->validate([
            'student_id' => ['required', 'uuid'],
            'relation' => ['required', 'in:'.implode(',', array_keys(self::RELATIONS))],
        ], [], ['student_id' => 'siswa']);

        $result = AbsensiApi::make()->call('POST', "/v1/users/{$id}/children", $data);

        return $result['success']
            ? back()->with('success', $result['message'])
            : back()->withErrors(['student_id' => $result['message']]);
    }

    /** Putuskan tautan seorang anak dari akun orang tua. */
    public function unlinkChild(Request $request, string $id, string $studentId): RedirectResponse
    {
        $result = AbsensiApi::make()->call('DELETE', "/v1/users/{$id}/children/{$studentId}");

        return $result['success']
            ? back()->with('success', $result['message'])
            : back()->withErrors(['student_id' => $result['message']]);
    }

    /**
     * Pencarian siswa untuk formulir tautan.
     *
     * Selalu dibatasi cakupan tenant: tanpa itu, mengetik nama di kotak
     * pencarian akan membocorkan daftar siswa seluruh provinsi.
     */
    public function searchStudents(Request $request)
    {
        $term = trim((string) $request->query('q', ''));
        if (strlen($term) < 3) {
            return response()->json(['data' => []]);
        }

        $query = Student::query()
            ->with(['school:id,name', 'classroom:id,name'])
            ->active()
            ->where(function ($q) use ($term) {
                $q->where('full_name', 'ILIKE', "%{$term}%")
                    ->orWhere('nisn', 'ILIKE', "{$term}%");
            });

        $allowed = Tenant::schoolIds();
        if ($allowed !== null) {
            $query->whereIn('school_id', $allowed ?: ['00000000-0000-0000-0000-000000000000']);
        }

        return response()->json([
            'data' => $query->orderBy('full_name')->limit(20)->get()->map(fn ($s) => [
                'id' => $s->id,
                'name' => $s->full_name,
                'nisn' => $s->nisn,
                'classroom' => $s->classroom->name ?? '-',
                'school' => $s->school->name ?? '-',
            ]),
        ]);
    }

    /** Formulir pembuatan akun siswa massal per kelas. */
    public function bulk(Request $request): View
    {
        $schoolId = Tenant::currentSchoolId($request->query('school_id'));

        $classrooms = collect();
        $pending = collect();

        if ($schoolId) {
            $classrooms = Classroom::where('school_id', $schoolId)
                ->where('is_active', true)
                ->orderBy('name')
                ->get(['id', 'name']);

            // Siswa aktif yang belum punya akun — ini yang akan dibuatkan.
            $pending = Student::query()
                ->active()
                ->where('school_id', $schoolId)
                ->whereDoesntHave('appAccount')
                ->when(
                    $request->filled('classroom_id'),
                    fn ($q) => $q->where('current_classroom_id', $request->classroom_id)
                )
                ->with('classroom:id,name')
                ->orderBy('full_name')
                ->limit(500)
                ->get(['id', 'full_name', 'nisn', 'current_classroom_id']);
        }

        return view('backend.account.bulk', [
            'schools' => Tenant::selectableSchools(),
            'schoolId' => $schoolId,
            'classrooms' => $classrooms,
            'pending' => $pending,
            // Kredensial hanya muncul sekali, dari flash session hasil POST.
            'credentials' => session('credentials', []),
            'notes' => session('credential_notes', []),
        ]);
    }

    public function bulkStore(Request $request): RedirectResponse
    {
        $data = $request->validate([
            'school_id' => ['required', 'uuid'],
            'classroom_id' => ['nullable', 'uuid'],
            'limit' => ['nullable', 'integer', 'between:1,1000'],
        ]);

        Tenant::authorizeSchool($data['school_id']);

        $result = AbsensiApi::make()->call('POST', '/v1/users/students/bulk', [
            'school_id' => $data['school_id'],
            'classroom_id' => $data['classroom_id'] ?: null,
            'skip_existing' => true,
            'limit' => $data['limit'] ?? 200,
        ]);

        if (! $result['success']) {
            return back()->withErrors(['school_id' => $result['message']]);
        }

        $payload = $result['data'] ?? [];

        // Kata sandi awal dikembalikan API SEKALI dan tidak pernah tersimpan
        // dalam bentuk terbaca. Diletakkan di flash session, bukan di tabel
        // apa pun — operator harus mencetaknya saat itu juga.
        return back()
            ->with('success', $result['message'])
            ->with('credentials', $payload['credentials'] ?? [])
            ->with('credential_notes', $payload['notes'] ?? []);
    }

    // =================================================================

    private function authorizeAccount(User $account): void
    {
        $allowed = Tenant::schoolIds();
        if ($allowed === null) {
            return;
        }

        $schools = collect([$account->school_id])
            ->merge($account->children->pluck('school_id'))
            ->filter()
            ->all();

        if (empty(array_intersect($schools, $allowed))) {
            abort(403, 'Akun ini berada di luar cakupan sekolah Anda.');
        }
    }

    /**
     * Peran yang boleh diberikan pengguna aktif.
     *
     * Peran tingkat provinsi tidak muncul bagi pengguna sekolah — kalau tidak,
     * seorang staff TU bisa mengangkat dirinya menjadi petugas dinas.
     *
     * @return array<string, string>
     */
    private function assignableRoles(): array
    {
        $roles = self::APP_ROLES;

        if (! Tenant::isProvinceScope()) {
            unset($roles['petugas_pengaduan']);
        }

        // Peran yang belum ada di database disembunyikan agar formulir tidak
        // menawarkan pilihan yang pasti gagal.
        $existing = Role::where('guard_name', 'web')->pluck('name')->all();

        return array_intersect_key($roles, array_flip($existing));
    }

    private function technicalUsername(string $role, string $identity): string
    {
        $prefix = match ($role) {
            'siswa' => 'siswa',
            'orang_tua' => 'ortu',
            default => 'user',
        };

        return $prefix.$identity;
    }

    /**
     * @param  array<int, string>|null  $allowed
     * @return array<string, int>
     */
    private function stats(?array $allowed): array
    {
        $base = fn () => User::query()
            ->whereNull('deleted_at')
            ->whereNotNull('identity_number')
            ->when($allowed !== null, function ($q) use ($allowed) {
                $ids = $allowed ?: ['00000000-0000-0000-0000-000000000000'];
                $q->where(function ($sub) use ($ids) {
                    $sub->whereIn('users.school_id', $ids)
                        ->orWhereExists(function ($ex) use ($ids) {
                            $ex->selectRaw('1')
                                ->from('student_guardians as g')
                                ->whereColumn('g.user_id', 'users.id')
                                ->whereIn('g.school_id', $ids);
                        });
                });
            });

        $byRole = fn (string $role) => (clone $base())
            ->whereHas('roles', fn ($r) => $r->where('name', $role))
            ->count();

        return [
            'total' => $base()->count(),
            'siswa' => $byRole('siswa'),
            'orang_tua' => $byRole('orang_tua'),
            'belum_ganti_sandi' => $base()->where('must_change_password', true)->count(),
        ];
    }
}
