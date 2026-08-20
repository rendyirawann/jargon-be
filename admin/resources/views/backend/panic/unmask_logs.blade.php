@extends('backend.layout.app')
@section('title', 'Audit Pembukaan Identitas')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Audit Pembukaan Identitas</h2>
            <span class="text-muted fs-7">
                Setiap kali identitas pelapor dibuka, barisnya tercatat di sini secara permanen.
            </span>
        </div>
        <a href="{{ route('panic.index') }}" class="btn btn-sm btn-light">Kembali ke Pengaduan</a>
    </div>

    <div class="alert alert-light-warning d-flex align-items-start mb-5">
        <i class="ki-duotone ki-information-5 fs-2x me-4"><span class="path1"></span><span class="path2"></span><span class="path3"></span></i>
        <div class="fs-8">
            <span class="fw-bold d-block mb-1">Halaman ini tidak dapat diedit atau dihapus.</span>
            Catatan pembukaan identitas adalah satu-satunya hal yang membuat kewenangan
            <code>unmask_panic_report</code> dapat dipertanggungjawabkan. Baris di bawah
            ditulis oleh API sebelum identitas dikembalikan, sehingga tidak ada jalur
            membuka identitas tanpa meninggalkan jejak.
        </div>
    </div>

    <div class="card card-flush border border-gray-200">
        <div class="card-body p-0">
            <div class="table-responsive">
                <table class="table table-row-bordered align-middle mb-0">
                    <thead class="bg-light">
                        <tr class="fw-bold fs-8 text-uppercase text-muted">
                            <th class="ps-5">Waktu</th>
                            <th>Petugas</th>
                            <th>Alasan</th>
                            <th>Alamat IP</th>
                            <th class="pe-5 text-end">Laporan</th>
                        </tr>
                    </thead>
                    <tbody>
                        @forelse ($logs as $log)
                            <tr>
                                <td class="ps-5 fs-8 text-nowrap">
                                    {{ $log->created_at->timezone(config('app.timezone'))->format('d/m/Y H:i') }}
                                </td>
                                <td class="fs-7 fw-semibold">{{ $log->actor_label }}</td>
                                <td class="fs-8 text-gray-700" style="max-width: 460px;">{{ $log->reason }}</td>
                                <td class="fs-8 text-muted">{{ $log->ip_address ?? '-' }}</td>
                                <td class="pe-5 text-end">
                                    <a href="{{ route('panic.show', $log->report_id) }}"
                                       class="btn btn-sm btn-light-primary">Lihat</a>
                                </td>
                            </tr>
                        @empty
                            <tr>
                                <td colspan="5" class="text-center text-muted py-10 fs-7">
                                    Belum ada identitas pelapor yang pernah dibuka.
                                </td>
                            </tr>
                        @endforelse
                    </tbody>
                </table>
            </div>

            <div class="p-5">{{ $logs->links() }}</div>
        </div>
    </div>
@endsection
