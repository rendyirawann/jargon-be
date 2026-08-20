@extends('backend.layout.app')
@section('title', 'Pemberkasan')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Pemberkasan Kepegawaian</h2>
            <span class="text-muted fs-7">
                Verifikasi berkas yang diunggah guru dan staf melalui aplikasi Jargon GO.
            </span>
        </div>
        @can('manage_document_type')
            <a href="{{ route('documents.types') }}" class="btn btn-sm btn-light-primary">
                Jenis Dokumen
            </a>
        @endcan
    </div>

    <div class="row g-3 mb-5">
        @foreach ([
            ['Total Pengajuan', $stats['total'], 'gray-900', null],
            ['Menunggu Diperiksa', $stats['menunggu'], 'warning', ['antrean' => 1]],
            ['Perlu Perbaikan', $stats['revisi'], 'info', ['status' => 'revisi']],
            ['Disetujui', $stats['disetujui'], 'success', ['status' => 'disetujui']],
        ] as [$label, $value, $color, $filter])
            <div class="col-6 col-xl-3">
                <a href="{{ $filter ? route('documents.index', $filter) : route('documents.index') }}"
                   class="card card-flush border border-gray-200 h-100">
                    <div class="card-body p-5">
                        <span class="text-muted fs-8 text-uppercase d-block mb-2">{{ $label }}</span>
                        <span class="fs-2hx fw-bold text-{{ $color }}">{{ number_format($value) }}</span>
                    </div>
                </a>
            </div>
        @endforeach
    </div>

    <div class="card card-flush border border-gray-200">
        <div class="card-header pt-6 pb-2">
            <form method="GET" class="row g-3 w-100 align-items-end">
                <div class="col-6 col-md-3">
                    <label class="form-label fs-8 text-muted">Keperluan</label>
                    <select name="purpose" class="form-select form-select-sm" onchange="this.form.submit()">
                        <option value="">Semua keperluan</option>
                        @foreach ($purposes as $key => $label)
                            <option value="{{ $key }}" {{ request('purpose') === $key ? 'selected' : '' }}>
                                {{ $label }}
                            </option>
                        @endforeach
                    </select>
                </div>
                <div class="col-6 col-md-3">
                    <label class="form-label fs-8 text-muted">Status</label>
                    <select name="status" class="form-select form-select-sm" onchange="this.form.submit()">
                        <option value="">Semua status</option>
                        @foreach ($statuses as $key => $label)
                            <option value="{{ $key }}" {{ request('status') === $key ? 'selected' : '' }}>
                                {{ $label }}
                            </option>
                        @endforeach
                    </select>
                </div>
                <div class="col-6 col-md-3">
                    <a href="{{ route('documents.index') }}" class="btn btn-sm btn-light w-100">Reset</a>
                </div>
            </form>
        </div>

        <div class="card-body pt-4 p-0">
            <div class="table-responsive">
                <table class="table table-row-bordered align-middle mb-0">
                    <thead class="bg-light">
                        <tr class="fw-bold fs-8 text-uppercase text-muted">
                            <th class="ps-5">Pengajuan</th>
                            <th>Pengusul</th>
                            <th>Sekolah</th>
                            <th class="text-center">Berkas</th>
                            <th>Status</th>
                            <th class="pe-5">Diajukan</th>
                        </tr>
                    </thead>
                    <tbody>
                        @forelse ($submissions as $s)
                            <tr>
                                <td class="ps-5" style="max-width: 320px;">
                                    <a href="{{ route('documents.show', $s->id) }}"
                                       class="fw-semibold fs-7 text-gray-800 text-hover-primary">
                                        {{ $s->title }}
                                    </a>
                                    <div class="d-flex align-items-center gap-2 mt-1">
                                        <span class="badge badge-light fs-9">{{ $s->purpose_label }}</span>
                                        @if ($s->period)
                                            <span class="text-muted fs-9">{{ $s->period }}</span>
                                        @endif
                                    </div>
                                </td>
                                <td class="fs-8">
                                    <span class="fw-semibold d-block">{{ $s->owner->name ?? '-' }}</span>
                                    <span class="text-muted fs-9">{{ $s->owner->employee_no ?? '' }}</span>
                                </td>
                                <td class="fs-8">{{ $s->school->name ?? '-' }}</td>
                                <td class="text-center fs-8">
                                    <span class="fw-bold">{{ $s->file_count }}</span>
                                    @if ($s->rejected_file_count > 0)
                                        <span class="badge badge-light-danger fs-9 ms-1">
                                            {{ $s->rejected_file_count }} ditolak
                                        </span>
                                    @endif
                                </td>
                                <td><span class="badge {{ $s->status_badge }}">{{ $s->status_label }}</span></td>
                                <td class="pe-5 fs-8 text-muted">
                                    {{ $s->submitted_at
                                        ? $s->submitted_at->timezone(config('app.timezone'))->format('d/m/Y H:i')
                                        : 'belum diajukan' }}
                                </td>
                            </tr>
                        @empty
                            <tr>
                                <td colspan="6" class="text-center text-muted py-10 fs-7">
                                    Tidak ada pengajuan pada filter ini.
                                </td>
                            </tr>
                        @endforelse
                    </tbody>
                </table>
            </div>

            <div class="p-5">{{ $submissions->links() }}</div>
        </div>
    </div>
@endsection
