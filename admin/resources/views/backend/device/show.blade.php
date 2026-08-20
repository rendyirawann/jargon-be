@extends('backend.layout.app')
@section('title', 'Perangkat '.$device->code)

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">{{ $device->code }}</h2>
            <span class="text-muted fs-7">
                {{ $device->name }} &middot; {{ $device->school->name ?? '-' }} &middot;
                {{ $device->placement_label }}{{ $device->classroom ? ' ('.$device->classroom->name.')' : '' }}
            </span>
        </div>
        <div class="d-flex gap-2">
            <span class="badge {{ $device->status_badge }} fs-7 py-2 px-3">{{ $device->status_label }}</span>
            <a href="{{ route('devices.index') }}" class="btn btn-sm btn-light">Kembali</a>
        </div>
    </div>

    <div class="row g-5">
        <div class="col-xl-4">
            <div class="card card-flush border border-gray-200 mb-5">
                <div class="card-header pt-5"><h3 class="card-title fw-bold">Aktivitas Hari Ini</h3></div>
                <div class="card-body pt-3">
                    @php $t = $todayScans; @endphp
                    <div class="d-flex justify-content-between mb-3">
                        <span class="text-muted fs-7">Total percobaan scan</span>
                        <span class="fw-bold">{{ number_format($t->total ?? 0) }}</span>
                    </div>
                    <div class="d-flex justify-content-between mb-3">
                        <span class="text-muted fs-7">Diterima</span>
                        <span class="fw-bold text-success">{{ number_format($t->accepted ?? 0) }}</span>
                    </div>
                    <div class="d-flex justify-content-between mb-3">
                        <span class="text-muted fs-7">Ditolak</span>
                        <span class="fw-bold text-danger">{{ number_format($t->rejected ?? 0) }}</span>
                    </div>
                    <div class="d-flex justify-content-between mb-3">
                        <span class="text-muted fs-7">Wajah tak dikenali</span>
                        <span class="fw-bold text-warning">{{ number_format($t->unknown ?? 0) }}</span>
                    </div>
                    <div class="separator my-3"></div>
                    <div class="d-flex justify-content-between">
                        <span class="text-muted fs-7">Rata-rata waktu proses</span>
                        <span class="fw-bold">{{ $t->avg_latency ? $t->avg_latency.' ms' : '-' }}</span>
                    </div>

                    @if (($t->unknown ?? 0) > 10)
                        <div class="alert alert-light-warning mt-4 mb-0 py-3 px-4 fs-8">
                            Banyak wajah tidak dikenali. Kemungkinan ada siswa yang belum
                            didaftarkan, atau pencahayaan di lokasi perangkat kurang memadai.
                        </div>
                    @endif
                </div>
            </div>

            <div class="card card-flush border border-gray-200">
                <div class="card-body p-5">
                    <span class="fw-semibold fs-7 d-block mb-3">Informasi Perangkat</span>
                    <div class="d-flex justify-content-between fs-8 mb-2">
                        <span class="text-muted">Versi aplikasi</span><span>{{ $device->app_version ?? '-' }}</span>
                    </div>
                    <div class="d-flex justify-content-between fs-8 mb-2">
                        <span class="text-muted">Sistem operasi</span><span>{{ $device->os_version ?? '-' }}</span>
                    </div>
                    <div class="d-flex justify-content-between fs-8 mb-2">
                        <span class="text-muted">IP terakhir</span><span>{{ $device->last_ip ?? '-' }}</span>
                    </div>
                    <div class="d-flex justify-content-between fs-8 mb-2">
                        <span class="text-muted">Token diterbitkan</span>
                        <span>{{ $device->token_issued_at?->timezone(config('app.timezone'))->format('d/m/Y H:i') ?? 'belum' }}</span>
                    </div>
                    <div class="d-flex justify-content-between fs-8">
                        <span class="text-muted">Mode</span><span>{{ $device->mode_label }}</span>
                    </div>
                </div>
            </div>
        </div>

        <div class="col-xl-8">
            <div class="card card-flush border border-gray-200">
                <div class="card-header pt-5">
                    <h3 class="card-title fw-bold">Riwayat Heartbeat</h3>
                    <span class="text-muted fs-8">50 laporan terakhir</span>
                </div>
                <div class="card-body pt-3 p-0">
                    <div class="table-responsive" style="max-height: 520px; overflow-y: auto;">
                        <table class="table table-row-bordered align-middle mb-0">
                            <thead class="bg-light sticky-top">
                                <tr class="fw-bold fs-8 text-uppercase text-muted">
                                    <th class="ps-5">Waktu</th>
                                    <th class="text-center">Baterai</th>
                                    <th class="text-center">Antrean Offline</th>
                                    <th>Jaringan</th>
                                    <th class="pe-5">Versi Model</th>
                                </tr>
                            </thead>
                            <tbody>
                                @forelse ($heartbeats as $h)
                                    <tr>
                                        <td class="ps-5 fs-8">
                                            {{ \Illuminate\Support\Carbon::parse($h->reported_at)->timezone(config('app.timezone'))->format('d/m H:i:s') }}
                                        </td>
                                        <td class="text-center fs-8">
                                            @if ($h->battery_pct !== null)
                                                <span class="badge badge-light-{{ $h->battery_pct < 20 ? 'danger' : ($h->battery_pct < 50 ? 'warning' : 'success') }}">
                                                    {{ $h->battery_pct }}%
                                                </span>
                                            @else
                                                -
                                            @endif
                                        </td>
                                        <td class="text-center fs-8">
                                            @if ($h->queued_events > 0)
                                                <span class="badge badge-light-warning">{{ $h->queued_events }}</span>
                                            @else
                                                0
                                            @endif
                                        </td>
                                        <td class="fs-8">{{ $h->network ?? '-' }}</td>
                                        <td class="pe-5 fs-8"><code>{{ $h->embedding_model_version ?? '-' }}</code></td>
                                    </tr>
                                @empty
                                    <tr>
                                        <td colspan="5" class="text-center text-muted py-10 fs-7">
                                            Perangkat belum pernah mengirim heartbeat.
                                            Pastikan tablet sudah dipasangkan dan terhubung ke jaringan.
                                        </td>
                                    </tr>
                                @endforelse
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>
        </div>
    </div>
@endsection
