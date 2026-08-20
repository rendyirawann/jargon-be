@extends('backend.layout.app')
@section('title', 'Akun Jargon GO')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Akun Aplikasi Jargon GO</h2>
            <span class="text-muted fs-7">
                Siswa login memakai <strong>NISN</strong>; guru, staf, dan orang tua memakai
                <strong>NIK</strong>. Pendaftaran hanya lewat halaman ini.
            </span>
        </div>
        <div class="d-flex gap-2">
            <a href="{{ route('app-accounts.bulk') }}" class="btn btn-sm btn-light-primary">
                Akun Siswa Massal
            </a>
            <a href="{{ route('app-accounts.create') }}" class="btn btn-sm btn-primary">
                Buat Akun
            </a>
        </div>
    </div>

    <div class="row g-3 mb-5">
        @foreach ([
            ['Total Akun', $stats['total'], 'gray-900', null],
            ['Akun Siswa', $stats['siswa'], 'primary', ['role' => 'siswa']],
            ['Akun Orang Tua', $stats['orang_tua'], 'info', ['role' => 'orang_tua']],
            ['Belum Ganti Sandi', $stats['belum_ganti_sandi'], 'warning', ['belum_ganti_sandi' => 1]],
        ] as [$label, $value, $color, $filter])
            <div class="col-6 col-xl-3">
                <a href="{{ $filter ? route('app-accounts.index', $filter) : route('app-accounts.index') }}"
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
                <div class="col-12 col-md-4">
                    <label class="form-label fs-8 text-muted">Cari nama atau NIK/NISN</label>
                    <input type="search" name="q" value="{{ request('q') }}"
                           class="form-control form-control-sm" placeholder="mis. 0071234567">
                </div>
                <div class="col-6 col-md-3">
                    <label class="form-label fs-8 text-muted">Peran</label>
                    <select name="role" class="form-select form-select-sm" onchange="this.form.submit()">
                        <option value="">Semua peran</option>
                        @foreach ($roles as $key => $label)
                            <option value="{{ $key }}" {{ request('role') === $key ? 'selected' : '' }}>
                                {{ $label }}
                            </option>
                        @endforeach
                    </select>
                </div>
                <div class="col-6 col-md-2">
                    <button class="btn btn-sm btn-light-primary w-100">Cari</button>
                </div>
                <div class="col-6 col-md-2">
                    <a href="{{ route('app-accounts.index') }}" class="btn btn-sm btn-light w-100">Reset</a>
                </div>
            </form>
        </div>

        <div class="card-body pt-4 p-0">
            <div class="table-responsive">
                <table class="table table-row-bordered align-middle mb-0">
                    <thead class="bg-light">
                        <tr class="fw-bold fs-8 text-uppercase text-muted">
                            <th class="ps-5">Nama</th>
                            <th>Identitas Login</th>
                            <th>Peran</th>
                            <th>Cakupan</th>
                            <th class="pe-5">Status</th>
                        </tr>
                    </thead>
                    <tbody>
                        @forelse ($accounts as $a)
                            <tr>
                                <td class="ps-5">
                                    <a href="{{ route('app-accounts.show', $a->id) }}"
                                       class="fw-semibold fs-7 text-gray-800 text-hover-primary">
                                        {{ $a->name }}
                                    </a>
                                    @if ($a->student)
                                        <span class="text-muted fs-9 d-block">
                                            data siswa: {{ $a->student->full_name }}
                                        </span>
                                    @endif
                                </td>
                                <td class="fs-8">
                                    <span class="badge badge-light fs-9 me-1">{{ $a->identity_label }}</span>
                                    <span class="fw-semibold">{{ $a->identity_number }}</span>
                                </td>
                                <td class="fs-8">{{ $a->role_label }}</td>
                                <td class="fs-8 text-muted">
                                    {{ $a->school->name ?? ($a->hasRole('orang_tua') ? 'mengikuti sekolah anaknya' : 'Provinsi') }}
                                </td>
                                <td class="pe-5">
                                    @if (! $a->is_active)
                                        <span class="badge badge-light-danger">nonaktif</span>
                                    @elseif ($a->must_change_password)
                                        <span class="badge badge-light-warning">belum ganti sandi</span>
                                    @else
                                        <span class="badge badge-light-success">aktif</span>
                                    @endif
                                </td>
                            </tr>
                        @empty
                            <tr>
                                <td colspan="5" class="text-center text-muted py-10 fs-7">
                                    Belum ada akun aplikasi pada filter ini.
                                </td>
                            </tr>
                        @endforelse
                    </tbody>
                </table>
            </div>

            <div class="p-5">{{ $accounts->links() }}</div>
        </div>
    </div>
@endsection
