@extends('backend.layout.app')
@section('title', 'Perangkat Tablet')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Perangkat Tablet</h2>
            <span class="text-muted fs-7">
                {{ $stats['online'] }} dari {{ $stats['total'] }} perangkat online
                @if ($stats['unpaired'] > 0)
                    &middot; <span class="text-info">{{ $stats['unpaired'] }} belum dipasangkan</span>
                @endif
            </span>
        </div>
        <div class="d-flex flex-wrap align-items-center gap-3">
            @include('backend.partials._school_picker', ['schools' => $schools, 'schoolId' => $schoolId, 'allowAll' => true])
            @can('create_device')
                <button class="btn btn-sm btn-primary" data-bs-toggle="modal" data-bs-target="#modalDevice">
                    <i class="ki-outline ki-plus fs-5 me-1"></i>Tambah Perangkat
                </button>
            @endcan
        </div>
    </div>

    <div class="card card-flush border border-gray-200">
        <div class="card-body p-0">
            <div class="table-responsive">
                <table class="table table-row-bordered align-middle mb-0">
                    <thead class="bg-light">
                        <tr class="fw-bold fs-8 text-uppercase text-muted">
                            <th class="ps-5">Kode / Nama</th>
                            <th>Sekolah</th>
                            <th>Penempatan</th>
                            <th>Mode</th>
                            <th>Terakhir Aktif</th>
                            <th>Status</th>
                            <th class="text-end pe-5">Aksi</th>
                        </tr>
                    </thead>
                    <tbody>
                        @forelse ($devices as $d)
                            <tr>
                                <td class="ps-5">
                                    <a href="{{ route('devices.show', $d) }}" class="fw-semibold fs-7 text-gray-800 text-hover-primary">
                                        {{ $d->code }}
                                    </a>
                                    <span class="text-muted fs-9 d-block">{{ $d->name }}</span>
                                </td>
                                <td class="fs-8">{{ $d->school->name ?? '-' }}</td>
                                <td class="fs-8">
                                    {{ $d->placement_label }}
                                    @if ($d->classroom)
                                        <span class="text-muted fs-9 d-block">{{ $d->classroom->name }}</span>
                                    @endif
                                </td>
                                <td class="fs-8">{{ $d->mode_label }}</td>
                                <td class="fs-8">
                                    {{ $d->last_seen_at
                                        ? $d->last_seen_at->timezone(config('app.timezone'))->diffForHumans()
                                        : 'belum pernah' }}
                                </td>
                                <td><span class="badge {{ $d->status_badge }}">{{ $d->status_label }}</span></td>
                                <td class="text-end pe-5">
                                    <div class="d-flex justify-content-end gap-1">
                                        @can('pair_device')
                                            <form method="POST" action="{{ route('devices.pairing-code', $d) }}">
                                                @csrf
                                                <button class="btn btn-sm btn-light-primary py-1 px-3 fs-9"
                                                        title="Buat kode pairing baru">Pairing</button>
                                            </form>
                                        @endcan
                                        @can('update_device')
                                            <button class="btn btn-icon btn-sm btn-light-warning"
                                                    data-bs-toggle="modal" data-bs-target="#modalEdit{{ $loop->index }}"
                                                    title="Ubah">
                                                <i class="ki-outline ki-pencil fs-5"></i>
                                            </button>
                                            @if ($d->is_paired)
                                                <form method="POST" action="{{ route('devices.revoke', $d) }}"
                                                      onsubmit="return confirm('Cabut token {{ $d->code }}? Tablet harus dipasangkan ulang.');">
                                                    @csrf
                                                    <button class="btn btn-icon btn-sm btn-light-danger" title="Cabut token">
                                                        <i class="ki-outline ki-lock fs-5"></i>
                                                    </button>
                                                </form>
                                            @endif
                                        @endcan
                                    </div>
                                </td>
                            </tr>

                            {{-- Modal ubah per perangkat --}}
                            @can('update_device')
                                <div class="modal fade" id="modalEdit{{ $loop->index }}" tabindex="-1" aria-hidden="true">
                                    <div class="modal-dialog modal-dialog-centered">
                                        <form method="POST" action="{{ route('devices.update', $d) }}" class="modal-content">
                                            @csrf @method('PUT')
                                            <div class="modal-header">
                                                <h4 class="modal-title">Ubah {{ $d->code }}</h4>
                                                <button type="button" class="btn btn-icon btn-sm" data-bs-dismiss="modal">
                                                    <i class="ki-outline ki-cross fs-2"></i>
                                                </button>
                                            </div>
                                            <div class="modal-body">
                                                <div class="mb-3">
                                                    <label class="form-label required">Nama</label>
                                                    <input type="text" name="name" class="form-control" required
                                                           value="{{ $d->name }}" maxlength="120">
                                                </div>
                                                <div class="mb-3">
                                                    <label class="form-label required">Penempatan</label>
                                                    <select name="placement" class="form-select" required>
                                                        @foreach ($placements as $p)
                                                            <option value="{{ $p }}" {{ $d->placement === $p ? 'selected' : '' }}>
                                                                {{ match ($p) {
                                                                    'gate' => 'Gerbang',
                                                                    'classroom' => 'Ruang Kelas',
                                                                    'office' => 'Kantor',
                                                                    default => 'Mobile',
                                                                } }}
                                                            </option>
                                                        @endforeach
                                                    </select>
                                                </div>
                                                <div class="mb-3">
                                                    <label class="form-label">Kelas (untuk tablet di kelas)</label>
                                                    <select name="classroom_id" class="form-select">
                                                        <option value="">-</option>
                                                        @foreach ($classrooms as $c)
                                                            <option value="{{ $c->id }}" {{ $d->classroom_id === $c->id ? 'selected' : '' }}>
                                                                {{ $c->name }}
                                                            </option>
                                                        @endforeach
                                                    </select>
                                                </div>
                                                <div class="mb-3">
                                                    <label class="form-label required">Mode</label>
                                                    <select name="mode" class="form-select" required>
                                                        @foreach ($modes as $m)
                                                            <option value="{{ $m }}" {{ $d->mode === $m ? 'selected' : '' }}>
                                                                {{ match ($m) {
                                                                    'auto' => 'Otomatis (masuk & pulang)',
                                                                    'check_in' => 'Hanya absen masuk',
                                                                    'check_out' => 'Hanya absen pulang',
                                                                    default => 'Pendaftaran wajah',
                                                                } }}
                                                            </option>
                                                        @endforeach
                                                    </select>
                                                </div>
                                                <label class="form-check form-check-custom">
                                                    <input type="checkbox" class="form-check-input" name="is_active" value="1"
                                                           {{ $d->is_active ? 'checked' : '' }}>
                                                    <span class="form-check-label fs-7">Perangkat aktif</span>
                                                </label>
                                            </div>
                                            <div class="modal-footer">
                                                <button type="button" class="btn btn-light" data-bs-dismiss="modal">Batal</button>
                                                <button class="btn btn-primary">Simpan</button>
                                            </div>
                                        </form>
                                    </div>
                                </div>
                            @endcan
                        @empty
                            <tr>
                                <td colspan="7" class="text-center text-muted py-10 fs-7">
                                    Belum ada perangkat terdaftar. Tanpa tablet, absensi wajah tidak dapat berjalan.
                                </td>
                            </tr>
                        @endforelse
                    </tbody>
                </table>
            </div>
        </div>
    </div>

    {{-- Modal tambah perangkat --}}
    @can('create_device')
        <div class="modal fade" id="modalDevice" tabindex="-1" aria-hidden="true">
            <div class="modal-dialog modal-dialog-centered">
                <form method="POST" action="{{ route('devices.store') }}" class="modal-content">
                    @csrf
                    <div class="modal-header">
                        <h4 class="modal-title">Tambah Perangkat</h4>
                        <button type="button" class="btn btn-icon btn-sm" data-bs-dismiss="modal">
                            <i class="ki-outline ki-cross fs-2"></i>
                        </button>
                    </div>
                    <div class="modal-body">
                        <div class="alert alert-light-primary py-3 px-4 fs-8 mb-4">
                            Setelah dibuat, Anda akan menerima <strong>kode pairing 8 digit</strong>
                            yang berlaku 30 menit. Masukkan kode itu di aplikasi tablet — token
                            permanen dibuat di sana dan tidak pernah ditampilkan lagi.
                        </div>

                        <div class="mb-3">
                            <label class="form-label required">Sekolah</label>
                            <select name="school_id" class="form-select" required>
                                <option value="">Pilih sekolah</option>
                                @foreach ($schools->isEmpty() ? \App\Support\Tenant::selectableSchools() : $schools as $s)
                                    <option value="{{ $s->id }}" {{ $schoolId === $s->id ? 'selected' : '' }}>{{ $s->name }}</option>
                                @endforeach
                            </select>
                        </div>
                        <div class="row g-3 mb-3">
                            <div class="col-6">
                                <label class="form-label required">Kode perangkat</label>
                                <input type="text" name="code" class="form-control text-uppercase" required
                                       maxlength="40" placeholder="MDN-SMAN1-GATE-01" value="{{ old('code') }}">
                            </div>
                            <div class="col-6">
                                <label class="form-label required">Nama</label>
                                <input type="text" name="name" class="form-control" required maxlength="120"
                                       placeholder="Tablet Gerbang Utama" value="{{ old('name') }}">
                            </div>
                        </div>
                        <div class="row g-3 mb-3">
                            <div class="col-6">
                                <label class="form-label required">Penempatan</label>
                                <select name="placement" class="form-select" required>
                                    <option value="gate">Gerbang</option>
                                    <option value="classroom">Ruang Kelas</option>
                                    <option value="office">Kantor</option>
                                    <option value="mobile">Mobile</option>
                                </select>
                            </div>
                            <div class="col-6">
                                <label class="form-label required">Mode</label>
                                <select name="mode" class="form-select" required>
                                    <option value="auto">Otomatis (masuk &amp; pulang)</option>
                                    <option value="check_in">Hanya absen masuk</option>
                                    <option value="check_out">Hanya absen pulang</option>
                                    <option value="enroll">Pendaftaran wajah</option>
                                </select>
                            </div>
                        </div>
                        <div class="mb-0">
                            <label class="form-label">Kelas (wajib bila penempatan = ruang kelas)</label>
                            <select name="classroom_id" class="form-select">
                                <option value="">-</option>
                                @foreach ($classrooms as $c)
                                    <option value="{{ $c->id }}">{{ $c->name }}</option>
                                @endforeach
                            </select>
                        </div>
                    </div>
                    <div class="modal-footer">
                        <button type="button" class="btn btn-light" data-bs-dismiss="modal">Batal</button>
                        <button class="btn btn-primary">Buat &amp; ambil kode pairing</button>
                    </div>
                </form>
            </div>
        </div>
    @endcan
@endsection
