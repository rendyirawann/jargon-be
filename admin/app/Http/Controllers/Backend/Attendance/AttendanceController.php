<?php

namespace App\Http\Controllers\Backend\Attendance;

use App\Http\Controllers\Controller;
use App\Models\Attendance;
use App\Models\AttendanceRule;
use App\Models\Classroom;
use App\Models\Student;
use App\Services\AbsensiApi;
use App\Support\Tenant;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Routing\Controllers\HasMiddleware;
use Illuminate\Routing\Controllers\Middleware;
use Illuminate\Support\Carbon;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Log;
use Illuminate\View\View;
use Yajra\DataTables\Facades\DataTables;

/**
 * Monitoring & koreksi absensi — layar utama guru dan kepala sekolah.
 *
 * Koreksi absensi TIDAK ditulis langsung ke database dari sini, melainkan
 * lewat API Rust. Alasannya: perhitungan menit keterlambatan, pemilihan
 * template notifikasi, dan penulisan outbox transaksional sudah ada di sana.
 * Menduplikasinya di PHP akan membuat angka pada laporan berbeda tergantung
 * apakah absensi berasal dari tablet atau dari koreksi manual.
 */
class AttendanceController extends Controller implements HasMiddleware
{
    public static function middleware(): array
    {
        return [
            'auth',
            new Middleware('can:view_attendance', only: ['index', 'data', 'live', 'byClassroom']),
            new Middleware('can:override_attendance', only: ['manual', 'bulk']),
            new Middleware('can:delete_attendance', only: ['destroy']),
            new Middleware('can:view_report', only: ['recap']),
            new Middleware('can:manage_attendance_rule', only: ['rules', 'storeRule']),
        ];
    }

    public function index(Request $request): View
    {
        $schoolId = Tenant::currentSchoolId($request->query('school_id'));
        [$from, $to] = $this->range($request);

        return view('backend.attendance.index', [
            'schoolId' => $schoolId,
            'schools' => Tenant::isProvinceScope() ? Tenant::selectableSchools() : collect(),
            'classrooms' => $this->classroomOptions($schoolId),
            'from' => $from,
            'to' => $to,
            'statuses' => Attendance::STATUSES,
            'summary' => $this->summary($schoolId, $from, $to),
        ]);
    }

    public function data(Request $request)
    {
        $schoolId = Tenant::currentSchoolId($request->query('school_id'));
        [$from, $to] = $this->range($request);

        // Filter tanggal SELALU dipasang lebih dulu — `attendances` dipartisi
        // per bulan, dan tanpa ini planner akan memindai seluruh riwayat.
        $query = Attendance::query()
            ->betweenDates($from->toDateString(), $to->toDateString())
            ->when($schoolId, fn ($q) => $q->where('school_id', $schoolId))
            ->select([
                'attendance_date', 'id', 'student_id', 'student_name', 'student_nis',
                'classroom_id', 'classroom_name', 'school_name', 'check_in_at',
                'check_out_at', 'status', 'late_minutes', 'check_in_method',
                'notes', 'notification_status',
            ]);

        $query->when($request->filled('classroom_id'),
            fn ($q) => $q->where('classroom_id', $request->classroom_id));
        $query->when($request->filled('status'),
            fn ($q) => $q->where('status', $request->status));
        $query->when($request->boolean('missing_check_out'),
            fn ($q) => $q->missingCheckOut());

        return DataTables::of($query)
            ->filterColumn('student_name', function ($q, $keyword) {
                $q->where(function ($sub) use ($keyword) {
                    $sub->where('student_name', 'ilike', "%{$keyword}%")
                        ->orWhere('student_nis', 'ilike', "%{$keyword}%");
                });
            })
            ->editColumn('attendance_date', fn ($row) => Carbon::parse($row->attendance_date)->translatedFormat('d M Y'))
            ->addColumn('check_in_label', fn ($row) => $row->check_in_at
                ? Carbon::parse($row->check_in_at)->timezone(config('app.timezone'))->format('H:i')
                : '-')
            ->addColumn('check_out_label', fn ($row) => $row->check_out_at
                ? Carbon::parse($row->check_out_at)->timezone(config('app.timezone'))->format('H:i')
                : '-')
            ->addColumn('status_badge', function ($row) {
                $badge = match ($row->status) {
                    'hadir' => 'success', 'terlambat' => 'warning',
                    'izin', 'dispensasi' => 'info', 'sakit' => 'primary',
                    'alfa' => 'danger', default => 'secondary',
                };
                $label = match ($row->status) {
                    'hadir' => 'Hadir', 'terlambat' => 'Terlambat +'.$row->late_minutes.'m',
                    'izin' => 'Izin', 'sakit' => 'Sakit',
                    'alfa' => 'Tanpa Keterangan', 'dispensasi' => 'Dispensasi',
                    default => ucfirst((string) $row->status),
                };

                return '<span class="badge badge-light-'.$badge.'">'.$label.'</span>';
            })
            ->addColumn('method_label', fn ($row) => match ($row->check_in_method) {
                'face' => '<i class="ki-duotone ki-user-tick text-success fs-5"></i> Wajah',
                'manual' => '<i class="ki-duotone ki-pencil text-warning fs-5"></i> Manual',
                'import' => 'Impor',
                default => '-',
            })
            ->addColumn('action', fn ($row) => view('backend.attendance._action', ['row' => $row])->render())
            ->rawColumns(['status_badge', 'method_label', 'action'])
            ->make(true);
    }

    /**
     * Umpan scan terbaru — dipakai layar monitoring yang auto-refresh.
     */
    public function live(Request $request)
    {
        $schoolId = Tenant::currentSchoolId($request->query('school_id'));
        $date = Carbon::today();

        $rows = Attendance::query()
            ->onDate($date)
            ->when($schoolId, fn ($q) => $q->where('school_id', $schoolId))
            ->where(fn ($q) => $q->whereNotNull('check_in_at')->orWhereNotNull('check_out_at'))
            ->orderByRaw('GREATEST(COALESCE(check_out_at, check_in_at), COALESCE(check_in_at, check_out_at)) DESC')
            ->limit(30)
            ->get();

        return response()->json([
            'server_time' => now()->toIso8601String(),
            'items' => $rows->map(fn ($a) => [
                'student_name' => $a->student_name,
                'classroom_name' => $a->classroom_name,
                'school_name' => $a->school_name,
                'check_in' => $a->check_in_time,
                'check_out' => $a->check_out_time,
                'status' => $a->status,
                'status_label' => $a->status_label,
                'badge' => $a->status_badge,
                'direction' => $a->check_out_at ? 'pulang' : 'masuk',
            ]),
        ]);
    }

    public function byClassroom(Request $request): View
    {
        $schoolId = Tenant::currentSchoolId($request->query('school_id'));
        abort_if($schoolId === null, 400, 'Pilih sekolah terlebih dahulu.');

        $date = $this->singleDate($request);

        $rows = collect(DB::select("
            WITH aktif AS (
                SELECT current_classroom_id AS classroom_id, COUNT(*) AS total
                FROM students
                WHERE school_id = ? AND deleted_at IS NULL AND status = 'aktif'
                GROUP BY current_classroom_id
            ),
            absen AS (
                SELECT classroom_id,
                       COUNT(*) FILTER (WHERE status = 'hadir')     AS hadir,
                       COUNT(*) FILTER (WHERE status = 'terlambat') AS terlambat,
                       COUNT(*) FILTER (WHERE status = 'izin')      AS izin,
                       COUNT(*) FILTER (WHERE status = 'sakit')     AS sakit,
                       COUNT(*) FILTER (WHERE status = 'alfa')      AS alfa,
                       COUNT(*)                                     AS tercatat
                FROM attendances
                WHERE school_id = ? AND attendance_date = ?
                GROUP BY classroom_id
            )
            SELECT c.id, c.name, c.grade_level, u.name AS homeroom_teacher_name,
                   COALESCE(a.total, 0) AS total_students,
                   COALESCE(b.hadir, 0) AS hadir, COALESCE(b.terlambat, 0) AS terlambat,
                   COALESCE(b.izin, 0) AS izin, COALESCE(b.sakit, 0) AS sakit,
                   COALESCE(b.alfa, 0) AS alfa,
                   GREATEST(COALESCE(a.total, 0) - COALESCE(b.tercatat, 0), 0) AS belum_absen
            FROM classrooms c
            LEFT JOIN aktif a ON a.classroom_id = c.id
            LEFT JOIN absen b ON b.classroom_id = c.id
            LEFT JOIN users u ON u.id = c.homeroom_teacher_id
            WHERE c.school_id = ? AND c.deleted_at IS NULL AND c.is_active
            ORDER BY c.grade_level, c.name
        ", [$schoolId, $schoolId, $date->toDateString(), $schoolId]));

        return view('backend.attendance.by_classroom', [
            'rows' => $rows,
            'date' => $date,
            'schoolId' => $schoolId,
            'schools' => Tenant::isProvinceScope() ? Tenant::selectableSchools() : collect(),
        ]);
    }

    public function recap(Request $request): View
    {
        $schoolId = Tenant::currentSchoolId($request->query('school_id'));
        [$from, $to] = $this->range($request);
        $classroomId = $request->query('classroom_id');

        $rows = collect();
        if ($schoolId) {
            $rows = collect(DB::select("
                WITH hari_efektif AS (
                    SELECT COUNT(DISTINCT attendance_date) AS n
                    FROM attendances
                    WHERE school_id = ? AND attendance_date BETWEEN ? AND ?
                )
                SELECT s.id, s.full_name, s.nis, c.name AS classroom_name,
                       COUNT(a.id) FILTER (WHERE a.status = 'hadir')     AS hadir,
                       COUNT(a.id) FILTER (WHERE a.status = 'terlambat') AS terlambat,
                       COUNT(a.id) FILTER (WHERE a.status = 'izin')      AS izin,
                       COUNT(a.id) FILTER (WHERE a.status = 'sakit')     AS sakit,
                       COUNT(a.id) FILTER (WHERE a.status = 'alfa')      AS alfa,
                       COALESCE(SUM(a.late_minutes), 0)                  AS total_late,
                       (SELECT n FROM hari_efektif)                      AS effective_days
                FROM students s
                LEFT JOIN classrooms c ON c.id = s.current_classroom_id
                LEFT JOIN attendances a
                       ON a.student_id = s.id AND a.attendance_date BETWEEN ? AND ?
                WHERE s.school_id = ? AND s.deleted_at IS NULL AND s.status = 'aktif'
                  AND (?::uuid IS NULL OR s.current_classroom_id = ?::uuid)
                GROUP BY s.id, s.full_name, s.nis, c.name
                ORDER BY c.name NULLS LAST, s.full_name
            ", [
                $schoolId, $from->toDateString(), $to->toDateString(),
                $from->toDateString(), $to->toDateString(),
                $schoolId, $classroomId, $classroomId,
            ]));
        }

        return view('backend.attendance.recap', [
            'rows' => $rows,
            'from' => $from,
            'to' => $to,
            'schoolId' => $schoolId,
            'classroomId' => $classroomId,
            'schools' => Tenant::isProvinceScope() ? Tenant::selectableSchools() : collect(),
            'classrooms' => $this->classroomOptions($schoolId),
        ]);
    }

    /**
     * Koreksi absensi satu siswa. Diteruskan ke API agar aturannya tunggal.
     */
    public function manual(Request $request): RedirectResponse
    {
        $data = $request->validate([
            'student_id' => ['required', 'exists:students,id'],
            'attendance_date' => ['nullable', 'date', 'before_or_equal:today'],
            'status' => ['required', 'in:'.implode(',', Attendance::STATUSES)],
            'check_in_time' => ['nullable', 'date_format:H:i'],
            'check_out_time' => ['nullable', 'date_format:H:i'],
            'notes' => ['required', 'string', 'min:3', 'max:300'],
            'notify_guardian' => ['nullable', 'boolean'],
        ], [], [
            'notes' => 'alasan koreksi',
            'check_in_time' => 'jam masuk',
        ]);

        $student = Student::findOrFail($data['student_id']);
        Tenant::authorizeSchool($student->school_id);

        // Status hadir/terlambat tanpa jam masuk menghasilkan baris yang
        // membingungkan di laporan ("hadir, jam masuk -").
        if (in_array($data['status'], Attendance::PRESENT_STATUSES, true) && empty($data['check_in_time'])) {
            return back()
                ->withInput()
                ->withErrors(['check_in_time' => 'Jam masuk wajib diisi untuk status hadir/terlambat/dispensasi.']);
        }

        $result = AbsensiApi::make()->manualAttendance(AbsensiApi::tokenFromSession(), [
            'student_id' => $data['student_id'],
            'attendance_date' => $data['attendance_date'] ?? null,
            'status' => $data['status'],
            'check_in_time' => $data['check_in_time'] ?? null,
            'check_out_time' => $data['check_out_time'] ?? null,
            'notes' => $data['notes'],
            'notify_guardian' => (bool) ($data['notify_guardian'] ?? false),
        ]);

        return $result['success']
            ? back()->with('success', $result['message'])
            : back()->withInput()->withErrors($result['errors'] ?: ['status' => $result['message']]);
    }

    /**
     * Koreksi massal, mis. satu kelas mengikuti lomba (dispensasi).
     */
    public function bulk(Request $request): RedirectResponse
    {
        $data = $request->validate([
            'student_ids' => ['required', 'array', 'min:1', 'max:500'],
            'student_ids.*' => ['uuid'],
            'attendance_date' => ['nullable', 'date', 'before_or_equal:today'],
            'status' => ['required', 'in:'.implode(',', Attendance::STATUSES)],
            'notes' => ['required', 'string', 'min:3', 'max:300'],
            'notify_guardian' => ['nullable', 'boolean'],
        ]);

        $result = AbsensiApi::make()->bulkAttendance(AbsensiApi::tokenFromSession(), $data);

        if (! $result['success']) {
            return back()->withErrors(['status' => $result['message']]);
        }

        $payload = $result['data'] ?? [];
        $message = $result['message'];
        if (! empty($payload['errors'])) {
            $message .= ' Catatan: '.implode('; ', array_slice($payload['errors'], 0, 3));
        }

        return back()->with('success', $message);
    }

    /**
     * Hapus satu baris absensi.
     *
     * Tiga hal yang disengaja di sini.
     *
     * Pertama, hanya superadmin. Izin 'delete_attendance' tidak dimiliki peran
     * mana pun di basis data, jadi satu-satunya yang lolos adalah superadmin
     * lewat Gate::before di AppServiceProvider. 'override_attendance' TIDAK
     * dipakai: izin itu juga dipegang guru dan staff_tu, dan mengoreksi status
     * kehadiran adalah satu hal, menghapus jejaknya hal lain.
     *
     * Kedua, penulisan langsung ke basis data — menyimpang dari aturan di
     * kepala kelas ini bahwa perubahan absensi lewat API Rust. Aturan itu ada
     * karena perhitungan menit keterlambatan, pemilihan template notifikasi,
     * dan penulisan outbox tinggal di sana; tidak satu pun berlaku saat
     * menghapus, dan API Rust memang tidak punya endpoint hapus. Baris
     * `attendance_events` sengaja DIBIARKAN: itu catatan mesin tentang apa yang
     * pernah dipindai, dan justru itu yang membuat penghapusan ini tetap bisa
     * dilacak.
     *
     * Ketiga, tanggal wajib dikirim pemanggil. Itu bukan formalitas:
     * `attendances` dipartisi RANGE per bulan pada attendance_date, jadi tanpa
     * filter tanggal satu DELETE akan menyentuh seluruh partisi riwayat
     * provinsi.
     */
    public function destroy(Request $request, string $attendance): JsonResponse|RedirectResponse
    {
        $data = $request->validate([
            'attendance_date' => ['required', 'date_format:Y-m-d'],
        ]);

        $baris = Attendance::query()
            ->where('attendance_date', $data['attendance_date'])
            ->whereKey($attendance)
            ->first();

        if (! $baris) {
            $pesan = 'Data absensi itu tidak ditemukan; mungkin sudah dihapus.';

            return $request->expectsJson()
                ? response()->json(['message' => $pesan], 404)
                : back()->with('error', $pesan);
        }

        // Dicatat SEBELUM barisnya hilang. Ini satu-satunya jejak di sisi
        // dashboard tentang siapa yang menghapus apa; tanpa ini penghapusan
        // hanya terlihat sebagai data yang tiba-tiba tidak ada.
        Log::warning('Absensi dihapus dari dashboard', [
            'attendance_id' => $baris->id,
            'attendance_date' => $data['attendance_date'],
            'student_id' => $baris->student_id,
            'student_name' => $baris->student_name,
            'status' => $baris->status,
            'check_in_at' => (string) $baris->check_in_at,
            'check_out_at' => (string) $baris->check_out_at,
            'oleh_user_id' => $request->user()?->id,
            'oleh_username' => $request->user()?->username,
        ]);

        Attendance::query()
            ->where('attendance_date', $data['attendance_date'])
            ->whereKey($attendance)
            ->delete();

        $pesan = 'Absensi '.$baris->student_name.' tanggal '
            .Carbon::parse($data['attendance_date'])->translatedFormat('d M Y').' dihapus.';

        return $request->expectsJson()
            ? response()->json(['message' => $pesan])
            : back()->with('success', $pesan);
    }

    // =================================================================
    // Aturan jam absensi
    // =================================================================

    public function rules(Request $request): View
    {
        $schoolId = Tenant::currentSchoolId($request->query('school_id'));

        return view('backend.attendance.rules', [
            'schoolId' => $schoolId,
            'schools' => Tenant::isProvinceScope() ? Tenant::selectableSchools() : collect(),
            'classrooms' => $this->classroomOptions($schoolId),
            'rules' => $schoolId
                ? AttendanceRule::where('school_id', $schoolId)
                    ->with('classroom:id,name')
                    ->orderByRaw('classroom_id IS NULL DESC')
                    ->orderBy('name')
                    ->get()
                : collect(),
        ]);
    }

    public function storeRule(Request $request): RedirectResponse
    {
        $data = $request->validate([
            'school_id' => ['required', 'exists:schools,id'],
            'classroom_id' => ['nullable', 'exists:classrooms,id'],
            'name' => ['nullable', 'string', 'max:80'],
            'check_in_opens_at' => ['required', 'date_format:H:i'],
            'check_in_start_at' => ['required', 'date_format:H:i'],
            'check_in_due_at' => ['required', 'date_format:H:i'],
            'check_in_closes_at' => ['required', 'date_format:H:i'],
            'check_out_opens_at' => ['required', 'date_format:H:i'],
            'check_out_closes_at' => ['required', 'date_format:H:i'],
            'late_grace_minutes' => ['nullable', 'integer', 'between:0,120'],
            'active_days' => ['required', 'array', 'min:1'],
            'active_days.*' => ['integer', 'between:0,6'],
            'require_check_out' => ['nullable', 'boolean'],
        ]);

        Tenant::authorizeSchool($data['school_id']);

        // Urutan jam harus konsisten; kalau tidak, klasifikasi hadir/terlambat
        // menjadi tidak bisa ditentukan.
        $order = [
            'check_in_opens_at', 'check_in_start_at', 'check_in_due_at', 'check_in_closes_at',
        ];
        for ($i = 1; $i < count($order); $i++) {
            if ($data[$order[$i]] < $data[$order[$i - 1]]) {
                return back()->withInput()->withErrors([
                    $order[$i] => 'Urutan jam tidak konsisten: harus lebih besar dari jam sebelumnya.',
                ]);
            }
        }
        if ($data['check_out_closes_at'] <= $data['check_out_opens_at']) {
            return back()->withInput()->withErrors([
                'check_out_closes_at' => 'Jam tutup absen pulang harus setelah jam bukanya.',
            ]);
        }

        // Bitmask: bit0 = Senin .. bit6 = Minggu.
        $mask = 0;
        foreach ($data['active_days'] as $bit) {
            $mask |= (1 << (int) $bit);
        }

        DB::transaction(function () use ($data, $mask) {
            // Aturan lama dinonaktifkan, bukan dihapus: absensi yang sudah
            // tercatat dinilai dengan aturan yang berlaku saat itu.
            AttendanceRule::where('school_id', $data['school_id'])
                ->where('is_active', true)
                ->when(
                    $data['classroom_id'] ?? null,
                    fn ($q) => $q->where('classroom_id', $data['classroom_id']),
                    fn ($q) => $q->whereNull('classroom_id')
                )
                ->update(['is_active' => false, 'effective_to' => now()->toDateString()]);

            AttendanceRule::create([
                'school_id' => $data['school_id'],
                'classroom_id' => $data['classroom_id'] ?? null,
                'name' => $data['name'] ?: 'Jadwal Reguler',
                'check_in_opens_at' => $data['check_in_opens_at'],
                'check_in_start_at' => $data['check_in_start_at'],
                'check_in_due_at' => $data['check_in_due_at'],
                'check_in_closes_at' => $data['check_in_closes_at'],
                'check_out_opens_at' => $data['check_out_opens_at'],
                'check_out_closes_at' => $data['check_out_closes_at'],
                'late_grace_minutes' => $data['late_grace_minutes'] ?? 0,
                'active_weekdays' => $mask,
                'require_check_out' => (bool) ($data['require_check_out'] ?? true),
                'effective_from' => now()->toDateString(),
                'is_active' => true,
            ]);
        });

        return back()->with('success', 'Aturan jam absensi disimpan dan langsung berlaku.');
    }

    // =================================================================

    /**
     * @return array{0: Carbon, 1: Carbon}
     */
    private function range(Request $request): array
    {
        $from = $this->parseDate($request->query('from')) ?? Carbon::today();
        $to = $this->parseDate($request->query('to')) ?? $from->copy();

        if ($to->lt($from)) {
            [$from, $to] = [$to, $from];
        }

        // Rentang dibatasi setahun agar satu permintaan laporan tidak
        // memindai seluruh riwayat provinsi.
        if ($from->diffInDays($to) > 366) {
            $to = $from->copy()->addDays(366);
        }

        return [$from, $to];
    }

    private function singleDate(Request $request): Carbon
    {
        return $this->parseDate($request->query('date')) ?? Carbon::today();
    }

    private function parseDate(?string $value): ?Carbon
    {
        if (! $value) {
            return null;
        }
        try {
            $date = Carbon::parse($value);
        } catch (\Throwable) {
            return null;
        }

        return $date->isFuture() ? Carbon::today() : $date;
    }

    /**
     * @return array<string, int|float>
     */
    private function summary(?string $schoolId, Carbon $from, Carbon $to): array
    {
        $row = Attendance::query()
            ->betweenDates($from->toDateString(), $to->toDateString())
            ->when($schoolId, fn ($q) => $q->where('school_id', $schoolId))
            ->selectRaw("
                COUNT(*) FILTER (WHERE status = 'hadir')      AS hadir,
                COUNT(*) FILTER (WHERE status = 'terlambat')  AS terlambat,
                COUNT(*) FILTER (WHERE status = 'izin')       AS izin,
                COUNT(*) FILTER (WHERE status = 'sakit')      AS sakit,
                COUNT(*) FILTER (WHERE status = 'alfa')       AS alfa,
                COUNT(*) FILTER (WHERE status = 'dispensasi') AS dispensasi,
                COUNT(*) AS total
            ")
            ->first();

        return [
            'hadir' => (int) ($row->hadir ?? 0),
            'terlambat' => (int) ($row->terlambat ?? 0),
            'izin' => (int) ($row->izin ?? 0),
            'sakit' => (int) ($row->sakit ?? 0),
            'alfa' => (int) ($row->alfa ?? 0),
            'dispensasi' => (int) ($row->dispensasi ?? 0),
            'total' => (int) ($row->total ?? 0),
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
