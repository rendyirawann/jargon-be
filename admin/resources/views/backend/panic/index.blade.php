@extends('backend.layout.app')
@section('title', 'Panic Button')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Pengaduan Panic Button</h2>
            <span class="text-muted fs-7">
                Identitas pelapor tidak ditampilkan. Tangani laporan berdasarkan isinya.
            </span>
        </div>
    </div>

    <div class="row g-3 mb-5">
        @foreach ([
            ['Total Laporan', $stats['total'], 'gray-900', null],
            ['Belum Diproses', $stats['baru'], 'warning', ['status' => 'baru']],
            ['Menunggu Moderasi', $stats['menunggu_moderasi'], 'info', ['pending' => 1]],
            ['Darurat Belum Ditangani', $stats['darurat'], 'danger', ['urgent' => 1]],
            ['Selesai', $stats['selesai'], 'success', ['status' => 'selesai']],
        ] as [$label, $value, $color, $filter])
            <div class="col-6 col-xl">
                <a href="{{ $filter ? route('panic.index', $filter) : route('panic.index') }}"
                   class="card card-flush border border-gray-200 h-100">
                    <div class="card-body p-5">
                        <span class="text-muted fs-8 text-uppercase d-block mb-2">{{ $label }}</span>
                        <span class="fs-2hx fw-bold text-{{ $color }}">{{ number_format($value) }}</span>
                    </div>
                </a>
            </div>
        @endforeach
    </div>

    @if ($stats['darurat'] > 0)
        <div class="alert alert-danger d-flex align-items-center mb-5">
            <i class="ki-duotone ki-shield-cross fs-2x me-4"><span class="path1"></span><span class="path2"></span><span class="path3"></span></i>
            <div>
                <span class="fw-bold d-block">{{ $stats['darurat'] }} laporan DARURAT belum ditangani</span>
                <span class="fs-8">
                    Kategori kekerasan dan pelecehan diteruskan langsung ke Dinas tanpa menunggu moderasi.
                    Tangani sesegera mungkin.
                </span>
            </div>
        </div>
    @endif

    <div class="card card-flush border border-gray-200">
        <div class="card-header pt-6 pb-2">
            <form method="GET" class="row g-3 w-100 align-items-end">
                <div class="col-6 col-md-3">
                    <label class="form-label fs-8 text-muted">Kategori</label>
                    <select name="category" class="form-select form-select-sm" onchange="this.form.submit()">
                        <option value="">Semua kategori</option>
                        @foreach ($categories as $c)
                            <option value="{{ $c->code }}" {{ request('category') === $c->code ? 'selected' : '' }}>
                                {{ $c->name }}
                            </option>
                        @endforeach
                    </select>
                </div>
                <div class="col-6 col-md-2">
                    <label class="form-label fs-8 text-muted">Status</label>
                    <select name="status" class="form-select form-select-sm" onchange="this.form.submit()">
                        <option value="">Semua</option>
                        @foreach ($statuses as $s)
                            <option value="{{ $s }}" {{ request('status') === $s ? 'selected' : '' }}>{{ ucfirst($s) }}</option>
                        @endforeach
                    </select>
                </div>
                <div class="col-6 col-md-2">
                    <label class="form-label fs-8 text-muted">Keparahan</label>
                    <select name="severity" class="form-select form-select-sm" onchange="this.form.submit()">
                        <option value="">Semua</option>
                        @foreach ($severities as $s)
                            <option value="{{ $s }}" {{ request('severity') === $s ? 'selected' : '' }}>{{ ucfirst($s) }}</option>
                        @endforeach
                    </select>
                </div>
                <div class="col-6 col-md-2">
                    <a href="{{ route('panic.index') }}" class="btn btn-sm btn-light w-100">Reset</a>
                </div>
            </form>
        </div>

        <div class="card-body pt-4 p-0">
            <div class="table-responsive">
                <table class="table table-row-bordered align-middle mb-0">
                    <thead class="bg-light">
                        <tr class="fw-bold fs-8 text-uppercase text-muted">
                            <th class="ps-5">Laporan</th>
                            <th>Kategori</th>
                            <th>Sekolah</th>
                            <th class="text-center">Dukungan</th>
                            <th>Status</th>
                            <th class="pe-5">Waktu</th>
                        </tr>
                    </thead>
                    <tbody>
                        @forelse ($reports as $r)
                            <tr>
                                <td class="ps-5" style="max-width: 340px;">
                                    <a href="{{ route('panic.show', $r->id) }}"
                                       class="fw-semibold fs-7 text-gray-800 text-hover-primary">
                                        {{ $r->title }}
                                    </a>
                                    <div class="d-flex align-items-center gap-2 mt-1">
                                        {{-- Handle anonim, bukan nama pelapor. --}}
                                        <span class="badge badge-light fs-9">{{ $r->anonymous_handle }}</span>
                                        <span class="badge {{ $r->severity_badge }} fs-9">{{ ucfirst($r->severity) }}</span>
                                        @if ($r->moderation_status === 'pending')
                                            <span class="badge badge-light-info fs-9">menunggu moderasi</span>
                                        @endif
                                        @if ($r->visibility === 'terbatas')
                                            <span class="badge badge-light-dark fs-9">terbatas</span>
                                        @endif
                                    </div>
                                </td>
                                <td class="fs-8">{{ $r->category->name ?? '-' }}</td>
                                <td class="fs-8">{{ $r->school->name ?? '-' }}</td>
                                <td class="text-center fs-7 fw-bold">{{ $r->support_count }}</td>
                                <td><span class="badge {{ $r->status_badge }}">{{ $r->status_label }}</span></td>
                                <td class="pe-5 fs-8 text-muted">
                                    {{ $r->created_at->timezone(config('app.timezone'))->diffForHumans() }}
                                </td>
                            </tr>
                        @empty
                            <tr>
                                <td colspan="6" class="text-center text-muted py-10 fs-7">
                                    Tidak ada laporan pada filter ini.
                                </td>
                            </tr>
                        @endforelse
                    </tbody>
                </table>
            </div>

            <div class="p-5">{{ $reports->links() }}</div>
        </div>
    </div>
@endsection
