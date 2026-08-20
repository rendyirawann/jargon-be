@extends('backend.layout.app')
@section('title', 'Rekap per Kelas')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Rekap Absensi per Kelas</h2>
            <span class="text-muted fs-7">{{ \Illuminate\Support\Carbon::parse($date)->translatedFormat('l, d F Y') }}</span>
        </div>
        <div class="d-flex flex-wrap align-items-center gap-3">
            @include('backend.partials._school_picker', ['schools' => $schools, 'schoolId' => $schoolId, 'allowAll' => false])
            <form method="GET" class="d-flex align-items-center gap-2">
                @if ($schoolId)<input type="hidden" name="school_id" value="{{ $schoolId }}">@endif
                <input type="date" name="date" class="form-control form-control-sm w-150px"
                       value="{{ \Illuminate\Support\Carbon::parse($date)->toDateString() }}"
                       max="{{ now()->toDateString() }}" onchange="this.form.submit()">
            </form>
        </div>
    </div>

    <div class="card card-flush border border-gray-200">
        <div class="card-body p-0">
            <div class="table-responsive">
                <table class="table table-row-bordered table-row-gray-200 align-middle mb-0">
                    <thead class="bg-light">
                        <tr class="fw-bold fs-8 text-uppercase text-muted">
                            <th class="ps-5">Kelas</th>
                            <th class="text-center">Siswa</th>
                            <th class="text-center">Hadir</th>
                            <th class="text-center">Terlambat</th>
                            <th class="text-center">Izin</th>
                            <th class="text-center">Sakit</th>
                            <th class="text-center">Alfa</th>
                            <th class="text-center">Belum Absen</th>
                            <th class="text-center pe-5">Kehadiran</th>
                        </tr>
                    </thead>
                    <tbody>
                        @forelse ($rows as $r)
                            @php
                                $present = $r->hadir + $r->terlambat;
                                $rate = $r->total_students > 0 ? round($present / $r->total_students * 100, 1) : 0;
                                $color = $rate >= 90 ? 'success' : ($rate >= 75 ? 'warning' : 'danger');
                            @endphp
                            <tr>
                                <td class="ps-5">
                                    <span class="fw-semibold fs-7 text-gray-800">{{ $r->name }}</span>
                                    <span class="text-muted fs-9 d-block">
                                        Tingkat {{ $r->grade_level }} &middot;
                                        {{ $r->homeroom_teacher_name ?? 'belum ada wali kelas' }}
                                    </span>
                                </td>
                                <td class="text-center">{{ $r->total_students }}</td>
                                <td class="text-center fw-bold text-success">{{ $r->hadir }}</td>
                                <td class="text-center fw-bold text-warning">{{ $r->terlambat }}</td>
                                <td class="text-center">{{ $r->izin }}</td>
                                <td class="text-center">{{ $r->sakit }}</td>
                                <td class="text-center fw-bold text-danger">{{ $r->alfa }}</td>
                                <td class="text-center">
                                    @if ($r->belum_absen > 0)
                                        <a href="{{ route('attendances.index', [
                                            'school_id' => $schoolId,
                                            'classroom_id' => $r->id,
                                            'from' => \Illuminate\Support\Carbon::parse($date)->toDateString(),
                                            'to' => \Illuminate\Support\Carbon::parse($date)->toDateString(),
                                        ]) }}" class="badge badge-light-secondary">{{ $r->belum_absen }}</a>
                                    @else
                                        <i class="ki-outline ki-check text-success fs-4"></i>
                                    @endif
                                </td>
                                <td class="text-center pe-5">
                                    <span class="badge badge-light-{{ $color }} fs-7">{{ $rate }}%</span>
                                </td>
                            </tr>
                        @empty
                            <tr>
                                <td colspan="9" class="text-center text-muted py-10 fs-7">
                                    Belum ada kelas aktif untuk sekolah ini.
                                </td>
                            </tr>
                        @endforelse
                    </tbody>
                    @if ($rows->isNotEmpty())
                        <tfoot class="bg-light">
                            <tr class="fw-bold fs-7">
                                <td class="ps-5">Total</td>
                                <td class="text-center">{{ $rows->sum('total_students') }}</td>
                                <td class="text-center text-success">{{ $rows->sum('hadir') }}</td>
                                <td class="text-center text-warning">{{ $rows->sum('terlambat') }}</td>
                                <td class="text-center">{{ $rows->sum('izin') }}</td>
                                <td class="text-center">{{ $rows->sum('sakit') }}</td>
                                <td class="text-center text-danger">{{ $rows->sum('alfa') }}</td>
                                <td class="text-center">{{ $rows->sum('belum_absen') }}</td>
                                <td class="text-center pe-5">
                                    @php
                                        $total = $rows->sum('total_students');
                                        $present = $rows->sum('hadir') + $rows->sum('terlambat');
                                    @endphp
                                    {{ $total > 0 ? round($present / $total * 100, 1) : 0 }}%
                                </td>
                            </tr>
                        </tfoot>
                    @endif
                </table>
            </div>
        </div>
    </div>
@endsection
