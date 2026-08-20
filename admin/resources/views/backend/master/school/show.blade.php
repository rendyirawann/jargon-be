@extends('backend.layout.app')
@section('title', $school->name)

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">{{ $school->name }}</h2>
            <span class="text-muted fs-7">
                NPSN {{ $school->npsn }} &middot; {{ $school->jenjang }} {{ ucfirst($school->status) }}
                &middot; {{ $school->region->name ?? 'wilayah belum diisi' }}
            </span>
        </div>
        <div class="d-flex gap-2">
            <a href="{{ route('dashboard', ['school_id' => $school->id]) }}" class="btn btn-sm btn-light-info">
                Buka Dashboard Sekolah
            </a>
            @can('update_school')
                <a href="{{ route('schools.edit', $school) }}" class="btn btn-sm btn-light-warning">Ubah</a>
            @endcan
            <a href="{{ route('schools.index') }}" class="btn btn-sm btn-light">Kembali</a>
        </div>
    </div>

    <div class="row g-5 mb-5">
        @foreach ([
            ['Siswa Aktif', $stats['students'], 'primary'],
            ['Wajah Terdaftar', $stats['enrolled'], 'success'],
            ['Kelas Aktif', $stats['classrooms'], 'info'],
            ['Tablet Aktif', $stats['devices'], 'warning'],
        ] as [$label, $value, $color])
            <div class="col-6 col-xl-3">
                <div class="card card-flush border border-gray-200">
                    <div class="card-body p-5">
                        <span class="text-muted fs-8 text-uppercase d-block mb-2">{{ $label }}</span>
                        <span class="fs-2hx fw-bold text-{{ $color }}">{{ number_format($value) }}</span>
                    </div>
                </div>
            </div>
        @endforeach
    </div>

    <div class="row g-5">
        <div class="col-xl-6">
            <div class="card card-flush border border-gray-200 h-100">
                <div class="card-header pt-5"><h3 class="card-title fw-bold">Identitas</h3></div>
                <div class="card-body pt-3">
                    @foreach ([
                        'Alamat' => $school->address,
                        'Kelurahan / Desa' => $school->village,
                        'Kecamatan' => $school->district,
                        'Kode pos' => $school->postal_code,
                        'Telepon' => $school->phone,
                        'Email' => $school->email,
                        'Kepala sekolah' => $school->principal_name,
                        'Zona waktu' => $school->timezone,
                    ] as $label => $value)
                        <div class="d-flex justify-content-between border-bottom border-gray-200 py-3">
                            <span class="text-muted fs-7">{{ $label }}</span>
                            <span class="fs-7 fw-semibold text-end">{{ $value ?: '-' }}</span>
                        </div>
                    @endforeach
                </div>
            </div>
        </div>

        <div class="col-xl-6">
            <div class="card card-flush border border-gray-200 h-100">
                <div class="card-header pt-5"><h3 class="card-title fw-bold">Konfigurasi Absensi</h3></div>
                <div class="card-body pt-3">
                    <div class="d-flex justify-content-between border-bottom border-gray-200 py-3">
                        <span class="text-muted fs-7">Ambang kemiripan wajah</span>
                        <span class="fs-7 fw-semibold">
                            {{ $school->face_match_threshold ?? 'default global (0.62)' }}
                        </span>
                    </div>
                    <div class="d-flex justify-content-between border-bottom border-gray-200 py-3">
                        <span class="text-muted fs-7">Koordinat</span>
                        <span class="fs-7 fw-semibold">
                            {{ $school->latitude && $school->longitude
                                ? $school->latitude.', '.$school->longitude
                                : 'belum diisi' }}
                        </span>
                    </div>
                    <div class="d-flex justify-content-between border-bottom border-gray-200 py-3">
                        <span class="text-muted fs-7">Radius geofence</span>
                        <span class="fs-7 fw-semibold">{{ $school->geofence_radius_m }} m</span>
                    </div>
                    <div class="d-flex justify-content-between py-3">
                        <span class="text-muted fs-7">Status</span>
                        <span class="badge badge-light-{{ $school->is_active ? 'success' : 'danger' }}">
                            {{ $school->is_active ? 'Aktif' : 'Nonaktif' }}
                        </span>
                    </div>

                    <div class="separator my-4"></div>
                    <div class="d-flex flex-wrap gap-2">
                        @can('view_classroom')
                            <a href="{{ route('classrooms.index', ['school_id' => $school->id]) }}" class="btn btn-sm btn-light">Kelas</a>
                        @endcan
                        @can('view_student')
                            <a href="{{ route('students.index', ['school_id' => $school->id]) }}" class="btn btn-sm btn-light">Siswa</a>
                        @endcan
                        @can('view_device')
                            <a href="{{ route('devices.index', ['school_id' => $school->id]) }}" class="btn btn-sm btn-light">Perangkat</a>
                        @endcan
                        @can('manage_attendance_rule')
                            <a href="{{ route('attendance-rules.index', ['school_id' => $school->id]) }}" class="btn btn-sm btn-light">Jam Absensi</a>
                        @endcan
                    </div>
                </div>
            </div>
        </div>
    </div>
@endsection
