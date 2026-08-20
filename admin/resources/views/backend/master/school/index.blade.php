@extends('backend.layout.app')
@section('title', 'Data Sekolah')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Data Sekolah</h2>
            <span class="text-muted fs-7">Setiap sekolah adalah tenant terpisah; datanya tidak saling terlihat.</span>
        </div>
        @can('create_school')
            <a href="{{ route('schools.create') }}" class="btn btn-sm btn-primary">
                <i class="ki-outline ki-plus fs-5 me-1"></i>Tambah Sekolah
            </a>
        @endcan
    </div>

    <div class="card card-flush border border-gray-200">
        <div class="card-header pt-6 pb-2">
            <form method="GET" class="row g-3 w-100 align-items-end">
                <div class="col-6 col-md-2">
                    <label class="form-label fs-8 text-muted">Jenjang</label>
                    <select name="jenjang" class="form-select form-select-sm" onchange="this.form.submit()">
                        <option value="">Semua</option>
                        @foreach ($jenjangList as $j)
                            <option value="{{ $j }}" {{ request('jenjang') === $j ? 'selected' : '' }}>{{ $j }}</option>
                        @endforeach
                    </select>
                </div>
                <div class="col-6 col-md-2">
                    <label class="form-label fs-8 text-muted">Status</label>
                    <select name="status" class="form-select form-select-sm" onchange="this.form.submit()">
                        <option value="">Semua</option>
                        <option value="negeri" {{ request('status') === 'negeri' ? 'selected' : '' }}>Negeri</option>
                        <option value="swasta" {{ request('status') === 'swasta' ? 'selected' : '' }}>Swasta</option>
                    </select>
                </div>
                <div class="col-12 col-md-4">
                    <label class="form-label fs-8 text-muted">Kabupaten / Kota</label>
                    <select name="region_id" class="form-select form-select-sm" onchange="this.form.submit()">
                        <option value="">Semua wilayah</option>
                        @foreach ($regions as $r)
                            <option value="{{ $r->id }}" {{ request('region_id') === $r->id ? 'selected' : '' }}>{{ $r->name }}</option>
                        @endforeach
                    </select>
                </div>
                <div class="col-6 col-md-2">
                    <a href="{{ route('schools.index') }}" class="btn btn-sm btn-light w-100">Reset</a>
                </div>
            </form>
        </div>

        <div class="card-body pt-4">
            <div class="table-responsive">
                <table class="table table-row-bordered table-row-gray-200 align-middle gy-3" id="tblSchools">
                    <thead>
                        <tr class="fw-bold fs-8 text-uppercase text-muted">
                            <th>Nama Sekolah</th>
                            <th>NPSN</th>
                            <th>Jenjang</th>
                            <th>Wilayah</th>
                            <th class="text-center">Siswa</th>
                            <th class="text-center">Cakupan Wajah</th>
                            <th class="text-center">Tablet</th>
                            <th>Status</th>
                            <th class="text-end">Aksi</th>
                        </tr>
                    </thead>
                </table>
            </div>
        </div>
    </div>
@endsection

@push('scripts')
    <script>
        $(function () {
            $('#tblSchools').DataTable({
                processing: true,
                serverSide: true,
                order: [],
                pageLength: 25,
                ajax: {
                    url: @json(route('schools.data')),
                    data: function (d) {
                        const p = new URLSearchParams(window.location.search);
                        ['jenjang', 'status', 'region_id'].forEach(function (k) {
                            if (p.get(k)) d[k] = p.get(k);
                        });
                    },
                },
                columns: [
                    { data: 'name', name: 'schools.name' },
                    { data: 'npsn', name: 'schools.npsn' },
                    { data: 'jenjang', name: 'schools.jenjang' },
                    { data: 'region_name', name: 'regions.name', defaultContent: '-' },
                    { data: 'student_count', name: 'student_count', className: 'text-center' },
                    { data: 'coverage', name: 'enrolled_count', className: 'text-center' },
                    { data: 'device_count', name: 'device_count', className: 'text-center' },
                    { data: 'status_badge', name: 'schools.is_active' },
                    { data: 'action', name: 'action', orderable: false, searchable: false, className: 'text-end' },
                ],
                language: {
                    processing: 'Memuat...',
                    emptyTable: 'Belum ada sekolah terdaftar.',
                    zeroRecords: 'Tidak ada sekolah yang cocok.',
                    search: 'Cari nama / NPSN:',
                    lengthMenu: 'Tampilkan _MENU_ baris',
                    info: 'Menampilkan _START_&ndash;_END_ dari _TOTAL_ sekolah',
                    paginate: { first: 'Awal', last: 'Akhir', next: 'Berikutnya', previous: 'Sebelumnya' },
                },
            });
        });
    </script>
@endpush
