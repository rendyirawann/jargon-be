@extends('backend.layout.app')
@section('title', 'Detail Pengajuan')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">{{ $submission->title }}</h2>
            <span class="text-muted fs-7">
                {{ $submission->purpose_label }}
                @if ($submission->period)
                    &middot; {{ $submission->period }}
                @endif
                &middot; {{ $submission->owner->name ?? '-' }}
                &middot; {{ $submission->school->name ?? '-' }}
            </span>
        </div>
        <a href="{{ route('documents.index') }}" class="btn btn-sm btn-light">Kembali</a>
    </div>

    <div class="row g-5">
        <div class="col-xl-8">
            {{-- Daftar periksa: disusun dari jenis dokumen, bukan dari berkas
                 yang sudah diunggah, supaya yang KURANG ikut terlihat. --}}
            <div class="card card-flush border border-gray-200 mb-5">
                <div class="card-header pt-5">
                    <h3 class="card-title fw-bold">Daftar Periksa Berkas</h3>
                    <div class="card-toolbar">
                        <span class="badge badge-light-success">{{ $submission->approved_file_count }} disetujui</span>
                        <span class="badge badge-light-danger ms-2">{{ $submission->rejected_file_count }} ditolak</span>
                    </div>
                </div>
                <div class="card-body pt-3 p-0">
                    <div class="table-responsive">
                        <table class="table table-row-bordered align-middle mb-0">
                            <thead class="bg-light">
                                <tr class="fw-bold fs-8 text-uppercase text-muted">
                                    <th class="ps-5">Jenis Dokumen</th>
                                    <th>Berkas</th>
                                    <th>Status</th>
                                    <th class="pe-5 text-end">Tindakan</th>
                                </tr>
                            </thead>
                            <tbody>
                                @foreach ($checklist as $item)
                                    @php $file = $item['file']; @endphp
                                    <tr>
                                        <td class="ps-5" style="max-width: 260px;">
                                            <span class="fw-semibold fs-7 d-block">
                                                {{ $item['type']->name }}
                                                @if ($item['type']->is_required)
                                                    <span class="text-danger">*</span>
                                                @endif
                                            </span>
                                            @if ($item['type']->description)
                                                <span class="text-muted fs-9">{{ $item['type']->description }}</span>
                                            @endif
                                        </td>
                                        <td class="fs-8">
                                            @if ($file)
                                                <a href="{{ $file->file_url }}" target="_blank" rel="noopener"
                                                   class="text-gray-800 text-hover-primary fw-semibold d-block">
                                                    {{ $file->original_name }}
                                                </a>
                                                <span class="text-muted fs-9">
                                                    {{ $file->size_label }} &middot;
                                                    {{ $file->uploaded_at->timezone(config('app.timezone'))->format('d/m/Y H:i') }}
                                                </span>
                                            @else
                                                <span class="text-muted fs-8">belum diunggah</span>
                                            @endif
                                        </td>
                                        <td>
                                            @if ($file)
                                                <span class="badge {{ $file->status_badge }}">{{ ucfirst($file->status) }}</span>
                                                @if ($file->reject_reason)
                                                    <span class="text-danger fs-9 d-block mt-1">{{ $file->reject_reason }}</span>
                                                @endif
                                            @elseif ($item['type']->is_required)
                                                <span class="badge badge-light-danger">wajib, belum ada</span>
                                            @else
                                                <span class="badge badge-light">opsional</span>
                                            @endif
                                        </td>
                                        <td class="pe-5 text-end">
                                            @if ($file)
                                                @can('verify_document_submission')
                                                    <div class="d-flex justify-content-end gap-2">
                                                        <form method="POST" action="{{ route('documents.files.review', $file->id) }}">
                                                            @csrf
                                                            <input type="hidden" name="status" value="disetujui">
                                                            <button class="btn btn-sm btn-light-success">Setujui</button>
                                                        </form>
                                                        <button type="button" class="btn btn-sm btn-light-danger"
                                                                data-bs-toggle="collapse"
                                                                data-bs-target="#tolak-{{ $file->id }}">
                                                            Tolak
                                                        </button>
                                                    </div>
                                                    <div class="collapse mt-2" id="tolak-{{ $file->id }}">
                                                        <form method="POST" action="{{ route('documents.files.review', $file->id) }}"
                                                              class="text-start">
                                                            @csrf
                                                            <input type="hidden" name="status" value="ditolak">
                                                            <input type="text" name="reject_reason"
                                                                   class="form-control form-control-sm mb-2"
                                                                   required minlength="5" maxlength="300"
                                                                   placeholder="Alasan penolakan berkas ini">
                                                            <button class="btn btn-sm btn-danger w-100">Tolak Berkas</button>
                                                        </form>
                                                    </div>
                                                @endcan
                                            @endif
                                        </td>
                                    </tr>
                                @endforeach
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>

            @if ($extraFiles->isNotEmpty())
                <div class="card card-flush border border-gray-200 mb-5">
                    <div class="card-header pt-5"><h3 class="card-title fw-bold">Berkas Tambahan</h3></div>
                    <div class="card-body pt-3">
                        @foreach ($extraFiles as $f)
                            <div class="d-flex align-items-center justify-content-between border-bottom border-gray-200 py-3">
                                <div>
                                    <a href="{{ $f->file_url }}" target="_blank" rel="noopener"
                                       class="fw-semibold fs-8 text-gray-800 text-hover-primary d-block">
                                        {{ $f->original_name }}
                                    </a>
                                    <span class="text-muted fs-9">{{ $f->size_label }}</span>
                                </div>
                                <span class="badge {{ $f->status_badge }}">{{ ucfirst($f->status) }}</span>
                            </div>
                        @endforeach
                    </div>
                </div>
            @endif

            <div class="card card-flush border border-gray-200">
                <div class="card-header pt-5"><h3 class="card-title fw-bold">Lini Masa</h3></div>
                <div class="card-body pt-3">
                    @forelse ($submission->events as $e)
                        <div class="border-bottom border-gray-200 py-3">
                            <span class="fw-semibold fs-7 d-block">
                                {{ \App\Models\DocumentSubmission::STATUSES[$e->status] ?? ucfirst($e->status) }}
                            </span>
                            @if ($e->note)
                                <span class="text-gray-700 fs-8 d-block mt-1">{{ $e->note }}</span>
                            @endif
                            <span class="text-muted fs-9 d-block mt-1">
                                {{ $e->actor_label ?? 'Sistem' }} &middot;
                                {{ $e->created_at->timezone(config('app.timezone'))->format('d/m/Y H:i') }}
                            </span>
                        </div>
                    @empty
                        <span class="text-muted fs-7">Belum ada riwayat.</span>
                    @endforelse
                </div>
            </div>
        </div>

        <div class="col-xl-4">
            <div class="card card-flush border border-gray-200 mb-5">
                <div class="card-body p-5">
                    <span class="badge {{ $submission->status_badge }} mb-4">{{ $submission->status_label }}</span>

                    <div class="fs-8 mb-2">
                        <span class="text-muted d-block">Pengusul</span>
                        <span class="fw-semibold">{{ $submission->owner->name ?? '-' }}</span>
                        @if ($submission->owner?->position)
                            <span class="text-muted d-block">{{ $submission->owner->position }}</span>
                        @endif
                    </div>
                    <div class="fs-8 mb-2">
                        <span class="text-muted d-block">NIP / NIK</span>
                        <span class="fw-semibold">
                            {{ $submission->owner->employee_no ?? $submission->owner->identity_number ?? '-' }}
                        </span>
                    </div>
                    <div class="fs-8 mb-2">
                        <span class="text-muted d-block">Diajukan</span>
                        <span class="fw-semibold">
                            {{ $submission->submitted_at
                                ? $submission->submitted_at->timezone(config('app.timezone'))->format('d/m/Y H:i')
                                : 'belum diajukan' }}
                        </span>
                    </div>
                    @if ($submission->reviewed_at)
                        <div class="fs-8 mb-2">
                            <span class="text-muted d-block">Diperiksa</span>
                            <span class="fw-semibold">
                                {{ $submission->reviewer->name ?? '-' }} &middot;
                                {{ $submission->reviewed_at->timezone(config('app.timezone'))->format('d/m/Y H:i') }}
                            </span>
                        </div>
                    @endif
                    @if ($submission->note)
                        <div class="fs-8 mt-4">
                            <span class="text-muted d-block mb-1">Catatan pengusul</span>
                            <span class="text-gray-800">{{ $submission->note }}</span>
                        </div>
                    @endif
                </div>
            </div>

            @if ($submission->review_note)
                <div class="alert alert-light-info mb-5">
                    <span class="fw-bold d-block mb-1">Catatan Pemeriksaan Terakhir</span>
                    <span class="fs-8">{{ $submission->review_note }}</span>
                </div>
            @endif

            @can('verify_document_submission')
                <div class="card card-flush border border-gray-200">
                    <div class="card-header pt-5"><h3 class="card-title fw-bold">Hasil Pemeriksaan</h3></div>
                    <form method="POST" action="{{ route('documents.review', $submission->id) }}" class="card-body pt-3">
                        @csrf
                        <div class="mb-3">
                            <label class="form-label required">Keputusan</label>
                            <select name="status" class="form-select form-select-sm" required>
                                <option value="diperiksa">Sedang diperiksa</option>
                                <option value="revisi">Perlu perbaikan</option>
                                <option value="disetujui">Disetujui</option>
                                <option value="ditolak">Ditolak</option>
                            </select>
                        </div>
                        <div class="mb-3">
                            <label class="form-label required">Catatan pemeriksaan</label>
                            <textarea name="note" class="form-control form-control-sm" rows="4" required
                                      minlength="3" maxlength="2000"
                                      placeholder="Sebutkan berkas mana yang bermasalah dan apa yang harus diperbaiki."></textarea>
                            <span class="form-text fs-9">
                                Catatan ini dibaca pengusul di aplikasi. Menolak tanpa alasan jelas
                                membuat berkas yang sama diunggah berulang kali.
                            </span>
                        </div>
                        <button class="btn btn-sm btn-primary w-100">Simpan Pemeriksaan</button>
                    </form>
                </div>
            @endcan
        </div>
    </div>
@endsection
