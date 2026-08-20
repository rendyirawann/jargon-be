@extends('backend.layout.app')
@section('title', 'Rekap per Siswa')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Rekap Kehadiran per Siswa</h2>
            <span class="text-muted fs-7">
                {{ \Illuminate\Support\Carbon::parse($from)->translatedFormat('d M Y') }}
                &ndash; {{ \Illuminate\Support\Carbon::parse($to)->translatedFormat('d M Y') }}
                @if ($rows->isNotEmpty())
                    &middot; {{ $rows->first()->effective_days }} hari efektif
                @endif
            </span>
        </div>
        @include('backend.partials._school_picker', ['schools' => $schools, 'schoolId' => $schoolId, 'allowAll' => false])
    </div>

    <div class="card card-flush border border-gray-200">
        <div class="card-header pt-6 pb-2">
            <form method="GET" class="row g-3 w-100 align-items-end">
                @if ($schoolId)<input type="hidden" name="school_id" value="{{ $schoolId }}">@endif
                <div class="col-6 col-md-2">
                    <label class="form-label fs-8 text-muted">Dari</label>
                    <input type="date" name="from" class="form-control form-control-sm"
                           value="{{ \Illuminate\Support\Carbon::parse($from)->toDateString() }}" max="{{ now()->toDateString() }}">
                </div>
                <div class="col-6 col-md-2">
                    <label class="form-label fs-8 text-muted">Sampai</label>
                    <input type="date" name="to" class="form-control form-control-sm"
                           value="{{ \Illuminate\Support\Carbon::parse($to)->toDateString() }}" max="{{ now()->toDateString() }}">
                </div>
                <div class="col-12 col-md-4">
                    <label class="form-label fs-8 text-muted">Kelas</label>
                    <select name="classroom_id" class="form-select form-select-sm">
                        <option value="">Semua kelas</option>
                        @foreach ($classrooms as $c)
                            <option value="{{ $c->id }}" {{ $classroomId === $c->id ? 'selected' : '' }}>{{ $c->name }}</option>
                        @endforeach
                    </select>
                </div>
                <div class="col-12 col-md-4 d-flex gap-2">
                    <button class="btn btn-sm btn-primary">Terapkan</button>
                    <button type="button" class="btn btn-sm btn-light-success" onclick="exportRecapCsv()">
                        <i class="ki-outline ki-file-down fs-5 me-1"></i>Unduh CSV
                    </button>
                </div>
            </form>
        </div>

        <div class="card-body pt-4">
            <div class="table-responsive">
                <table class="table table-row-bordered table-row-gray-200 align-middle" id="tblRecap">
                    <thead>
                        <tr class="fw-bold fs-8 text-uppercase text-muted">
                            <th>Siswa</th>
                            <th>Kelas</th>
                            <th class="text-center">Hadir</th>
                            <th class="text-center">Terlambat</th>
                            <th class="text-center">Izin</th>
                            <th class="text-center">Sakit</th>
                            <th class="text-center">Alfa</th>
                            <th class="text-center">Total Telat</th>
                            <th class="text-center">% Hadir</th>
                        </tr>
                    </thead>
                    <tbody>
                        @forelse ($rows as $r)
                            @php
                                $days = (int) $r->effective_days;
                                $pct = $days > 0 ? round(($r->hadir + $r->terlambat) / $days * 100, 1) : 0;
                                $color = $pct >= 90 ? 'success' : ($pct >= 75 ? 'warning' : 'danger');
                            @endphp
                            <tr>
                                <td>
                                    <a href="{{ route('students.show', $r->id) }}" class="fw-semibold fs-7 text-gray-800 text-hover-primary">
                                        {{ $r->full_name }}
                                    </a>
                                    <span class="text-muted fs-9 d-block">{{ $r->nis ?? '-' }}</span>
                                </td>
                                <td class="fs-7">{{ $r->classroom_name ?? '-' }}</td>
                                <td class="text-center fw-bold text-success">{{ $r->hadir }}</td>
                                <td class="text-center fw-bold text-warning">{{ $r->terlambat }}</td>
                                <td class="text-center">{{ $r->izin }}</td>
                                <td class="text-center">{{ $r->sakit }}</td>
                                <td class="text-center fw-bold text-danger">{{ $r->alfa }}</td>
                                <td class="text-center fs-7">{{ $r->total_late }} menit</td>
                                <td class="text-center"><span class="badge badge-light-{{ $color }}">{{ $pct }}%</span></td>
                            </tr>
                        @empty
                            <tr>
                                <td colspan="9" class="text-center text-muted py-10 fs-7">
                                    @if (! $schoolId)
                                        Pilih sekolah terlebih dahulu untuk melihat rekap.
                                    @else
                                        Belum ada data absensi pada rentang tanggal ini.
                                    @endif
                                </td>
                            </tr>
                        @endforelse
                    </tbody>
                </table>
            </div>
        </div>
    </div>
@endsection

@push('scripts')
    <script>
        $(function () {
            // Rekap dimuat penuh (satu sekolah, satu rentang), jadi DataTables
            // client-side sudah cukup dan pencarian menjadi instan.
            $('#tblRecap').DataTable({
                pageLength: 50,
                order: [],
                language: {
                    emptyTable: 'Tidak ada data.',
                    search: 'Cari siswa:',
                    lengthMenu: 'Tampilkan _MENU_ baris',
                    info: 'Menampilkan _START_&ndash;_END_ dari _TOTAL_ siswa',
                    paginate: { first: 'Awal', last: 'Akhir', next: 'Berikutnya', previous: 'Sebelumnya' },
                },
            });
        });

        // Ekspor dilakukan di sisi klien dari tabel yang sudah tampil —
        // menghindari query ulang untuk data yang sudah ada di halaman.
        function exportRecapCsv() {
            const rows = [];
            document.querySelectorAll('#tblRecap thead tr, #tblRecap tbody tr').forEach(function (tr) {
                const cells = Array.from(tr.querySelectorAll('th, td')).map(function (td) {
                    const text = td.innerText.replace(/\s+/g, ' ').trim();
                    return '"' + text.replace(/"/g, '""') + '"';
                });
                if (cells.length > 1) rows.push(cells.join(','));
            });

            // BOM agar Excel di Windows membaca UTF-8 dengan benar.
            const blob = new Blob(['﻿' + rows.join('\n')], { type: 'text/csv;charset=utf-8;' });
            const link = document.createElement('a');
            link.href = URL.createObjectURL(blob);
            link.download = 'rekap-kehadiran-{{ \Illuminate\Support\Carbon::parse($from)->format('Ymd') }}-{{ \Illuminate\Support\Carbon::parse($to)->format('Ymd') }}.csv';
            link.click();
            URL.revokeObjectURL(link.href);
        }
    </script>
@endpush
