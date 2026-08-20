<?php

namespace App\Http\Controllers\Backend\MasterData;

use App\Http\Controllers\Controller;
use App\Models\NotificationPolicy;
use App\Models\Region;
use App\Models\School;
use App\Support\Tenant;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Routing\Controllers\HasMiddleware;
use Illuminate\Routing\Controllers\Middleware;
use Illuminate\Support\Str;
use Illuminate\View\View;
use Yajra\DataTables\Facades\DataTables;

/**
 * Master sekolah.
 *
 * Membuat/menghapus sekolah adalah wewenang tingkat provinsi. Pengguna
 * tingkat sekolah tetap boleh membuka detail sekolahnya (untuk mengatur
 * ambang pengenalan wajah dan melihat identitas sekolah pada laporan).
 */
class SchoolController extends Controller implements HasMiddleware
{
    public static function middleware(): array
    {
        return [
            'auth',
            new Middleware('can:view_school', only: ['index', 'data', 'show']),
            new Middleware('can:create_school', only: ['create', 'store']),
            new Middleware('can:update_school', only: ['edit', 'update']),
            new Middleware('can:delete_school', only: ['destroy']),
        ];
    }

    public function index(): View
    {
        return view('backend.master.school.index', [
            'regions' => Region::orderBy('name')->get(['id', 'name']),
            'jenjangList' => School::JENJANG,
        ]);
    }

    public function data(Request $request)
    {
        $query = School::query()
            ->accessible()
            ->leftJoin('regions', 'regions.id', '=', 'schools.region_id')
            ->whereNull('schools.deleted_at')
            ->select([
                'schools.id', 'schools.npsn', 'schools.name', 'schools.jenjang',
                'schools.status', 'schools.is_active', 'regions.name as region_name',
            ])
            // Hitungan dibuat sebagai sub-select, bukan JOIN + GROUP BY: pada
            // tabel siswa berisi ratusan ribu baris, sub-select per sekolah
            // jauh lebih murah dan tetap bisa diurutkan oleh DataTables.
            ->selectSub(
                'SELECT COUNT(*) FROM students st WHERE st.school_id = schools.id
                 AND st.deleted_at IS NULL AND st.status = \'aktif\'',
                'student_count'
            )
            ->selectSub(
                'SELECT COUNT(*) FROM students st WHERE st.school_id = schools.id
                 AND st.deleted_at IS NULL AND st.status = \'aktif\' AND st.face_enrolled',
                'enrolled_count'
            )
            ->selectSub(
                'SELECT COUNT(*) FROM devices d WHERE d.school_id = schools.id
                 AND d.deleted_at IS NULL AND d.is_active',
                'device_count'
            );

        $query->when($request->filled('jenjang'), fn ($q) => $q->where('schools.jenjang', $request->jenjang));
        $query->when($request->filled('region_id'), fn ($q) => $q->where('schools.region_id', $request->region_id));
        $query->when($request->filled('status'), fn ($q) => $q->where('schools.status', $request->status));

        return DataTables::of($query)
            ->filterColumn('name', function ($q, $keyword) {
                $q->where(function ($sub) use ($keyword) {
                    $sub->where('schools.name', 'ilike', "%{$keyword}%")
                        ->orWhere('schools.npsn', 'ilike', "%{$keyword}%");
                });
            })
            ->addColumn('coverage', function ($row) {
                if ($row->student_count == 0) {
                    return '<span class="text-muted">-</span>';
                }
                $pct = round($row->enrolled_count / $row->student_count * 100, 1);
                $class = $pct >= 90 ? 'success' : ($pct >= 50 ? 'warning' : 'danger');

                return '<span class="badge badge-light-'.$class.'">'.$pct.'%</span>'
                    .'<div class="text-muted fs-8">'.$row->enrolled_count.' / '.$row->student_count.'</div>';
            })
            ->addColumn('status_badge', fn ($row) => $row->is_active
                ? '<span class="badge badge-light-success">Aktif</span>'
                : '<span class="badge badge-light-danger">Nonaktif</span>')
            ->addColumn('action', fn ($row) => view('backend.master.school._action', ['row' => $row])->render())
            ->rawColumns(['coverage', 'status_badge', 'action'])
            ->make(true);
    }

    public function create(): View
    {
        return view('backend.master.school.form', [
            'school' => new School(),
            'regions' => Region::orderBy('name')->get(['id', 'name']),
        ]);
    }

    public function store(Request $request): RedirectResponse
    {
        $data = $this->validated($request);

        $school = new School($data);
        $school->slug = $this->makeSlug($data['name'], $data['npsn']);
        $school->save();

        // Sekolah baru langsung punya kebijakan notifikasi default supaya bisa
        // dipakai tanpa konfigurasi tambahan.
        NotificationPolicy::forSchool($school->id);

        return redirect()
            ->route('schools.index')
            ->with('success', "Sekolah {$school->name} berhasil ditambahkan.");
    }

    public function show(School $school): View
    {
        Tenant::authorizeSchool($school->id);

        return view('backend.master.school.show', [
            'school' => $school->load('region'),
            'stats' => [
                'students' => $school->students()->where('status', 'aktif')->count(),
                'enrolled' => $school->students()->where('status', 'aktif')->where('face_enrolled', true)->count(),
                'classrooms' => $school->classrooms()->where('is_active', true)->count(),
                'devices' => $school->devices()->where('is_active', true)->count(),
            ],
        ]);
    }

    public function edit(School $school): View
    {
        Tenant::authorizeSchool($school->id);

        return view('backend.master.school.form', [
            'school' => $school,
            'regions' => Region::orderBy('name')->get(['id', 'name']),
        ]);
    }

    public function update(Request $request, School $school): RedirectResponse
    {
        Tenant::authorizeSchool($school->id);

        $school->update($this->validated($request, $school->id));

        return redirect()
            ->route('schools.index')
            ->with('success', "Data {$school->name} diperbarui.");
    }

    public function destroy(School $school): RedirectResponse
    {
        // Soft delete: riwayat absensi tahun-tahun sebelumnya harus tetap bisa
        // dibuka, jadi sekolah hanya diarsipkan dan perangkatnya dimatikan.
        $school->devices()->update(['is_active' => false, 'token_revoked_at' => now()]);
        $school->update(['is_active' => false]);
        $school->delete();

        return redirect()
            ->route('schools.index')
            ->with('success', "Sekolah {$school->name} diarsipkan. Data absensi historis tetap tersimpan.");
    }

    /**
     * @return array<string, mixed>
     */
    private function validated(Request $request, ?string $ignoreId = null): array
    {
        return $request->validate([
            'npsn' => [
                'required', 'string', 'min:6', 'max:12',
                'unique:schools,npsn'.($ignoreId ? ",{$ignoreId}" : ''),
            ],
            'name' => ['required', 'string', 'min:3', 'max:200'],
            'jenjang' => ['required', 'in:'.implode(',', School::JENJANG)],
            'status' => ['required', 'in:'.implode(',', School::STATUS)],
            'region_id' => ['nullable', 'exists:regions,id'],
            'address' => ['nullable', 'string', 'max:500'],
            'village' => ['nullable', 'string', 'max:120'],
            'district' => ['nullable', 'string', 'max:120'],
            'postal_code' => ['nullable', 'string', 'max:10'],
            // Batas koordinat wilayah Indonesia — salah ketik yang membuat
            // geofence berada di benua lain akan langsung ketahuan.
            'latitude' => ['nullable', 'numeric', 'between:-11,6'],
            'longitude' => ['nullable', 'numeric', 'between:95,141'],
            'geofence_radius_m' => ['nullable', 'integer', 'between:20,5000'],
            'phone' => ['nullable', 'string', 'max:30'],
            'email' => ['nullable', 'email', 'max:150'],
            'principal_name' => ['nullable', 'string', 'max:150'],
            // Ambang terlalu rendah = orang lain bisa dikenali sebagai siswa;
            // terlalu tinggi = siswa sah selalu gagal absen.
            'face_match_threshold' => ['nullable', 'numeric', 'between:0.3,0.99'],
            'is_active' => ['nullable', 'boolean'],
        ], [], [
            'npsn' => 'NPSN',
            'name' => 'nama sekolah',
            'face_match_threshold' => 'ambang kemiripan wajah',
        ]);
    }

    private function makeSlug(string $name, string $npsn): string
    {
        // NPSN disertakan karena nama sekolah sering sama persis
        // ("SD Negeri 1") di kabupaten berbeda.
        return Str::slug(Str::limit($name, 180, '')).'-'.trim($npsn);
    }
}
