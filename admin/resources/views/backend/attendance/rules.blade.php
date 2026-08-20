@extends('backend.layout.app')
@section('title', 'Jam Masuk & Pulang')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Jam Masuk &amp; Pulang</h2>
            <span class="text-muted fs-7">Menentukan siswa dihitung hadir, terlambat, atau tanpa keterangan.</span>
        </div>
        @include('backend.partials._school_picker', ['schools' => $schools, 'schoolId' => $schoolId, 'allowAll' => false])
    </div>

    <div class="row g-5">
        <div class="col-xl-7">
            <div class="card card-flush border border-gray-200">
                <div class="card-header pt-5"><h3 class="card-title fw-bold">Aturan Berlaku</h3></div>
                <div class="card-body pt-3 p-0">
                    <div class="table-responsive">
                        <table class="table table-row-bordered align-middle mb-0">
                            <thead class="bg-light">
                                <tr class="fw-bold fs-8 text-uppercase text-muted">
                                    <th class="ps-5">Cakupan</th>
                                    <th>Masuk</th>
                                    <th>Batas Telat</th>
                                    <th>Pulang</th>
                                    <th class="pe-5">Hari</th>
                                </tr>
                            </thead>
                            <tbody>
                                @forelse ($rules as $r)
                                    <tr class="{{ $r->is_active ? '' : 'opacity-50' }}">
                                        <td class="ps-5">
                                            <span class="fw-semibold fs-7">{{ $r->name }}</span>
                                            <span class="text-muted fs-9 d-block">
                                                {{ $r->classroom_id ? ($r->classroom->name ?? 'Kelas') : 'Seluruh sekolah' }}
                                                @unless ($r->is_active) &middot; tidak aktif @endunless
                                            </span>
                                        </td>
                                        <td class="fs-7">{{ substr($r->check_in_opens_at, 0, 5) }}</td>
                                        <td class="fs-7">
                                            <span class="fw-bold">{{ substr($r->check_in_due_at, 0, 5) }}</span>
                                            @if ($r->late_grace_minutes > 0)
                                                <span class="text-muted fs-9 d-block">+{{ $r->late_grace_minutes }} menit toleransi</span>
                                            @endif
                                        </td>
                                        <td class="fs-7">
                                            {{ substr($r->check_out_opens_at, 0, 5) }}&ndash;{{ substr($r->check_out_closes_at, 0, 5) }}
                                        </td>
                                        <td class="pe-5">
                                            @foreach ($r->active_day_names as $d)
                                                <span class="badge badge-light fs-9">{{ substr($d, 0, 3) }}</span>
                                            @endforeach
                                        </td>
                                    </tr>
                                @empty
                                    <tr>
                                        <td colspan="5" class="text-center text-muted py-10 fs-7">
                                            Belum ada aturan khusus. Sistem memakai default:
                                            masuk 06:30, batas 07:15, pulang 12:00&ndash;18:00, Senin&ndash;Jumat.
                                        </td>
                                    </tr>
                                @endforelse
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>

            <div class="alert alert-light-info mt-5 d-flex align-items-start">
                <i class="ki-duotone ki-information-5 fs-2 me-3 mt-1"><span class="path1"></span><span class="path2"></span><span class="path3"></span></i>
                <div class="fs-8">
                    <span class="fw-semibold d-block mb-1">Mengapa aturan lama tidak dihapus?</span>
                    Absensi yang sudah tercatat dinilai memakai aturan yang berlaku saat itu.
                    Menyimpan aturan baru akan menonaktifkan yang lama, bukan menghapusnya,
                    sehingga rekap bulan-bulan sebelumnya tetap bisa dipertanggungjawabkan.
                </div>
            </div>
        </div>

        <div class="col-xl-5">
            @can('manage_attendance_rule')
                <div class="card card-flush border border-gray-200">
                    <div class="card-header pt-5"><h3 class="card-title fw-bold">Atur Jadwal Baru</h3></div>
                    <form method="POST" action="{{ route('attendance-rules.store') }}" class="card-body pt-3">
                        @csrf
                        <input type="hidden" name="school_id" value="{{ $schoolId }}">

                        <div class="mb-4">
                            <label class="form-label">Nama jadwal</label>
                            <input type="text" name="name" class="form-control form-control-sm"
                                   placeholder="Jadwal Reguler" value="{{ old('name') }}" maxlength="80">
                        </div>

                        <div class="mb-4">
                            <label class="form-label">Berlaku untuk</label>
                            <select name="classroom_id" class="form-select form-select-sm">
                                <option value="">Seluruh sekolah</option>
                                @foreach ($classrooms as $c)
                                    <option value="{{ $c->id }}" {{ old('classroom_id') === $c->id ? 'selected' : '' }}>
                                        Hanya {{ $c->name }}
                                    </option>
                                @endforeach
                            </select>
                            <span class="form-text fs-9">Aturan kelas mengalahkan aturan sekolah.</span>
                        </div>

                        <div class="separator my-4"></div>
                        <span class="text-muted fs-8 text-uppercase fw-semibold d-block mb-3">Absen Masuk</span>

                        <div class="row g-3 mb-4">
                            <div class="col-6">
                                <label class="form-label fs-8 required">Gerbang dibuka</label>
                                <input type="time" name="check_in_opens_at" class="form-control form-control-sm"
                                       value="{{ old('check_in_opens_at', '05:30') }}" required>
                            </div>
                            <div class="col-6">
                                <label class="form-label fs-8 required">Mulai dihitung hadir</label>
                                <input type="time" name="check_in_start_at" class="form-control form-control-sm"
                                       value="{{ old('check_in_start_at', '06:30') }}" required>
                            </div>
                            <div class="col-6">
                                <label class="form-label fs-8 required">Batas tepat waktu</label>
                                <input type="time" name="check_in_due_at" class="form-control form-control-sm"
                                       value="{{ old('check_in_due_at', '07:15') }}" required>
                                <span class="form-text fs-9">Lewat ini = terlambat.</span>
                            </div>
                            <div class="col-6">
                                <label class="form-label fs-8 required">Gerbang ditutup</label>
                                <input type="time" name="check_in_closes_at" class="form-control form-control-sm"
                                       value="{{ old('check_in_closes_at', '09:00') }}" required>
                                <span class="form-text fs-9">Lewat ini = tanpa keterangan.</span>
                            </div>
                            <div class="col-6">
                                <label class="form-label fs-8">Toleransi (menit)</label>
                                <input type="number" name="late_grace_minutes" class="form-control form-control-sm"
                                       min="0" max="120" value="{{ old('late_grace_minutes', 0) }}">
                            </div>
                        </div>

                        <div class="separator my-4"></div>
                        <span class="text-muted fs-8 text-uppercase fw-semibold d-block mb-3">Absen Pulang</span>

                        <div class="row g-3 mb-4">
                            <div class="col-6">
                                <label class="form-label fs-8 required">Mulai</label>
                                <input type="time" name="check_out_opens_at" class="form-control form-control-sm"
                                       value="{{ old('check_out_opens_at', '12:00') }}" required>
                            </div>
                            <div class="col-6">
                                <label class="form-label fs-8 required">Selesai</label>
                                <input type="time" name="check_out_closes_at" class="form-control form-control-sm"
                                       value="{{ old('check_out_closes_at', '18:00') }}" required>
                            </div>
                        </div>

                        <label class="form-check form-check-sm form-check-custom mb-4">
                            <input type="checkbox" class="form-check-input" name="require_check_out" value="1"
                                   {{ old('require_check_out', true) ? 'checked' : '' }}>
                            <span class="form-check-label fs-7">Wajib absen pulang</span>
                        </label>

                        <div class="separator my-4"></div>
                        <label class="form-label required">Hari aktif</label>
                        <div class="d-flex flex-wrap gap-3 mb-5">
                            @foreach (['Senin', 'Selasa', 'Rabu', 'Kamis', 'Jumat', 'Sabtu', 'Minggu'] as $bit => $day)
                                <label class="form-check form-check-sm form-check-custom">
                                    <input type="checkbox" class="form-check-input" name="active_days[]"
                                           value="{{ $bit }}" {{ $bit <= 4 ? 'checked' : '' }}>
                                    <span class="form-check-label fs-8">{{ substr($day, 0, 3) }}</span>
                                </label>
                            @endforeach
                        </div>

                        <button class="btn btn-primary w-100" {{ $schoolId ? '' : 'disabled' }}>
                            Simpan &amp; berlakukan
                        </button>
                        @unless ($schoolId)
                            <span class="form-text fs-8 text-center d-block mt-2">Pilih sekolah terlebih dahulu.</span>
                        @endunless
                    </form>
                </div>
            @endcan
        </div>
    </div>
@endsection
