<?php

namespace App\Http\Controllers\Backend\Panic;

use App\Http\Controllers\Controller;
use App\Models\PanicCategory;
use App\Models\PanicReport;
use App\Models\PanicUnmaskLog;
use App\Services\AbsensiApi;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Routing\Controllers\HasMiddleware;
use Illuminate\Routing\Controllers\Middleware;
use Illuminate\View\View;

/**
 * Penanganan pengaduan Panic Button dari dashboard.
 *
 * PRINSIP YANG MENENTUKAN SELURUH DESAIN HALAMAN INI
 *
 * Identitas pelapor tidak pernah ditampilkan — bahkan kepada Superadmin —
 * kecuali lewat tindakan sadar yang mewajibkan alasan tertulis dan tercatat
 * permanen. Karena itu:
 *
 *   * Query di sini tidak pernah menyentuh kolom `author_user_id`
 *     (model menyembunyikannya).
 *   * Tombol "Buka Identitas" memanggil API, bukan menulis sendiri ke
 *     database, supaya pencatatan tidak bisa dilewati.
 *   * Kepala sekolah tidak melihat kategori kekerasan/pelecehan/pungli
 *     sama sekali — ditegakkan scope `visibleTo` pada model.
 */
class PanicController extends Controller implements HasMiddleware
{
    public static function middleware(): array
    {
        return [
            'auth',
            new Middleware('can:view_panic_feed', only: ['index', 'show']),
            new Middleware('can:moderate_panic_report', only: ['moderate']),
            new Middleware('can:handle_panic_report', only: ['updateStatus', 'comment']),
            new Middleware('can:unmask_panic_report', only: ['unmask', 'unmaskLogs']),
        ];
    }

    public function index(Request $request): View
    {
        $user = $request->user();

        $query = PanicReport::query()
            ->recent(180)
            ->visibleTo($user)
            ->with(['category:id,code,name,icon', 'school:id,name,jenjang']);

        $query->when($request->filled('status'), fn ($q) => $q->where('status', $request->status));
        $query->when($request->filled('severity'), fn ($q) => $q->where('severity', $request->severity));
        $query->when($request->filled('category'), fn ($q) => $q->whereHas(
            'category',
            fn ($c) => $c->where('code', $request->category)
        ));
        $query->when($request->boolean('pending'), fn ($q) => $q->pendingModeration());
        $query->when($request->boolean('urgent'), fn ($q) => $q->urgent());

        $reports = $query
            // Laporan darurat yang belum ditangani naik ke atas; sisanya
            // terbaru dulu. Urutan ini yang menentukan apa yang dilihat
            // petugas pertama kali saat membuka dashboard pagi hari.
            ->orderByRaw("(severity = 'darurat' AND handled_at IS NULL) DESC")
            ->orderByDesc('created_at')
            ->paginate(20)
            ->withQueryString();

        return view('backend.panic.index', [
            'reports' => $reports,
            'categories' => PanicCategory::where('is_active', true)
                ->orderBy('sort_order')
                ->get(),
            'stats' => $this->stats($user),
            'statuses' => PanicReport::STATUSES,
            'severities' => PanicReport::SEVERITIES,
        ]);
    }

    public function show(Request $request, string $id): View
    {
        $report = PanicReport::query()
            ->recent(180)
            ->visibleTo($request->user())
            ->with(['category', 'school:id,name,jenjang', 'comments', 'events'])
            ->where('panic_reports.id', $id)
            ->firstOrFail();

        return view('backend.panic.show', [
            'report' => $report,
            'statuses' => PanicReport::STATUSES,
            // Riwayat pembukaan identitas ditampilkan kepada siapa pun yang
            // boleh melihat laporan — transparansi ini yang membuat
            // kewenangan unmask tetap terkendali.
            'unmaskLogs' => PanicUnmaskLog::where('report_id', $id)
                ->orderByDesc('created_at')
                ->get(),
        ]);
    }

    /** Setujui / tolak tampilnya laporan di beranda aplikasi. */
    public function moderate(Request $request, string $id): RedirectResponse
    {
        $data = $request->validate([
            'moderation_status' => ['required', 'in:approved,rejected'],
            'note' => ['nullable', 'string', 'max:300'],
        ]);

        $result = AbsensiApi::make()->call(
            'POST',
            "/v1/panic/reports/{$id}/moderate",
            $data,
        );

        return $result['success']
            ? back()->with('success', $result['message'])
            : back()->withErrors(['moderation' => $result['message']]);
    }

    /** Perbarui status penanganan. */
    public function updateStatus(Request $request, string $id): RedirectResponse
    {
        $data = $request->validate([
            'status' => ['required', 'in:'.implode(',', PanicReport::STATUSES)],
            'note' => ['required', 'string', 'min:3', 'max:500'],
            'resolution' => ['nullable', 'string', 'max:2000'],
            'visible_to_reporter' => ['nullable', 'boolean'],
        ], [], ['note' => 'catatan tindak lanjut']);

        // Menutup laporan tanpa menjelaskan hasilnya membuat pelapor tidak
        // pernah tahu apa yang terjadi — dan berhenti melapor lain kali.
        if ($data['status'] === 'selesai' && strlen(trim($data['resolution'] ?? '')) < 10) {
            return back()->withInput()->withErrors([
                'resolution' => 'Jelaskan hasil penanganan sebelum menutup laporan.',
            ]);
        }

        $result = AbsensiApi::make()->call('POST', "/v1/panic/reports/{$id}/status", [
            'status' => $data['status'],
            'note' => $data['note'],
            'resolution' => $data['resolution'] ?? null,
            'visible_to_reporter' => (bool) ($data['visible_to_reporter'] ?? true),
        ]);

        return $result['success']
            ? back()->with('success', $result['message'])
            : back()->withInput()->withErrors(['status' => $result['message']]);
    }

    /** Balas laporan sebagai petugas resmi. */
    public function comment(Request $request, string $id): RedirectResponse
    {
        $data = $request->validate([
            'body' => ['required', 'string', 'min:2', 'max:2000'],
            'as_official' => ['nullable', 'boolean'],
        ], [], ['body' => 'isi balasan']);

        $result = AbsensiApi::make()->call('POST', "/v1/panic/reports/{$id}/comments", [
            'body' => $data['body'],
            'as_official' => (bool) ($data['as_official'] ?? true),
        ]);

        return $result['success']
            ? back()->with('success', 'Balasan terkirim.')
            : back()->withErrors(['body' => $result['message']]);
    }

    /**
     * Buka identitas pelapor.
     *
     * Sengaja diteruskan ke API alih-alih membaca kolomnya langsung: API
     * mencatat pembukaan ini ke `panic_unmask_logs` sebelum mengembalikan
     * data, sehingga tidak ada jalan membuka identitas tanpa jejak.
     */
    public function unmask(Request $request, string $id): RedirectResponse
    {
        $data = $request->validate([
            'reason' => ['required', 'string', 'min:20', 'max:500'],
        ], [], ['reason' => 'alasan pembukaan identitas']);

        $result = AbsensiApi::make()->call('POST', "/v1/panic/reports/{$id}/unmask", $data);

        if (! $result['success']) {
            return back()->withInput()->withErrors(['reason' => $result['message']]);
        }

        $author = $result['data'] ?? [];

        // Identitas hanya diletakkan di flash session — tidak pernah tersimpan
        // di tabel mana pun milik dashboard, dan hilang setelah satu tampilan.
        return back()->with('unmasked', [
            'name' => $author['name'] ?? '-',
            'identity_number' => $author['identity_number'] ?? '-',
            'role' => $author['role'] ?? '-',
            'school_name' => $author['school_name'] ?? '-',
            'notice' => $author['notice'] ?? '',
        ]);
    }

    /** Audit pembukaan identitas di seluruh sistem. */
    public function unmaskLogs(): View
    {
        return view('backend.panic.unmask_logs', [
            'logs' => PanicUnmaskLog::orderByDesc('created_at')->paginate(50),
        ]);
    }

    /**
     * @return array<string, int>
     */
    private function stats(?\App\Models\User $user): array
    {
        $row = PanicReport::query()
            ->recent(180)
            ->visibleTo($user)
            ->selectRaw("
                COUNT(*)::int AS total,
                COUNT(*) FILTER (WHERE status = 'baru')::int AS baru,
                COUNT(*) FILTER (WHERE moderation_status = 'pending')::int AS menunggu_moderasi,
                COUNT(*) FILTER (WHERE severity = 'darurat' AND handled_at IS NULL)::int AS darurat,
                COUNT(*) FILTER (WHERE status = 'selesai')::int AS selesai
            ")
            ->first();

        return [
            'total' => (int) ($row->total ?? 0),
            'baru' => (int) ($row->baru ?? 0),
            'menunggu_moderasi' => (int) ($row->menunggu_moderasi ?? 0),
            'darurat' => (int) ($row->darurat ?? 0),
            'selesai' => (int) ($row->selesai ?? 0),
        ];
    }
}
