<?php

namespace App\Http\Controllers\Backend\Dashboard;

use App\Http\Controllers\Controller;
use App\Models\Attendance;
use App\Models\Device;
use App\Models\NotificationOutbox;
use App\Models\School;
use App\Models\Student;
use App\Services\AbsensiApi;
use App\Support\Tenant;
use Illuminate\Http\Request;
use Illuminate\Support\Carbon;
use Illuminate\Support\Facades\DB;
use Illuminate\View\View;

/**
 * Dashboard.
 *
 * Menampilkan dua wajah berbeda tergantung peran:
 *
 *   - Superadmin / Admin Dinas -> ikhtisar PROVINSI: berapa sekolah sudah
 *     melapor hari ini, cakupan pendaftaran wajah, sekolah dengan kehadiran
 *     terendah (yang perlu ditindak), kesehatan perangkat.
 *   - Kepala Sekolah / Guru / Staff -> monitoring SEKOLAHNYA: rekap per
 *     kelas, siapa yang belum absen, siswa yang belum terdaftar wajahnya.
 *
 * Semua query membawa filter tanggal karena `attendances` dipartisi per bulan.
 */
class DashboardAdminController extends Controller
{
    public function index(Request $request): View
    {
        $date = $this->resolveDate($request);
        $schoolId = Tenant::currentSchoolId($request->query('school_id'));
        $isProvince = Tenant::isProvinceScope();

        $data = [
            'date' => $date,
            'isProvince' => $isProvince,
            'schoolId' => $schoolId,
            'school' => $schoolId ? School::find($schoolId) : null,
            'schools' => $isProvince ? Tenant::selectableSchools() : collect(),
            'summary' => $this->attendanceSummary($schoolId, $date),
            'biometric' => $this->biometricCoverage($schoolId),
            'devices' => $this->deviceHealth($schoolId),
            'notifications' => $this->notificationBrief($schoolId, $date),
            'trend' => $this->trend($schoolId, $date),
            'recent' => $this->recentScans($schoolId, $date),
        ];

        if ($isProvince && $schoolId === null) {
            $data['provinceStats'] = $this->provinceStats($date);
            $data['lowestSchools'] = $this->schoolRates($date, 'asc');
            $data['topSchools'] = $this->schoolRates($date, 'desc');
            $data['apiHealth'] = AbsensiApi::make()->health();
        } else {
            $data['classrooms'] = $this->classroomSummary($schoolId, $date);
            $data['pendingFaces'] = $this->studentsWithoutFace($schoolId);
        }

        return view('backend.dashboard.index', $data);
    }

    private function resolveDate(Request $request): Carbon
    {
        $raw = $request->query('date');

        try {
            $date = $raw ? Carbon::parse($raw) : Carbon::today();
        } catch (\Throwable) {
            $date = Carbon::today();
        }

        // Absensi masa depan tidak ada isinya; diamkan ke hari ini agar
        // pengguna tidak melihat dashboard kosong tanpa penjelasan.
        return $date->isFuture() ? Carbon::today() : $date;
    }

    /**
     * @return array<string, int|float>
     */
    private function attendanceSummary(?string $schoolId, Carbon $date): array
    {
        $totalStudents = Student::query()
            ->when($schoolId, fn ($q) => $q->where('school_id', $schoolId))
            ->where('status', 'aktif')
            ->count();

        $row = Attendance::query()
            ->onDate($date)
            ->when($schoolId, fn ($q) => $q->where('school_id', $schoolId))
            ->selectRaw("
                COUNT(*) FILTER (WHERE status = 'hadir')      AS hadir,
                COUNT(*) FILTER (WHERE status = 'terlambat')  AS terlambat,
                COUNT(*) FILTER (WHERE status = 'izin')       AS izin,
                COUNT(*) FILTER (WHERE status = 'sakit')      AS sakit,
                COUNT(*) FILTER (WHERE status = 'alfa')       AS alfa,
                COUNT(*) FILTER (WHERE status = 'dispensasi') AS dispensasi,
                COUNT(*)                                     AS tercatat
            ")
            ->first();

        $hadir = (int) ($row->hadir ?? 0);
        $terlambat = (int) ($row->terlambat ?? 0);
        $dispensasi = (int) ($row->dispensasi ?? 0);
        $present = $hadir + $terlambat + $dispensasi;

        return [
            'total_students' => $totalStudents,
            'hadir' => $hadir,
            'terlambat' => $terlambat,
            'izin' => (int) ($row->izin ?? 0),
            'sakit' => (int) ($row->sakit ?? 0),
            'alfa' => (int) ($row->alfa ?? 0),
            'dispensasi' => $dispensasi,
            'belum_absen' => max(0, $totalStudents - (int) ($row->tercatat ?? 0)),
            'rate' => $totalStudents > 0 ? round($present / $totalStudents * 100, 1) : 0.0,
        ];
    }

    /**
     * @return array<string, int|float>
     */
    private function biometricCoverage(?string $schoolId): array
    {
        $row = Student::query()
            ->when($schoolId, fn ($q) => $q->where('school_id', $schoolId))
            ->where('status', 'aktif')
            ->selectRaw('
                COUNT(*) AS total,
                COUNT(*) FILTER (WHERE face_enrolled) AS enrolled,
                COUNT(*) FILTER (WHERE face_enrolled AND face_sample_count < 3) AS under_sampled
            ')
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

    /**
     * @return array<string, int>
     */
    private function deviceHealth(?string $schoolId): array
    {
        $row = Device::query()
            ->when($schoolId, fn ($q) => $q->where('school_id', $schoolId))
            ->where('is_active', true)
            ->selectRaw('
                COUNT(*) AS total,
                COUNT(*) FILTER (WHERE last_seen_at > NOW() - INTERVAL \'10 minutes\') AS online,
                COUNT(*) FILTER (WHERE token_hash IS NULL) AS never_paired
            ')
            ->first();

        $total = (int) ($row->total ?? 0);
        $online = (int) ($row->online ?? 0);

        return [
            'total' => $total,
            'online' => $online,
            'offline' => max(0, $total - $online),
            'never_paired' => (int) ($row->never_paired ?? 0),
        ];
    }

    /**
     * @return array<string, int>
     */
    private function notificationBrief(?string $schoolId, Carbon $date): array
    {
        $row = NotificationOutbox::query()
            ->recent(7)
            ->when($schoolId, fn ($q) => $q->where('school_id', $schoolId))
            ->selectRaw("
                COUNT(*) FILTER (WHERE status = 'queued') AS queued,
                COUNT(*) FILTER (WHERE status = 'sent'   AND sent_at::date = ?) AS sent,
                COUNT(*) FILTER (WHERE status = 'failed' AND created_at::date = ?) AS failed
            ", [$date->toDateString(), $date->toDateString()])
            ->first();

        return [
            'queued' => (int) ($row->queued ?? 0),
            'sent' => (int) ($row->sent ?? 0),
            'failed' => (int) ($row->failed ?? 0),
        ];
    }

    /**
     * Tren 7 hari untuk grafik.
     *
     * @return array<string, array<int, int|string>>
     */
    private function trend(?string $schoolId, Carbon $date): array
    {
        $rows = Attendance::query()
            ->betweenDates($date->copy()->subDays(6)->toDateString(), $date->toDateString())
            ->when($schoolId, fn ($q) => $q->where('school_id', $schoolId))
            ->selectRaw("
                attendance_date,
                COUNT(*) FILTER (WHERE status = 'hadir')     AS hadir,
                COUNT(*) FILTER (WHERE status = 'terlambat') AS terlambat,
                COUNT(*) FILTER (WHERE status = 'alfa')      AS alfa
            ")
            ->groupBy('attendance_date')
            ->orderBy('attendance_date')
            ->get()
            ->keyBy(fn ($r) => Carbon::parse($r->attendance_date)->toDateString());

        $labels = [];
        $hadir = [];
        $terlambat = [];
        $alfa = [];

        for ($i = 6; $i >= 0; $i--) {
            $day = $date->copy()->subDays($i);
            $key = $day->toDateString();
            $row = $rows->get($key);

            $labels[] = $day->translatedFormat('d M');
            $hadir[] = (int) ($row->hadir ?? 0);
            $terlambat[] = (int) ($row->terlambat ?? 0);
            $alfa[] = (int) ($row->alfa ?? 0);
        }

        return compact('labels', 'hadir', 'terlambat', 'alfa');
    }

    private function recentScans(?string $schoolId, Carbon $date)
    {
        return Attendance::query()
            ->onDate($date)
            ->when($schoolId, fn ($q) => $q->where('school_id', $schoolId))
            ->where(fn ($q) => $q->whereNotNull('check_in_at')->orWhereNotNull('check_out_at'))
            ->orderByRaw('GREATEST(COALESCE(check_out_at, check_in_at), COALESCE(check_in_at, check_out_at)) DESC')
            ->limit(15)
            ->get([
                'attendance_date', 'id', 'student_id', 'student_name', 'classroom_name',
                'school_name', 'check_in_at', 'check_out_at', 'status', 'late_minutes',
            ]);
    }

    private function classroomSummary(?string $schoolId, Carbon $date)
    {
        if (! $schoolId) {
            return collect();
        }

        // Dua CTE, bukan LEFT JOIN berantai: jumlah siswa aktif per kelas dan
        // rekap absensi hari itu punya granularitas berbeda, dan menggabungkan
        // keduanya langsung akan menghasilkan hitungan ganda.
        return collect(DB::select("
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
                   COALESCE(a.total, 0)     AS total_students,
                   COALESCE(b.hadir, 0)     AS hadir,
                   COALESCE(b.terlambat, 0) AS terlambat,
                   COALESCE(b.izin, 0)      AS izin,
                   COALESCE(b.sakit, 0)     AS sakit,
                   COALESCE(b.alfa, 0)      AS alfa,
                   GREATEST(COALESCE(a.total, 0) - COALESCE(b.tercatat, 0), 0) AS belum_absen
            FROM classrooms c
            LEFT JOIN aktif a ON a.classroom_id = c.id
            LEFT JOIN absen b ON b.classroom_id = c.id
            LEFT JOIN users u ON u.id = c.homeroom_teacher_id
            WHERE c.school_id = ? AND c.deleted_at IS NULL AND c.is_active
            ORDER BY c.grade_level, c.name
        ", [$schoolId, $schoolId, $date->toDateString(), $schoolId]));
    }

    private function studentsWithoutFace(?string $schoolId)
    {
        return Student::query()
            ->with('classroom:id,name')
            ->when($schoolId, fn ($q) => $q->where('school_id', $schoolId))
            ->needsFaceEnrollment()
            ->orderBy('full_name')
            ->limit(10)
            ->get(['id', 'full_name', 'nis', 'current_classroom_id']);
    }

    /**
     * @return array<string, int>
     */
    private function provinceStats(Carbon $date): array
    {
        $schools = DB::selectOne('
            SELECT COUNT(*) AS total, COUNT(*) FILTER (WHERE is_active) AS active
            FROM schools WHERE deleted_at IS NULL
        ');

        $reporting = DB::selectOne(
            'SELECT COUNT(DISTINCT school_id) AS n FROM attendances WHERE attendance_date = ?',
            [$date->toDateString()]
        );

        return [
            'total_schools' => (int) ($schools->total ?? 0),
            'active_schools' => (int) ($schools->active ?? 0),
            'reporting_schools' => (int) ($reporting->n ?? 0),
        ];
    }

    /**
     * Sekolah dengan tingkat kehadiran tertinggi/terendah.
     *
     * Hanya sekolah yang benar-benar melaporkan absensi disertakan — sekolah
     * yang belum memasang tablet akan selalu tampak 0% dan mengubur sekolah
     * yang betulan bermasalah.
     */
    private function schoolRates(Carbon $date, string $direction)
    {
        return collect(DB::select("
            WITH aktif AS (
                SELECT school_id, COUNT(*) AS total
                FROM students WHERE deleted_at IS NULL AND status = 'aktif'
                GROUP BY school_id
            ),
            hadir AS (
                SELECT school_id,
                       COUNT(*) FILTER (WHERE status IN ('hadir','terlambat','dispensasi')) AS present
                FROM attendances WHERE attendance_date = ?
                GROUP BY school_id
            )
            SELECT s.id, s.name, s.jenjang, a.total AS total_students,
                   COALESCE(h.present, 0) AS present,
                   ROUND(COALESCE(h.present, 0)::numeric / NULLIF(a.total, 0) * 100, 1) AS rate
            FROM schools s
            JOIN aktif a ON a.school_id = s.id
            JOIN hadir h ON h.school_id = s.id
            WHERE s.deleted_at IS NULL AND s.is_active
            ORDER BY rate ".($direction === 'asc' ? 'ASC' : 'DESC')."
            LIMIT 10
        ", [$date->toDateString()]));
    }
}
