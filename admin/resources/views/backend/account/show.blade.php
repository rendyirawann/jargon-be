@extends('backend.layout.app')
@section('title', 'Detail Akun')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">{{ $account->name }}</h2>
            <span class="text-muted fs-7">
                {{ $account->role_label }} &middot;
                {{ $account->identity_label }} {{ $account->identity_number ?? '-' }}
            </span>
        </div>
        <a href="{{ route('app-accounts.index') }}" class="btn btn-sm btn-light">Kembali</a>
    </div>

    <div class="row g-5">
        <div class="col-xl-8">
            @if ($isParent)
                <div class="card card-flush border border-gray-200 mb-5">
                    <div class="card-header pt-5">
                        <h3 class="card-title fw-bold">Anak yang Dipantau</h3>
                        <div class="card-toolbar">
                            <span class="badge badge-light">{{ $account->children->count() }} anak</span>
                        </div>
                    </div>
                    <div class="card-body pt-3">
                        <div class="alert alert-light-warning py-3 px-4 fs-9 mb-4">
                            Daftar ini menentukan data siapa saja yang boleh dibaca akun ini.
                            Salah menautkan berarti seseorang dapat membaca absensi anak orang lain.
                        </div>

                        @forelse ($account->children as $child)
                            <div class="d-flex align-items-center justify-content-between border-bottom border-gray-200 py-3">
                                <div>
                                    <span class="fw-semibold fs-7 d-block">{{ $child->full_name }}</span>
                                    <span class="text-muted fs-9">
                                        {{ $child->classroom->name ?? 'tanpa kelas' }} &middot;
                                        {{ $child->school->name ?? '-' }} &middot;
                                        sebagai {{ $child->pivot->relation }}
                                    </span>
                                </div>
                                <form method="POST"
                                      action="{{ route('app-accounts.children.unlink', [$account->id, $child->id]) }}"
                                      onsubmit="return confirm('Putuskan tautan {{ $child->full_name }}?\n\nSesi login akun ini akan diakhiri.');">
                                    @csrf
                                    @method('DELETE')
                                    <button class="btn btn-sm btn-light-danger">Putuskan</button>
                                </form>
                            </div>
                        @empty
                            <span class="text-muted fs-7">
                                Belum ada anak yang ditautkan &mdash; akun ini belum bisa melihat data apa pun.
                            </span>
                        @endforelse

                        <form method="POST" action="{{ route('app-accounts.children.link', $account->id) }}"
                              class="mt-5 pt-4 border-top border-gray-200">
                            @csrf
                            <label class="form-label">Tambah anak</label>
                            <div class="row g-2">
                                <div class="col-md-7">
                                    <input type="search" id="cari-siswa" class="form-control form-control-sm"
                                           placeholder="cari nama atau NISN (minimal 3 huruf)" autocomplete="off">
                                    <input type="hidden" name="student_id" id="student_id">
                                    <div id="hasil-siswa" class="list-group mt-2"></div>
                                    <span class="form-text fs-9" id="terpilih-label"></span>
                                </div>
                                <div class="col-md-3">
                                    <select name="relation" class="form-select form-select-sm">
                                        @foreach ($relations as $key => $label)
                                            <option value="{{ $key }}">{{ $label }}</option>
                                        @endforeach
                                    </select>
                                </div>
                                <div class="col-md-2">
                                    <button class="btn btn-sm btn-primary w-100">Tautkan</button>
                                </div>
                            </div>
                        </form>
                    </div>
                </div>
            @endif

            @if ($account->student)
                <div class="card card-flush border border-gray-200">
                    <div class="card-header pt-5"><h3 class="card-title fw-bold">Data Siswa</h3></div>
                    <div class="card-body pt-3">
                        <div class="row g-3 fs-8">
                            <div class="col-md-6">
                                <span class="text-muted d-block">Nama</span>
                                <span class="fw-semibold">{{ $account->student->full_name }}</span>
                            </div>
                            <div class="col-md-3">
                                <span class="text-muted d-block">NISN</span>
                                <span class="fw-semibold">{{ $account->student->nisn ?? '-' }}</span>
                            </div>
                            <div class="col-md-3">
                                <span class="text-muted d-block">Kelas</span>
                                <span class="fw-semibold">{{ $account->student->classroom->name ?? '-' }}</span>
                            </div>
                            <div class="col-md-6">
                                <span class="text-muted d-block">Status Wajah</span>
                                <span class="badge {{ $account->student->biometric_badge }}">
                                    {{ $account->student->biometric_status }}
                                </span>
                            </div>
                        </div>
                        <div class="alert alert-light-info py-3 px-4 fs-9 mt-4 mb-0">
                            Akun ini hanya untuk MEMBACA data absensi. Kehadiran tetap dicatat
                            lewat pengenalan wajah di tablet &mdash; siswa tidak dapat mengabsenkan
                            dirinya dari aplikasi.
                        </div>
                    </div>
                </div>
            @endif
        </div>

        <div class="col-xl-4">
            <div class="card card-flush border border-gray-200">
                <div class="card-body p-5 fs-8">
                    <div class="mb-3">
                        <span class="text-muted d-block">Status</span>
                        @if (! $account->is_active)
                            <span class="badge badge-light-danger">nonaktif</span>
                        @elseif ($account->must_change_password)
                            <span class="badge badge-light-warning">belum ganti kata sandi</span>
                        @else
                            <span class="badge badge-light-success">aktif</span>
                        @endif
                    </div>
                    <div class="mb-3">
                        <span class="text-muted d-block">Cakupan</span>
                        <span class="fw-semibold">
                            {{ $account->school->name
                                ?? ($isParent ? 'mengikuti sekolah anaknya' : 'Provinsi Sumatera Utara') }}
                        </span>
                    </div>
                    <div class="mb-3">
                        <span class="text-muted d-block">Email</span>
                        <span class="fw-semibold">{{ $account->email }}</span>
                    </div>
                    @if ($account->phone)
                        <div class="mb-3">
                            <span class="text-muted d-block">Nomor HP</span>
                            <span class="fw-semibold">{{ $account->phone }}</span>
                        </div>
                    @endif
                    <div class="mb-3">
                        <span class="text-muted d-block">Login Terakhir</span>
                        <span class="fw-semibold">
                            {{ $account->last_login
                                ? $account->last_login->timezone(config('app.timezone'))->diffForHumans()
                                : 'belum pernah' }}
                        </span>
                    </div>
                    <div>
                        <span class="text-muted d-block">Dibuat</span>
                        <span class="fw-semibold">
                            {{ $account->created_at->timezone(config('app.timezone'))->format('d/m/Y H:i') }}
                        </span>
                    </div>
                </div>
            </div>
        </div>
    </div>
@endsection

@if ($isParent)
    @push('scripts')
    <script>
    (function () {
        const cari = document.getElementById('cari-siswa');
        const hasil = document.getElementById('hasil-siswa');
        const hidden = document.getElementById('student_id');
        const label = document.getElementById('terpilih-label');
        let timer = null;

        cari.addEventListener('input', function () {
            clearTimeout(timer);
            hidden.value = '';
            label.textContent = '';
            const term = cari.value.trim();
            if (term.length < 3) { hasil.innerHTML = ''; return; }

            timer = setTimeout(function () {
                fetch('{{ route('app-accounts.students') }}?q=' + encodeURIComponent(term))
                    .then(function (r) { return r.json(); })
                    .then(function (json) {
                        hasil.innerHTML = '';
                        (json.data || []).forEach(function (s) {
                            const item = document.createElement('button');
                            item.type = 'button';
                            item.className = 'list-group-item list-group-item-action fs-8';
                            item.textContent = s.name + ' — ' + (s.nisn || 'tanpa NISN')
                                + ' · ' + s.classroom + ' · ' + s.school;
                            item.onclick = function () {
                                hidden.value = s.id;
                                label.textContent = 'Dipilih: ' + s.name;
                                hasil.innerHTML = '';
                                cari.value = s.name;
                            };
                            hasil.appendChild(item);
                        });
                    });
            }, 300);
        });
    })();
    </script>
    @endpush
@endif
