@extends('backend.layout.app')
@section('title', 'Riwayat Pengiriman')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Riwayat Pengiriman Notifikasi</h2>
            <span class="text-muted fs-7">
                90 hari terakhir. Nomor tujuan disamarkan; daftar ini bukan sumber ekspor kontak.
            </span>
        </div>
        @include('backend.partials._school_picker', ['schools' => $schools, 'schoolId' => $schoolId, 'allowAll' => true])
    </div>

    <div class="card card-flush border border-gray-200">
        <div class="card-header pt-6 pb-2">
            <form method="GET" class="row g-3 w-100 align-items-end">
                @if ($schoolId)<input type="hidden" name="school_id" value="{{ $schoolId }}">@endif
                <div class="col-6 col-md-3">
                    <label class="form-label fs-8 text-muted">Status</label>
                    <select name="status" class="form-select form-select-sm" onchange="this.form.submit()">
                        <option value="">Semua</option>
                        @foreach ($statuses as $s)
                            <option value="{{ $s }}" {{ request('status') === $s ? 'selected' : '' }}>{{ ucfirst($s) }}</option>
                        @endforeach
                    </select>
                </div>
                <div class="col-6 col-md-3">
                    <label class="form-label fs-8 text-muted">Kanal</label>
                    <select name="channel" class="form-select form-select-sm" onchange="this.form.submit()">
                        <option value="">Semua</option>
                        @foreach ($channels as $c)
                            <option value="{{ $c }}" {{ request('channel') === $c ? 'selected' : '' }}>{{ ucfirst($c) }}</option>
                        @endforeach
                    </select>
                </div>
                <div class="col-6 col-md-2">
                    <a href="{{ route('notifications.outbox', ['school_id' => $schoolId]) }}" class="btn btn-sm btn-light w-100">Reset</a>
                </div>
            </form>
        </div>

        <div class="card-body pt-4">
            <div class="table-responsive">
                <table class="table table-row-bordered table-row-gray-200 align-middle gy-3" id="tblOutbox">
                    <thead>
                        <tr class="fw-bold fs-8 text-uppercase text-muted">
                            <th>Waktu</th>
                            <th>Siswa</th>
                            <th>Kanal</th>
                            <th>Jenis</th>
                            <th>Tujuan</th>
                            <th>Status</th>
                            <th>Percobaan</th>
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
            $('#tblOutbox').DataTable({
                processing: true,
                serverSide: true,
                order: [],
                pageLength: 25,
                ajax: {
                    url: @json(route('notifications.outbox.data')),
                    data: function (d) {
                        const p = new URLSearchParams(window.location.search);
                        ['school_id', 'status', 'channel'].forEach(function (k) {
                            if (p.get(k)) d[k] = p.get(k);
                        });
                    },
                },
                columns: [
                    { data: 'created_at', name: 'notification_outbox.created_at' },
                    { data: 'student_name', name: 'student_name', defaultContent: '-' },
                    { data: 'channel', name: 'notification_outbox.channel' },
                    { data: 'template_key', name: 'notification_outbox.template_key' },
                    { data: 'recipient', name: 'notification_outbox.recipient' },
                    { data: 'status_badge', name: 'notification_outbox.status' },
                    {
                        data: 'attempts',
                        name: 'notification_outbox.attempts',
                        render: function (data, type, row) {
                            if (type !== 'display') return data;
                            if (!row.last_error) return data;
                            // Pesan galat provider ditampilkan sebagai tooltip
                            // agar baris tetap ringkas namun bisa didiagnosis.
                            const span = document.createElement('span');
                            span.className = 'text-danger';
                            span.title = row.last_error;
                            span.textContent = data + '×';
                            return span.outerHTML;
                        },
                    },
                    { data: 'action', name: 'action', orderable: false, searchable: false, className: 'text-end' },
                ],
                language: {
                    processing: 'Memuat...',
                    emptyTable: 'Belum ada notifikasi terkirim.',
                    zeroRecords: 'Tidak ada data yang cocok.',
                    search: 'Cari nama siswa:',
                    lengthMenu: 'Tampilkan _MENU_ baris',
                    info: 'Menampilkan _START_&ndash;_END_ dari _TOTAL_ pesan',
                    paginate: { first: 'Awal', last: 'Akhir', next: 'Berikutnya', previous: 'Sebelumnya' },
                },
            });
        });
    </script>
@endpush
