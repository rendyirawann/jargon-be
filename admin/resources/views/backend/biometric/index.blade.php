@extends('backend.layout.app')
@section('title', 'Pendaftaran Wajah')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Pendaftaran Wajah Siswa</h2>
            <span class="text-muted fs-7">
                Siswa hanya bisa absen setelah wajahnya terdaftar. Dianjurkan
                {{ \App\Models\Student::RECOMMENDED_SAMPLES }} sampel dari sudut berbeda.
            </span>
        </div>
        @include('backend.partials._school_picker', ['schools' => $schools, 'schoolId' => $schoolId, 'allowAll' => false])
    </div>

    <div class="row g-3 mb-5">
        @foreach ([
            ['Total Siswa Aktif', $coverage['total'], 'gray-900', null],
            ['Sudah Terdaftar', $coverage['enrolled'], 'success', 'lengkap'],
            ['Belum Terdaftar', $coverage['not_enrolled'], 'danger', 'belum'],
            ['Sampel Kurang', $coverage['under_sampled'], 'warning', 'kurang'],
        ] as [$label, $value, $color, $filterKey])
            <div class="col-6 col-xl-3">
                <a href="{{ $filterKey ? route('biometric.index', ['school_id' => $schoolId, 'filter' => $filterKey]) : '#' }}"
                   class="card card-flush border border-gray-200 h-100 {{ $filter === $filterKey ? 'border-primary' : '' }}">
                    <div class="card-body p-5">
                        <span class="text-muted fs-8 text-uppercase d-block mb-2">{{ $label }}</span>
                        <span class="fs-2hx fw-bold text-{{ $color }}">{{ number_format($value) }}</span>
                    </div>
                </a>
            </div>
        @endforeach
    </div>

    <div class="card card-flush border border-gray-200 mb-5">
        <div class="card-body p-5">
            <div class="d-flex align-items-center justify-content-between mb-2">
                <span class="fw-semibold fs-7">Cakupan pendaftaran</span>
                <span class="fw-bold fs-5">{{ $coverage['percent'] }}%</span>
            </div>
            <div class="progress h-10px bg-light-success">
                <div class="progress-bar bg-success" style="width: {{ min(100, $coverage['percent']) }}%"></div>
            </div>
        </div>
    </div>

    <div class="card card-flush border border-gray-200">
        <div class="card-header pt-6 pb-2">
            <form method="GET" class="row g-3 w-100 align-items-end">
                @if ($schoolId)<input type="hidden" name="school_id" value="{{ $schoolId }}">@endif
                <div class="col-md-4">
                    <label class="form-label fs-8 text-muted">Kelas</label>
                    <select name="classroom_id" class="form-select form-select-sm" onchange="this.form.submit()">
                        <option value="">Semua kelas</option>
                        @foreach ($classrooms as $c)
                            <option value="{{ $c->id }}" {{ $classroomId === $c->id ? 'selected' : '' }}>{{ $c->name }}</option>
                        @endforeach
                    </select>
                </div>
                <div class="col-md-4">
                    <label class="form-label fs-8 text-muted">Tampilkan</label>
                    <select name="filter" class="form-select form-select-sm" onchange="this.form.submit()">
                        <option value="belum" {{ $filter === 'belum' ? 'selected' : '' }}>Belum terdaftar</option>
                        <option value="kurang" {{ $filter === 'kurang' ? 'selected' : '' }}>Sampel kurang</option>
                        <option value="lengkap" {{ $filter === 'lengkap' ? 'selected' : '' }}>Sudah lengkap</option>
                        <option value="semua" {{ $filter === 'semua' ? 'selected' : '' }}>Semua siswa</option>
                    </select>
                </div>
            </form>
        </div>

        <div class="card-body pt-4 p-0">
            <div class="table-responsive">
                <table class="table table-row-bordered align-middle mb-0">
                    <thead class="bg-light">
                        <tr class="fw-bold fs-8 text-uppercase text-muted">
                            <th class="ps-5">Siswa</th>
                            <th>Kelas</th>
                            <th class="text-center">Sampel</th>
                            <th class="text-center">Status</th>
                            <th class="text-end pe-5">Aksi</th>
                        </tr>
                    </thead>
                    <tbody>
                        @forelse ($students as $s)
                            <tr>
                                <td class="ps-5">
                                    <a href="{{ route('students.show', $s) }}" class="fw-semibold fs-7 text-gray-800 text-hover-primary">
                                        {{ $s->full_name }}
                                    </a>
                                    <span class="text-muted fs-9 d-block">{{ $s->nis ?? '-' }}</span>
                                </td>
                                <td class="fs-7">{{ $s->classroom?->name ?? '-' }}</td>
                                <td class="text-center fs-7">
                                    {{ $s->face_sample_count }} / {{ \App\Models\Student::RECOMMENDED_SAMPLES }}
                                </td>
                                <td class="text-center">
                                    <span class="badge {{ $s->biometric_badge }}">
                                        {{ match ($s->biometric_status) {
                                            'lengkap' => 'Lengkap',
                                            'kurang' => 'Kurang',
                                            default => 'Belum',
                                        } }}
                                    </span>
                                </td>
                                <td class="text-end pe-5">
                                    @can('create_face_enrollment')
                                        <a href="{{ route('biometric.capture', $s) }}" class="btn btn-sm btn-light-primary py-1 px-3">
                                            {{ $s->face_enrolled ? 'Tambah sampel' : 'Daftarkan' }}
                                        </a>
                                    @else
                                        <a href="{{ route('biometric.show', $s) }}" class="btn btn-sm btn-light py-1 px-3">Lihat</a>
                                    @endcan
                                </td>
                            </tr>
                        @empty
                            <tr>
                                <td colspan="5" class="text-center text-muted py-10 fs-7">
                                    @if (! $schoolId)
                                        Pilih sekolah terlebih dahulu.
                                    @elseif ($filter === 'belum')
                                        Semua siswa aktif sudah terdaftar wajahnya.
                                    @else
                                        Tidak ada siswa pada filter ini.
                                    @endif
                                </td>
                            </tr>
                        @endforelse
                    </tbody>
                </table>
            </div>

            @if ($students instanceof \Illuminate\Contracts\Pagination\Paginator || $students instanceof \Illuminate\Pagination\LengthAwarePaginator)
                <div class="p-5">{{ $students->links() }}</div>
            @endif
        </div>
    </div>
@endsection
