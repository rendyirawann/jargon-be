@extends('backend.layout.app')
@section('title', 'Data Absensi')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Data Absensi</h2>
            <span class="text-muted fs-7">
                {{ \Illuminate\Support\Carbon::parse($from)->translatedFormat('d M Y') }}
                &ndash; {{ \Illuminate\Support\Carbon::parse($to)->translatedFormat('d M Y') }}
            </span>
        </div>
        @include('backend.partials._school_picker', ['schools' => $schools, 'schoolId' => $schoolId, 'allowAll' => true])
    </div>

    {{-- Ringkasan rentang terpilih --}}
    <div class="row g-3 mb-5">
        @foreach ([
            ['Hadir', 'hadir', 'success'],
            ['Terlambat', 'terlambat', 'warning'],
            ['Izin', 'izin', 'info'],
            ['Sakit', 'sakit', 'primary'],
            ['Alfa', 'alfa', 'danger'],
            ['Dispensasi', 'dispensasi', 'secondary'],
        ] as [$label, $key, $color])
            <div class="col-4 col-md-2">
                <div class="card card-flush border border-gray-200">
                    <div class="card-body p-4 text-center">
                        <span class="text-muted fs-9 text-uppercase d-block">{{ $label }}</span>
                        <span class="fs-2 fw-bold text-{{ $color }}">{{ number_format($summary[$key]) }}</span>
                    </div>
                </div>
            </div>
        @endforeach
    </div>

    <div class="card card-flush border border-gray-200">
        <div class="card-header pt-6 pb-2">
            <form method="GET" class="row g-3 w-100 align-items-end">
                @if ($schoolId)<input type="hidden" name="school_id" value="{{ $schoolId }}">@endif

                <div class="col-6 col-md-2">
                    <label class="form-label fs-8 text-muted">Dari tanggal</label>
                    <input type="date" name="from" class="form-control form-control-sm"
                           value="{{ \Illuminate\Support\Carbon::parse($from)->toDateString() }}"
                           max="{{ now()->toDateString() }}">
                </div>
                <div class="col-6 col-md-2">
                    <label class="form-label fs-8 text-muted">Sampai</label>
                    <input type="date" name="to" class="form-control form-control-sm"
                           value="{{ \Illuminate\Support\Carbon::parse($to)->toDateString() }}"
                           max="{{ now()->toDateString() }}">
                </div>
                <div class="col-6 col-md-3">
                    <label class="form-label fs-8 text-muted">Kelas</label>
                    <select name="classroom_id" class="form-select form-select-sm">
                        <option value="">Semua kelas</option>
                        @foreach ($classrooms as $c)
                            <option value="{{ $c->id }}" {{ request('classroom_id') === $c->id ? 'selected' : '' }}>
                                {{ $c->name }}
                            </option>
                        @endforeach
                    </select>
                </div>
                <div class="col-6 col-md-2">
                    <label class="form-label fs-8 text-muted">Status</label>
                    <select name="status" class="form-select form-select-sm">
                        <option value="">Semua</option>
                        @foreach ($statuses as $s)
                            <option value="{{ $s }}" {{ request('status') === $s ? 'selected' : '' }}>
                                {{ ucfirst($s) }}
                            </option>
                        @endforeach
                    </select>
                </div>
                <div class="col-12 col-md-3 d-flex gap-2">
                    <button class="btn btn-sm btn-primary flex-grow-1">Terapkan</button>
                    <a href="{{ route('attendances.index') }}" class="btn btn-sm btn-light">Reset</a>
                </div>

                <div class="col-12">
                    <label class="form-check form-check-sm form-check-custom">
                        <input type="checkbox" class="form-check-input" name="missing_check_out" value="1"
                               {{ request()->boolean('missing_check_out') ? 'checked' : '' }}>
                        <span class="form-check-label fs-8 text-muted">
                            Hanya yang belum absen pulang
                        </span>
                    </label>
                </div>
            </form>
        </div>

        <div class="card-body pt-4">
            <div class="table-responsive">
                <table class="table table-row-bordered table-row-gray-200 align-middle gy-3" id="tblAttendance">
                    <thead>
                        <tr class="fw-bold fs-8 text-uppercase text-muted">
                            <th>Tanggal</th>
                            <th>Siswa</th>
                            <th>Kelas</th>
                            <th>Masuk</th>
                            <th>Pulang</th>
                            <th>Status</th>
                            <th>Metode</th>
                            <th class="text-end">Aksi</th>
                        </tr>
                    </thead>
                </table>
            </div>
        </div>
    </div>

    {{-- Modal koreksi absensi --}}
    @can('override_attendance')
        <div class="modal fade" id="modalManual" tabindex="-1" aria-hidden="true">
            <div class="modal-dialog modal-dialog-centered">
                <form method="POST" action="{{ route('attendances.manual') }}" class="modal-content">
                    @csrf
                    <input type="hidden" name="student_id" id="mStudentId">
                    <input type="hidden" name="attendance_date" id="mDate">

                    <div class="modal-header">
                        <h4 class="modal-title">Koreksi Absensi</h4>
                        <button type="button" class="btn btn-icon btn-sm btn-active-light-primary"
                                data-bs-dismiss="modal">
                            <i class="ki-outline ki-cross fs-2"></i>
                        </button>
                    </div>

                    <div class="modal-body">
                        <div class="alert alert-light-warning d-flex align-items-center py-3 px-4 mb-5">
                            <i class="ki-duotone ki-information-5 fs-2 me-3"><span class="path1"></span><span class="path2"></span><span class="path3"></span></i>
                            <span class="fs-8">
                                Setiap koreksi dicatat dalam jejak audit beserta nama Anda dan alasannya.
                            </span>
                        </div>

                        <div class="mb-4">
                            <span class="text-muted fs-8 d-block">Siswa</span>
                            <span class="fw-bold fs-6" id="mStudentName">-</span>
                        </div>

                        <div class="mb-4">
                            <label class="form-label required">Status</label>
                            <select name="status" class="form-select" id="mStatus" required>
                                <option value="hadir">Hadir</option>
                                <option value="terlambat">Terlambat</option>
                                <option value="izin">Izin</option>
                                <option value="sakit">Sakit</option>
                                <option value="alfa">Tanpa Keterangan</option>
                                <option value="dispensasi">Dispensasi</option>
                            </select>
                        </div>

                        <div class="row g-3 mb-4">
                            <div class="col-6">
                                <label class="form-label">Jam masuk</label>
                                <input type="time" name="check_in_time" class="form-control" id="mCheckIn">
                                <span class="form-text fs-9">Wajib untuk hadir/terlambat/dispensasi.</span>
                            </div>
                            <div class="col-6">
                                <label class="form-label">Jam pulang</label>
                                <input type="time" name="check_out_time" class="form-control">
                            </div>
                        </div>

                        <div class="mb-4">
                            <label class="form-label required">Alasan koreksi</label>
                            <textarea name="notes" class="form-control" rows="2" minlength="3" maxlength="300"
                                      placeholder="mis. Surat izin dari orang tua, atau lupa absen" required></textarea>
                        </div>

                        <label class="form-check form-check-custom">
                            <input type="checkbox" class="form-check-input" name="notify_guardian" value="1">
                            <span class="form-check-label fs-7">Kirim notifikasi ke wali murid</span>
                        </label>
                    </div>

                    <div class="modal-footer">
                        <button type="button" class="btn btn-light" data-bs-dismiss="modal">Batal</button>
                        <button class="btn btn-primary">Simpan koreksi</button>
                    </div>
                </form>
            </div>
        </div>
    @endcan
@endsection

@push('scripts')
    <script>
        $(function () {
            $('#tblAttendance').DataTable({
                processing: true,
                serverSide: true,
                // Data absensi berukuran besar; pengurutan default mengikuti
                // tanggal terbaru dan diserahkan ke server.
                order: [],
                pageLength: 25,
                lengthMenu: [[25, 50, 100], [25, 50, 100]],
                ajax: {
                    url: @json(route('attendances.data')),
                    data: function (d) {
                        const params = new URLSearchParams(window.location.search);
                        ['school_id', 'from', 'to', 'classroom_id', 'status', 'missing_check_out']
                            .forEach(function (k) {
                                if (params.get(k)) d[k] = params.get(k);
                            });
                    },
                },
                columns: [
                    { data: 'attendance_date', name: 'attendance_date' },
                    {
                        data: 'student_name',
                        name: 'student_name',
                        render: function (data, type, row) {
                            if (type !== 'display') return data;
                            const wrap = document.createElement('div');
                            const n = document.createElement('span');
                            n.className = 'fw-semibold text-gray-800';
                            n.textContent = data || '-';
                            const nis = document.createElement('span');
                            nis.className = 'text-muted fs-8 d-block';
                            nis.textContent = row.student_nis || '-';
                            wrap.append(n, nis);
                            return wrap.innerHTML;
                        },
                    },
                    { data: 'classroom_name', name: 'classroom_name', defaultContent: '-' },
                    { data: 'check_in_label', name: 'check_in_at' },
                    { data: 'check_out_label', name: 'check_out_at' },
                    { data: 'status_badge', name: 'status' },
                    { data: 'method_label', name: 'check_in_method', orderable: false },
                    { data: 'action', name: 'action', orderable: false, searchable: false, className: 'text-end' },
                ],
                language: {
                    processing: 'Memuat...',
                    emptyTable: 'Tidak ada data absensi pada rentang ini.',
                    zeroRecords: 'Tidak ada data yang cocok.',
                    search: 'Cari:',
                    lengthMenu: 'Tampilkan _MENU_ baris',
                    info: 'Menampilkan _START_&ndash;_END_ dari _TOTAL_ baris',
                    infoEmpty: 'Tidak ada baris',
                    paginate: { first: 'Awal', last: 'Akhir', next: 'Berikutnya', previous: 'Sebelumnya' },
                },
            });

            // Modal koreksi diisi dari atribut data pada tombol baris.
            document.addEventListener('click', function (e) {
                const btn = e.target.closest('[data-correct-attendance]');
                if (!btn) return;

                document.getElementById('mStudentId').value = btn.dataset.studentId;
                document.getElementById('mDate').value = btn.dataset.date;
                document.getElementById('mStudentName').textContent = btn.dataset.studentName;
                document.getElementById('mStatus').value = btn.dataset.status || 'hadir';
                document.getElementById('mCheckIn').value = btn.dataset.checkIn || '';
            });
        });
    </script>
@endpush
