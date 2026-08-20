<?php

namespace App\Http\Controllers\Backend\Document;

use App\Models\DocumentSubmission;
use App\Models\DocumentType;
use App\Http\Controllers\Controller;
use App\Services\AbsensiApi;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Routing\Controllers\HasMiddleware;
use Illuminate\Routing\Controllers\Middleware;
use Illuminate\View\View;

/**
 * Verifikasi pemberkasan kepegawaian.
 *
 * Berkas di sini memuat data pribadi (NIK, nomor rekening, ijazah), sehingga
 * aturan penglihatannya lebih ketat daripada tenant biasa: tanpa izin
 * verifikasi, seseorang hanya melihat pengajuannya sendiri — bahkan untuk
 * rekan sesekolah. Itu ditegakkan scope `visibleTo` pada model.
 *
 * Perubahan status diteruskan ke API agar lini masa pengajuan ikut tercatat.
 * Menulis kolom `status` langsung dari sini akan menghasilkan pengajuan yang
 * berubah tanpa jejak siapa yang mengubahnya.
 */
class DocumentController extends Controller implements HasMiddleware
{
    public static function middleware(): array
    {
        return [
            'auth',
            new Middleware('can:view_document_submission', only: ['index', 'show']),
            new Middleware('can:verify_document_submission', only: ['review', 'reviewFile']),
            new Middleware('can:manage_document_type', only: ['types', 'storeType']),
        ];
    }

    public function index(Request $request): View
    {
        $user = $request->user();

        $query = DocumentSubmission::query()
            ->visibleTo($user)
            ->with(['owner:id,name,employee_no', 'school:id,name']);

        $query->when($request->filled('status'), fn ($q) => $q->where('status', $request->status));
        $query->when($request->filled('purpose'), fn ($q) => $q->where('purpose', $request->purpose));
        $query->when($request->boolean('antrean'), fn ($q) => $q->awaitingReview());

        $submissions = $query
            // Yang menunggu diperiksa naik ke atas: itulah pekerjaan
            // verifikator, sisanya sekadar arsip.
            ->orderByRaw("(status = 'diajukan') DESC")
            ->orderByRaw('COALESCE(submitted_at, created_at) DESC')
            ->paginate(20)
            ->withQueryString();

        return view('backend.document.index', [
            'submissions' => $submissions,
            'statuses' => DocumentSubmission::STATUSES,
            'purposes' => DocumentType::PURPOSES,
            'stats' => $this->stats($user),
        ]);
    }

    public function show(Request $request, string $id): View
    {
        $submission = DocumentSubmission::query()
            ->visibleTo($request->user())
            ->with([
                'owner:id,name,employee_no,identity_number,position',
                'school:id,name',
                'reviewer:id,name',
                'files.documentType',
                'files.reviewer:id,name',
                'events',
            ])
            ->where('document_submissions.id', $id)
            ->firstOrFail();

        // Daftar periksa disusun dari jenis dokumen untuk keperluan ini,
        // bukan dari berkas yang sudah diunggah — supaya yang KURANG juga
        // terlihat, bukan hanya yang ada.
        $types = DocumentType::where('purpose', $submission->purpose)
            ->where('is_active', true)
            ->orderBy('sort_order')
            ->get();

        $byType = $submission->files->keyBy('document_type_id');

        return view('backend.document.show', [
            'submission' => $submission,
            'checklist' => $types->map(fn ($t) => [
                'type' => $t,
                'file' => $byType->get($t->id),
            ]),
            // Berkas tambahan di luar daftar periksa.
            'extraFiles' => $submission->files->whereNull('document_type_id'),
            'statuses' => DocumentSubmission::STATUSES,
        ]);
    }

    /** Simpan hasil pemeriksaan pengajuan. */
    public function review(Request $request, string $id): RedirectResponse
    {
        $data = $request->validate([
            'status' => ['required', 'in:diperiksa,revisi,disetujui,ditolak'],
            'note' => ['required', 'string', 'min:3', 'max:2000'],
        ], [], ['note' => 'catatan pemeriksaan']);

        // Menolak atau meminta revisi tanpa alasan yang jelas membuat guru
        // mengunggah ulang berkas yang sama berkali-kali.
        if (in_array($data['status'], ['revisi', 'ditolak'], true)
            && strlen(trim($data['note'])) < 10) {
            return back()->withInput()->withErrors([
                'note' => 'Sebutkan berkas mana yang bermasalah dan apa yang harus diperbaiki.',
            ]);
        }

        $result = AbsensiApi::make()->call('POST', "/v1/documents/submissions/{$id}/review", $data);

        return $result['success']
            ? back()->with('success', $result['message'])
            : back()->withInput()->withErrors(['status' => $result['message']]);
    }

    /** Setujui / tolak satu berkas. */
    public function reviewFile(Request $request, string $fileId): RedirectResponse
    {
        $data = $request->validate([
            'status' => ['required', 'in:disetujui,ditolak'],
            'reject_reason' => ['nullable', 'string', 'max:300'],
        ]);

        if ($data['status'] === 'ditolak'
            && strlen(trim($data['reject_reason'] ?? '')) < 5) {
            return back()->withErrors([
                'reject_reason' => 'Sebutkan alasan penolakan agar berkas dapat diperbaiki.',
            ]);
        }

        $result = AbsensiApi::make()->call('POST', "/v1/documents/files/{$fileId}/review", $data);

        return $result['success']
            ? back()->with('success', 'Status berkas diperbarui.')
            : back()->withErrors(['file' => $result['message']]);
    }

    /** Kelola jenis dokumen yang diminta per keperluan. */
    public function types(Request $request): View
    {
        return view('backend.document.types', [
            'types' => DocumentType::orderBy('purpose')
                ->orderBy('sort_order')
                ->get()
                ->groupBy('purpose'),
            'purposes' => DocumentType::PURPOSES,
        ]);
    }

    public function storeType(Request $request): RedirectResponse
    {
        $data = $request->validate([
            'code' => ['required', 'string', 'max:40', 'unique:document_types,code'],
            'name' => ['required', 'string', 'max:120'],
            'description' => ['nullable', 'string', 'max:300'],
            'purpose' => ['required', 'in:'.implode(',', array_keys(DocumentType::PURPOSES))],
            'is_required' => ['nullable', 'boolean'],
            'max_mb' => ['nullable', 'integer', 'between:1,25'],
            'sort_order' => ['nullable', 'integer', 'between:0,999'],
        ], [], ['code' => 'kode dokumen', 'name' => 'nama dokumen']);

        DocumentType::create([
            'code' => $data['code'],
            'name' => $data['name'],
            'description' => $data['description'] ?? null,
            'purpose' => $data['purpose'],
            'is_required' => (bool) ($data['is_required'] ?? false),
            'max_bytes' => (($data['max_mb'] ?? 5) * 1024 * 1024),
            'allowed_mime' => ['application/pdf', 'image/jpeg', 'image/png'],
            'sort_order' => $data['sort_order'] ?? 0,
            'is_active' => true,
        ]);

        return back()->with('success', "Jenis dokumen {$data['name']} ditambahkan.");
    }

    /**
     * @return array<string, int>
     */
    private function stats(?\App\Models\User $user): array
    {
        $row = DocumentSubmission::query()
            ->visibleTo($user)
            ->selectRaw("
                COUNT(*)::int AS total,
                COUNT(*) FILTER (WHERE status = 'diajukan')::int AS menunggu,
                COUNT(*) FILTER (WHERE status = 'revisi')::int AS revisi,
                COUNT(*) FILTER (WHERE status = 'disetujui')::int AS disetujui
            ")
            ->first();

        return [
            'total' => (int) ($row->total ?? 0),
            'menunggu' => (int) ($row->menunggu ?? 0),
            'revisi' => (int) ($row->revisi ?? 0),
            'disetujui' => (int) ($row->disetujui ?? 0),
        ];
    }
}
