@extends('backend.layout.app')
@section('title', 'Akun Siswa Massal')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Pembuatan Akun Siswa Massal</h2>
            <span class="text-muted fs-7">
                Membuat akun Jargon GO untuk siswa aktif yang belum memilikinya, satu kelas sekaligus.
            </span>
        </div>
        <a href="{{ route('app-accounts.index') }}" class="btn btn-sm btn-light">Kembali</a>
    </div>

    {{-- Kredensial hanya ada pada satu tampilan ini. Setelah halaman
         berpindah, kata sandi awal tidak bisa dilihat lagi oleh siapa pun. --}}
    @if (! empty($credentials))
        <div class="card card-flush border border-success mb-5">
            <div class="card-header pt-5">
                <h3 class="card-title fw-bold text-success">
                    {{ count($credentials) }} Kata Sandi Awal
                </h3>
                <div class="card-toolbar">
                    <button class="btn btn-sm btn-light-primary" onclick="window.print()">Cetak</button>
                    <button class="btn btn-sm btn-primary ms-2" id="unduh-csv">Unduh CSV</button>
                </div>
            </div>
            <div class="card-body pt-3">
                <div class="alert alert-warning py-3 px-4 fs-8 mb-4">
                    <span class="fw-bold d-block mb-1">Cetak atau unduh sekarang juga.</span>
                    Kata sandi disimpan dalam bentuk hash dan tidak dapat ditampilkan ulang.
                    Bila halaman ini ditutup, satu-satunya jalan adalah mereset kata sandi per siswa.
                </div>

                <div class="table-responsive">
                    <table class="table table-row-bordered align-middle mb-0" id="tabel-kredensial">
                        <thead class="bg-light">
                            <tr class="fw-bold fs-8 text-uppercase text-muted">
                                <th class="ps-5">Nama</th>
                                <th>Kelas</th>
                                <th>NISN</th>
                                <th class="pe-5">Kata Sandi Awal</th>
                            </tr>
                        </thead>
                        <tbody>
                            @foreach ($credentials as $c)
                                <tr>
                                    <td class="ps-5 fs-8 fw-semibold">{{ $c['full_name'] }}</td>
                                    <td class="fs-8">{{ $c['classroom_name'] ?? '-' }}</td>
                                    <td class="fs-8">{{ $c['nisn'] }}</td>
                                    <td class="pe-5"><code class="fs-7">{{ $c['initial_password'] }}</code></td>
                                </tr>
                            @endforeach
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
    @endif

    @if (! empty($notes))
        <div class="alert alert-light-warning mb-5">
            <span class="fw-bold d-block mb-2">Siswa yang dilewati</span>
            <ul class="mb-0 fs-8">
                @foreach ($notes as $note)
                    <li>{{ $note }}</li>
                @endforeach
            </ul>
        </div>
    @endif

    <div class="row g-5">
        <div class="col-xl-5">
            <div class="card card-flush border border-gray-200">
                <div class="card-header pt-5"><h3 class="card-title fw-bold">Pilih Sasaran</h3></div>
                <form method="GET" class="card-body pt-3">
                    <div class="mb-3">
                        <label class="form-label required">Sekolah</label>
                        <select name="school_id" class="form-select form-select-sm" onchange="this.form.submit()" required>
                            <option value="">&mdash; pilih sekolah &mdash;</option>
                            @foreach ($schools as $s)
                                <option value="{{ $s->id }}" {{ $schoolId === $s->id ? 'selected' : '' }}>
                                    {{ $s->name }}
                                </option>
                            @endforeach
                        </select>
                    </div>
                    <div class="mb-3">
                        <label class="form-label">Kelas</label>
                        <select name="classroom_id" class="form-select form-select-sm" onchange="this.form.submit()">
                            <option value="">Seluruh sekolah</option>
                            @foreach ($classrooms as $c)
                                <option value="{{ $c->id }}" {{ request('classroom_id') === $c->id ? 'selected' : '' }}>
                                    {{ $c->name }}
                                </option>
                            @endforeach
                        </select>
                        <span class="form-text fs-9">
                            Membuat per kelas jauh lebih mudah dibagikan daripada satu sekolah sekaligus.
                        </span>
                    </div>
                </form>

                @if ($schoolId)
                    <div class="card-footer pt-0 pb-5 px-5 border-0">
                        <form method="POST" action="{{ route('app-accounts.bulk.store') }}"
                              onsubmit="return confirm('Buat akun untuk {{ $pending->count() }} siswa?');">
                            @csrf
                            <input type="hidden" name="school_id" value="{{ $schoolId }}">
                            <input type="hidden" name="classroom_id" value="{{ request('classroom_id') }}">
                            <div class="mb-3">
                                <label class="form-label">Batas per sekali proses</label>
                                <input type="number" name="limit" class="form-control form-control-sm"
                                       value="200" min="1" max="1000">
                            </div>
                            <button class="btn btn-primary w-100" {{ $pending->isEmpty() ? 'disabled' : '' }}>
                                Buat {{ $pending->count() }} Akun
                            </button>
                        </form>
                    </div>
                @endif
            </div>
        </div>

        <div class="col-xl-7">
            <div class="card card-flush border border-gray-200">
                <div class="card-header pt-5">
                    <h3 class="card-title fw-bold">Siswa Belum Punya Akun</h3>
                    <div class="card-toolbar">
                        <span class="badge badge-light">{{ $pending->count() }} siswa</span>
                    </div>
                </div>
                <div class="card-body pt-3 p-0">
                    <div class="table-responsive" style="max-height: 520px; overflow-y: auto;">
                        <table class="table table-row-bordered align-middle mb-0">
                            <thead class="bg-light">
                                <tr class="fw-bold fs-8 text-uppercase text-muted">
                                    <th class="ps-5">Nama</th>
                                    <th>Kelas</th>
                                    <th class="pe-5">NISN</th>
                                </tr>
                            </thead>
                            <tbody>
                                @forelse ($pending as $s)
                                    <tr>
                                        <td class="ps-5 fs-8">{{ $s->full_name }}</td>
                                        <td class="fs-8">{{ $s->classroom->name ?? '-' }}</td>
                                        <td class="pe-5 fs-8">
                                            @if ($s->nisn && strlen($s->nisn) === 10)
                                                {{ $s->nisn }}
                                            @else
                                                {{-- Tanpa NISN yang sah, siswa tidak punya identitas
                                                     login dan akan dilewati API. --}}
                                                <span class="badge badge-light-danger fs-9">NISN belum sah</span>
                                            @endif
                                        </td>
                                    </tr>
                                @empty
                                    <tr>
                                        <td colspan="3" class="text-center text-muted py-10 fs-7">
                                            {{ $schoolId
                                                ? 'Semua siswa aktif pada pilihan ini sudah punya akun.'
                                                : 'Pilih sekolah terlebih dahulu.' }}
                                        </td>
                                    </tr>
                                @endforelse
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>
        </div>
    </div>
@endsection

@if (! empty($credentials))
    @push('scripts')
    <script>
    document.getElementById('unduh-csv').addEventListener('click', function () {
        const rows = [['Nama', 'Kelas', 'NISN', 'Kata Sandi Awal']];
        document.querySelectorAll('#tabel-kredensial tbody tr').forEach(function (tr) {
            rows.push(Array.from(tr.children).map(function (td) {
                return '"' + td.textContent.trim().replace(/"/g, '""') + '"';
            }));
        });

        const blob = new Blob(["﻿" + rows.map(function (r) { return r.join(','); }).join('\r\n')],
            { type: 'text/csv;charset=utf-8;' });
        const link = document.createElement('a');
        link.href = URL.createObjectURL(blob);
        link.download = 'akun-siswa-jargon-go.csv';
        link.click();
        URL.revokeObjectURL(link.href);
    });
    </script>
    @endpush
@endif
