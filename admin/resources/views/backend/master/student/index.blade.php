@extends('backend.layout.app')
@section('title', 'Data Siswa')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Data Siswa</h2>
            <span class="text-muted fs-7">Siswa tidak memiliki akun login; identitas operasionalnya adalah wajah terdaftar.</span>
        </div>
        <div class="d-flex flex-wrap align-items-center gap-3">
            @include('backend.partials._school_picker', ['schools' => $schools, 'schoolId' => $schoolId, 'allowAll' => true])
            @can('create_student')
                <a href="{{ route('students.create', ['school_id' => $schoolId]) }}" class="btn btn-sm btn-primary">
                    <i class="ki-outline ki-plus fs-5 me-1"></i>Tambah Siswa
                </a>
            @endcan
        </div>
    </div>

    <div class="card card-flush border border-gray-200">
        <div class="card-header pt-6 pb-2">
            <form method="GET" class="row g-3 w-100 align-items-end">
                @if ($schoolId)<input type="hidden" name="school_id" value="{{ $schoolId }}">@endif
                <div class="col-6 col-md-3">
                    <label class="form-label fs-8 text-muted">Kelas</label>
                    <select name="classroom_id" class="form-select form-select-sm" onchange="this.form.submit()">
                        <option value="">Semua kelas</option>
                        @foreach ($classrooms as $c)
                            <option value="{{ $c->id }}" {{ request('classroom_id') === $c->id ? 'selected' : '' }}>{{ $c->name }}</option>
                        @endforeach
                    </select>
                </div>
                <div class="col-6 col-md-2">
                    <label class="form-label fs-8 text-muted">Status</label>
                    <select name="status" class="form-select form-select-sm" onchange="this.form.submit()">
                        @foreach ($statuses as $s)
                            <option value="{{ $s }}" {{ request('status', 'aktif') === $s ? 'selected' : '' }}>{{ ucfirst($s) }}</option>
                        @endforeach
                        <option value="all" {{ request('status') === 'all' ? 'selected' : '' }}>Semua status</option>
                    </select>
                </div>
                <div class="col-6 col-md-3">
                    <label class="form-label fs-8 text-muted">Data wajah</label>
                    <select name="face" class="form-select form-select-sm" onchange="this.form.submit()">
                        <option value="">Semua</option>
                        <option value="sudah" {{ request('face') === 'sudah' ? 'selected' : '' }}>Sudah terdaftar</option>
                        <option value="belum" {{ request('face') === 'belum' ? 'selected' : '' }}>Belum terdaftar</option>
                    </select>
                </div>
                <div class="col-6 col-md-2">
                    <a href="{{ route('students.index', ['school_id' => $schoolId]) }}" class="btn btn-sm btn-light w-100">Reset</a>
                </div>
            </form>
        </div>

        <div class="card-body pt-4">
            <div class="table-responsive">
                <table class="table table-row-bordered table-row-gray-200 align-middle gy-3" id="tblStudents">
                    <thead>
                        <tr class="fw-bold fs-8 text-uppercase text-muted">
                            <th>Nama</th>
                            <th>NISN / NIS</th>
                            <th>Kelas</th>
                            <th>Jenis Kelamin</th>
                            <th>Status</th>
                            <th>Data Wajah</th>
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
            $('#tblStudents').DataTable({
                processing: true,
                serverSide: true,
                order: [],
                pageLength: 25,
                ajax: {
                    url: @json(route('students.data')),
                    data: function (d) {
                        const p = new URLSearchParams(window.location.search);
                        ['school_id', 'classroom_id', 'status', 'face'].forEach(function (k) {
                            if (p.get(k)) d[k] = p.get(k);
                        });
                    },
                },
                columns: [
                    { data: 'full_name', name: 'full_name' },
                    {
                        data: 'nisn',
                        name: 'nisn',
                        render: function (data, type, row) {
                            if (type !== 'display') return data;
                            return (data || '-') + '<span class="text-muted fs-9 d-block">' + (row.nis || '-') + '</span>';
                        },
                    },
                    { data: 'classroom_name', name: 'classrooms.name', defaultContent: '-' },
                    { data: 'gender_label', name: 'gender' },
                    { data: 'status', name: 'status' },
                    { data: 'biometric', name: 'face_enrolled' },
                    { data: 'action', name: 'action', orderable: false, searchable: false, className: 'text-end' },
                ],
                language: {
                    processing: 'Memuat...',
                    emptyTable: 'Belum ada siswa. Pilih sekolah atau tambahkan siswa baru.',
                    zeroRecords: 'Tidak ada siswa yang cocok.',
                    search: 'Cari nama / NIS / NISN:',
                    lengthMenu: 'Tampilkan _MENU_ baris',
                    info: 'Menampilkan _START_&ndash;_END_ dari _TOTAL_ siswa',
                    paginate: { first: 'Awal', last: 'Akhir', next: 'Berikutnya', previous: 'Sebelumnya' },
                },
            });
        });
    </script>
@endpush
