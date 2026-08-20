<?php

namespace App\Http\Controllers\Backend\Notification;

use App\Http\Controllers\Controller;
use App\Models\Classroom;
use App\Models\NotificationOutbox;
use App\Models\NotificationPolicy;
use App\Models\NotificationTemplate;
use App\Models\Student;
use App\Services\AbsensiApi;
use App\Support\Tenant;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Routing\Controllers\HasMiddleware;
use Illuminate\Routing\Controllers\Middleware;
use Illuminate\View\View;
use Yajra\DataTables\Facades\DataTables;

/**
 * Notifikasi ke wali murid (WhatsApp / Telegram / Email).
 *
 * Pengiriman dijalankan worker di API Rust memakai pola transactional outbox,
 * sehingga provider yang sedang down tidak pernah membuat absensi siswa gagal
 * tercatat. Dashboard hanya: mengelola template & kebijakan, memantau
 * riwayat, dan meminta kirim ulang.
 */
class NotificationController extends Controller implements HasMiddleware
{
    public static function middleware(): array
    {
        return [
            'auth',
            new Middleware('can:view_notification', only: ['index', 'outbox', 'outboxData', 'templates']),
            new Middleware('can:send_notification', only: ['send', 'retry']),
            new Middleware('can:manage_notification_template', only: ['storeTemplate', 'updatePolicy']),
        ];
    }

    public function index(Request $request): View
    {
        $schoolId = Tenant::currentSchoolId($request->query('school_id'));

        return view('backend.notification.index', [
            'schoolId' => $schoolId,
            'schools' => Tenant::isProvinceScope() ? Tenant::selectableSchools() : collect(),
            'stats' => $this->stats($schoolId),
            'policy' => $schoolId ? NotificationPolicy::forSchool($schoolId) : null,
            'classrooms' => $this->classroomOptions($schoolId),
        ]);
    }

    public function outbox(Request $request): View
    {
        $schoolId = Tenant::currentSchoolId($request->query('school_id'));

        return view('backend.notification.outbox', [
            'schoolId' => $schoolId,
            'schools' => Tenant::isProvinceScope() ? Tenant::selectableSchools() : collect(),
            'statuses' => NotificationOutbox::STATUSES,
            'channels' => NotificationTemplate::CHANNELS,
        ]);
    }

    public function outboxData(Request $request)
    {
        $schoolId = Tenant::currentSchoolId($request->query('school_id'));

        // recent() wajib: tabel dipartisi per bulan, tanpa batas waktu query
        // akan menyentuh seluruh partisi.
        $query = NotificationOutbox::query()
            ->recent(90)
            ->when($schoolId, fn ($q) => $q->where('school_id', $schoolId))
            ->leftJoin('students', 'students.id', '=', 'notification_outbox.student_id')
            ->select([
                'notification_outbox.created_at', 'notification_outbox.id',
                'notification_outbox.channel', 'notification_outbox.template_key',
                'notification_outbox.recipient', 'notification_outbox.status',
                'notification_outbox.attempts', 'notification_outbox.provider',
                'notification_outbox.last_error', 'notification_outbox.sent_at',
                'students.full_name as student_name',
            ]);

        $query->when($request->filled('status'),
            fn ($q) => $q->where('notification_outbox.status', $request->status));
        $query->when($request->filled('channel'),
            fn ($q) => $q->where('notification_outbox.channel', $request->channel));

        return DataTables::of($query)
            ->filterColumn('student_name', fn ($q, $kw) => $q->where('students.full_name', 'ilike', "%{$kw}%"))
            ->editColumn('created_at', fn ($row) => \Illuminate\Support\Carbon::parse($row->created_at)
                ->timezone(config('app.timezone'))->format('d/m/Y H:i'))
            // Nomor tujuan disamarkan: daftar log tidak boleh menjadi sumber
            // ekspor nomor telepon orang tua.
            ->editColumn('recipient', function ($row) {
                $v = (string) $row->recipient;
                if (str_contains($v, '@')) {
                    [$local, $domain] = explode('@', $v, 2);

                    return mb_substr($local, 0, 1).'***@'.$domain;
                }
                $len = mb_strlen($v);

                return $len <= 6
                    ? str_repeat('*', $len)
                    : mb_substr($v, 0, 4).str_repeat('*', $len - 7).mb_substr($v, -3);
            })
            ->addColumn('status_badge', function ($row) {
                $badge = match ($row->status) {
                    'sent' => 'success', 'queued' => 'info', 'sending' => 'primary',
                    'failed' => 'danger', 'cancelled' => 'warning', default => 'secondary',
                };

                return '<span class="badge badge-light-'.$badge.'">'.ucfirst((string) $row->status).'</span>';
            })
            ->addColumn('action', function ($row) {
                if ($row->status !== 'failed') {
                    return '<span class="text-muted fs-8">-</span>';
                }

                return '<form method="POST" action="'.route('notifications.retry', $row->id).'">'
                    .csrf_field()
                    .'<button class="btn btn-sm btn-light-primary py-1">Kirim ulang</button></form>';
            })
            ->rawColumns(['status_badge', 'action'])
            ->make(true);
    }

    public function templates(Request $request): View
    {
        $schoolId = Tenant::currentSchoolId($request->query('school_id'));

        return view('backend.notification.templates', [
            'schoolId' => $schoolId,
            'schools' => Tenant::isProvinceScope() ? Tenant::selectableSchools() : collect(),
            // Template bawaan (school_id NULL) ikut ditampilkan sebagai
            // rujukan; sekolah boleh menimpanya dengan versi sendiri.
            'templates' => NotificationTemplate::forSchool($schoolId)
                ->orderByRaw('school_id IS NULL DESC')
                ->orderBy('key')
                ->orderBy('channel')
                ->get(),
            'keys' => NotificationTemplate::KEYS,
            'channels' => NotificationTemplate::CHANNELS,
            'variables' => NotificationTemplate::VARIABLES,
        ]);
    }

    public function storeTemplate(Request $request): RedirectResponse
    {
        $data = $request->validate([
            'school_id' => ['required', 'exists:schools,id'],
            'key' => ['required', 'in:'.implode(',', array_keys(NotificationTemplate::KEYS))],
            'channel' => ['required', 'in:'.implode(',', NotificationTemplate::CHANNELS)],
            'subject' => ['nullable', 'string', 'max:200'],
            'body' => ['required', 'string', 'min:10', 'max:4000'],
            'is_active' => ['nullable', 'boolean'],
        ], [], ['body' => 'isi pesan']);

        Tenant::authorizeSchool($data['school_id']);

        // Placeholder salah tulis baru terlihat setelah pesan salah terkirim
        // ke ribuan orang tua, jadi divalidasi sebelum disimpan.
        $unknown = NotificationTemplate::unknownPlaceholders($data['body']);
        if ($unknown !== []) {
            return back()->withInput()->withErrors([
                'body' => 'Placeholder tidak dikenal: {{'.implode('}}, {{', $unknown).'}}. '
                    .'Yang tersedia: '.implode(', ', NotificationTemplate::VARIABLES).'.',
            ]);
        }

        NotificationTemplate::updateOrCreate(
            [
                'school_id' => $data['school_id'],
                'key' => $data['key'],
                'channel' => $data['channel'],
            ],
            [
                'subject' => $data['subject'] ?? null,
                'body' => $data['body'],
                'is_active' => (bool) ($data['is_active'] ?? true),
            ]
        );

        return back()->with('success', 'Template notifikasi disimpan.');
    }

    public function updatePolicy(Request $request): RedirectResponse
    {
        $data = $request->validate([
            'school_id' => ['required', 'exists:schools,id'],
            'notify_on_check_in' => ['nullable', 'boolean'],
            'notify_on_check_out' => ['nullable', 'boolean'],
            'notify_on_late' => ['nullable', 'boolean'],
            'notify_on_absent' => ['nullable', 'boolean'],
            'absent_notify_after' => ['required', 'date_format:H:i'],
            'quiet_hours_start' => ['nullable', 'date_format:H:i'],
            'quiet_hours_end' => ['nullable', 'date_format:H:i'],
        ]);

        Tenant::authorizeSchool($data['school_id']);

        $policy = NotificationPolicy::forSchool($data['school_id']);
        $policy->update([
            'notify_on_check_in' => (bool) ($data['notify_on_check_in'] ?? false),
            'notify_on_check_out' => (bool) ($data['notify_on_check_out'] ?? false),
            'notify_on_late' => (bool) ($data['notify_on_late'] ?? false),
            'notify_on_absent' => (bool) ($data['notify_on_absent'] ?? false),
            'absent_notify_after' => $data['absent_notify_after'],
            'quiet_hours_start' => $data['quiet_hours_start'] ?? null,
            'quiet_hours_end' => $data['quiet_hours_end'] ?? null,
        ]);

        return back()->with('success', 'Kebijakan notifikasi diperbarui.');
    }

    /**
     * Kirim pesan bebas ke wali murid (satu kelas atau siswa terpilih).
     */
    public function send(Request $request): RedirectResponse
    {
        $data = $request->validate([
            'target' => ['required', 'in:classroom,students'],
            'classroom_id' => ['required_if:target,classroom', 'nullable', 'exists:classrooms,id'],
            'student_ids' => ['required_if:target,students', 'nullable', 'array', 'max:500'],
            'student_ids.*' => ['uuid'],
            'channel' => ['nullable', 'in:'.implode(',', NotificationTemplate::CHANNELS)],
            'subject' => ['nullable', 'string', 'max:200'],
            'body' => ['required', 'string', 'min:5', 'max:4000'],
        ], [], ['body' => 'isi pesan']);

        $studentIds = $data['target'] === 'classroom'
            ? Student::where('current_classroom_id', $data['classroom_id'])
                ->where('status', 'aktif')
                ->pluck('id')
                ->all()
            : ($data['student_ids'] ?? []);

        if ($studentIds === []) {
            return back()->withInput()->withErrors([
                'body' => 'Tidak ada siswa aktif yang menjadi tujuan pesan.',
            ]);
        }

        // Batas 500 per permintaan mengikuti batas API; pengiriman ke seluruh
        // sekolah dilakukan lewat rekap harian otomatis, bukan dari form ini.
        if (count($studentIds) > 500) {
            return back()->withInput()->withErrors([
                'body' => 'Maksimum 500 siswa per pengiriman. Pilih per kelas.',
            ]);
        }

        $result = AbsensiApi::make()->sendNotification(
            AbsensiApi::tokenFromSession(),
            $studentIds,
            $data['body'],
            $data['channel'] ?? null,
            $data['subject'] ?? null,
        );

        if (! $result['success']) {
            return back()->withInput()->withErrors(['body' => $result['message']]);
        }

        $payload = $result['data'] ?? [];
        $message = $result['message'];

        // Wali yang dilewati dilaporkan eksplisit — kalau tidak, operator akan
        // menganggap semua orang tua sudah menerima pesan.
        if (! empty($payload['skipped'])) {
            $names = array_slice(array_column($payload['skipped'], 'student_name'), 0, 5);
            $message .= ' Dilewati: '.implode(', ', $names)
                .(count($payload['skipped']) > 5 ? ', dan lainnya' : '')
                .' (kontak wali belum lengkap).';
        }

        return back()->with('success', $message);
    }

    public function retry(string $outboxId): RedirectResponse
    {
        $result = AbsensiApi::make()->retryNotification(
            AbsensiApi::tokenFromSession(),
            $outboxId
        );

        return $result['success']
            ? back()->with('success', 'Pesan dijadwalkan untuk dikirim ulang.')
            : back()->withErrors(['outbox' => $result['message']]);
    }

    // =================================================================

    /**
     * @return array<string, int>
     */
    private function stats(?string $schoolId): array
    {
        $row = NotificationOutbox::query()
            ->recent(7)
            ->when($schoolId, fn ($q) => $q->where('school_id', $schoolId))
            ->selectRaw("
                COUNT(*) FILTER (WHERE status = 'queued') AS queued,
                COUNT(*) FILTER (WHERE status = 'sent'   AND sent_at::date    = CURRENT_DATE) AS sent_today,
                COUNT(*) FILTER (WHERE status = 'failed' AND created_at::date = CURRENT_DATE) AS failed_today,
                COUNT(*) FILTER (WHERE channel = 'whatsapp') AS whatsapp,
                COUNT(*) FILTER (WHERE channel = 'telegram') AS telegram,
                COUNT(*) FILTER (WHERE channel = 'email')    AS email
            ")
            ->first();

        return [
            'queued' => (int) ($row->queued ?? 0),
            'sent_today' => (int) ($row->sent_today ?? 0),
            'failed_today' => (int) ($row->failed_today ?? 0),
            'whatsapp' => (int) ($row->whatsapp ?? 0),
            'telegram' => (int) ($row->telegram ?? 0),
            'email' => (int) ($row->email ?? 0),
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
