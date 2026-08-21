@extends('backend.layout.app')
@section('title', $student->full_name)

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">{{ $student->full_name }}</h2>
            <span class="text-muted fs-7">
                {{ $student->classroom?->name ?? 'Belum ditempatkan' }} &middot;
                {{ $student->school->name }} &middot;
                NIS {{ $student->nis ?? '-' }} / NISN {{ $student->nisn ?? '-' }}
            </span>
        </div>
        <div class="d-flex gap-2">
            @can('create_face_enrollment')
                <a href="{{ route('biometric.capture', $student) }}" class="btn btn-sm btn-light-primary">
                    <i class="ki-outline ki-scan-barcode fs-5 me-1"></i>Kelola Data Wajah
                </a>
            @endcan
            @can('update_student')
                <a href="{{ route('students.edit', $student) }}" class="btn btn-sm btn-light-warning">Ubah Data</a>
            @endcan
            <a href="{{ route('students.index') }}" class="btn btn-sm btn-light">Kembali</a>
        </div>
    </div>

    <div class="row g-5">
        {{-- Rekap 30 hari --}}
        <div class="col-xl-8">
            <div class="card card-flush border border-gray-200 mb-5">
                <div class="card-header pt-5">
                    <h3 class="card-title fw-bold">Rekap 30 Hari Terakhir</h3>
                    <span class="text-muted fs-8">
                        {{ $from->translatedFormat('d M') }} &ndash; {{ $to->translatedFormat('d M Y') }}
                    </span>
                </div>
                <div class="card-body pt-3">
                    <div class="row g-3 text-center">
                        @foreach ([
                            ['Hadir', $recap['hadir'], 'success'],
                            ['Terlambat', $recap['terlambat'], 'warning'],
                            ['Izin', $recap['izin'], 'info'],
                            ['Sakit', $recap['sakit'], 'primary'],
                            ['Alfa', $recap['alfa'], 'danger'],
                        ] as [$label, $value, $color])
                            <div class="col">
                                <div class="border border-gray-200 rounded p-3">
                                    <span class="fs-2 fw-bold text-{{ $color }} d-block">{{ $value }}</span>
                                    <span class="text-muted fs-9 text-uppercase">{{ $label }}</span>
                                </div>
                            </div>
                        @endforeach
                    </div>
                    @if ($recap['total_late'] > 0)
                        <div class="alert alert-light-warning mt-4 mb-0 py-3 px-4 fs-8">
                            Total keterlambatan {{ $recap['total_late'] }} menit dalam 30 hari terakhir.
                        </div>
                    @endif
                </div>
            </div>

            <div class="card card-flush border border-gray-200">
                <div class="card-header pt-5"><h3 class="card-title fw-bold">Riwayat Absensi</h3></div>
                <div class="card-body pt-3 p-0">
                    <div class="table-responsive" style="max-height: 480px; overflow-y: auto;">
                        <table class="table table-row-bordered align-middle mb-0">
                            <thead class="bg-light sticky-top">
                                <tr class="fw-bold fs-8 text-uppercase text-muted">
                                    <th class="ps-5">Tanggal</th>
                                    <th>Masuk</th>
                                    <th>Pulang</th>
                                    <th>Status</th>
                                    <th>Catatan</th>
                                    @can('delete_attendance')
                                        <th class="pe-5 text-end">Hapus</th>
                                    @endcan
                                </tr>
                            </thead>
                            <tbody>
                                @forelse ($attendances as $a)
                                    <tr>
                                        <td class="ps-5 fs-7">{{ $a->attendance_date->translatedFormat('D, d M Y') }}</td>
                                        <td class="fs-7">{{ $a->check_in_time }}</td>
                                        <td class="fs-7">{{ $a->check_out_time }}</td>
                                        <td>
                                            <span class="badge {{ $a->status_badge }}">{{ $a->status_label }}</span>
                                            @if ($a->late_minutes > 0)
                                                <span class="text-muted fs-9 d-block">+{{ $a->late_minutes }} menit</span>
                                            @endif
                                        </td>
                                        <td class="fs-8 text-muted">{{ $a->notes ?? '-' }}</td>
                                        @can('delete_attendance')
                                            <td class="pe-5 text-end">
                                                <button type="button"
                                                        class="btn btn-icon btn-sm btn-light-danger"
                                                        data-hapus-absensi
                                                        data-id="{{ $a->id }}"
                                                        data-tanggal="{{ $a->attendance_date->toDateString() }}"
                                                        data-nama="{{ $student->full_name }}"
                                                        data-label="{{ $a->attendance_date->translatedFormat('d M Y') }}"
                                                        title="Hapus absensi ini">
                                                    <i class="ki-outline ki-trash fs-5"></i>
                                                </button>
                                            </td>
                                        @endcan
                                    </tr>
                                @empty
                                    <tr><td colspan="{{ auth()->user()->can('delete_attendance') ? 6 : 5 }}" class="text-center text-muted py-10 fs-7">Belum ada riwayat absensi.</td></tr>
                                @endforelse
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>
        </div>

        <div class="col-xl-4">
            {{-- Status biometrik --}}
            <div class="card card-flush border border-gray-200 mb-5">
                <div class="card-header pt-5"><h3 class="card-title fw-bold">Data Wajah</h3></div>
                <div class="card-body pt-3">
                    <div class="d-flex align-items-center justify-content-between mb-4">
                        <span class="badge {{ $student->biometric_badge }} fs-7 py-2 px-3">
                            {{ match ($student->biometric_status) {
                                'lengkap' => 'Lengkap',
                                'kurang' => 'Sampel kurang',
                                default => 'Belum terdaftar',
                            } }}
                        </span>
                        <span class="text-muted fs-8">
                            {{ $student->face_sample_count }} / {{ \App\Models\Student::RECOMMENDED_SAMPLES }} sampel
                        </span>
                    </div>

                    @if ($samples->isNotEmpty())
                        <div class="row g-2">
                            @foreach ($samples as $s)
                                <div class="col-4">
                                    <div class="position-relative">
                                        {{-- Foto disajikan lewat API yang memverifikasi hak akses,
                                             bukan langsung dari web server. --}}
                                        <img src="{{ $s->image_url }}" class="rounded w-100" alt="Sampel wajah"
                                             style="aspect-ratio: 1; object-fit: cover;" loading="lazy">
                                        <span class="badge {{ $s->quality_badge }} position-absolute bottom-0 start-0 m-1 fs-9">
                                            {{ $s->quality_score !== null ? number_format($s->quality_score, 2) : '?' }}
                                        </span>
                                    </div>
                                    <span class="text-muted fs-9 d-block text-center mt-1">{{ $s->pose_label }}</span>
                                </div>
                            @endforeach
                        </div>
                    @else
                        <div class="text-center text-muted py-6 fs-8">
                            Siswa ini belum bisa absen dengan wajah.
                        </div>
                    @endif

                    @can('create_face_enrollment')
                        <a href="{{ route('biometric.capture', $student) }}" class="btn btn-sm btn-light-primary w-100 mt-4">
                            {{ $samples->isEmpty() ? 'Daftarkan wajah' : 'Tambah sampel' }}
                        </a>
                    @endcan
                </div>
            </div>

            {{-- Wali murid --}}
            <div class="card card-flush border border-gray-200">
                <div class="card-header pt-5"><h3 class="card-title fw-bold">Wali Murid</h3></div>
                <div class="card-body pt-3">
                    @forelse ($student->guardians as $g)
                        <div class="border border-gray-200 rounded p-3 mb-3">
                            <div class="d-flex align-items-start justify-content-between mb-2">
                                <div>
                                    <span class="fw-semibold fs-7 d-block">{{ $g->full_name }}</span>
                                    <span class="text-muted fs-9">{{ ucfirst($g->relation) }}</span>
                                </div>
                                <div class="text-end">
                                    @if ($g->is_primary)
                                        <span class="badge badge-light-primary fs-9">Utama</span>
                                    @endif
                                    @unless ($g->is_reachable)
                                        <span class="badge badge-light-danger fs-9 d-block mt-1">Kontak belum lengkap</span>
                                    @endunless
                                </div>
                            </div>
                            <div class="fs-8 text-muted">
                                Kanal: <span class="fw-semibold">{{ ucfirst($g->preferred_channel) }}</span>
                                @if ($g->whatsapp) &middot; WA {{ $g->whatsapp }} @endif
                                @if ($g->email) &middot; {{ $g->email }} @endif
                            </div>

                            @can('manage_guardian')
                                <form method="POST" action="{{ route('students.guardians.destroy', [$student, $g]) }}"
                                      class="mt-2" onsubmit="return confirm('Hapus wali {{ addslashes($g->full_name) }}?');">
                                    @csrf @method('DELETE')
                                    <button class="btn btn-sm btn-light-danger py-1 px-3 fs-9">Hapus</button>
                                </form>
                            @endcan
                        </div>
                    @empty
                        <div class="alert alert-light-danger py-3 px-4 fs-8 mb-3">
                            Belum ada wali murid. Notifikasi absensi tidak akan terkirim.
                        </div>
                    @endforelse

                    @can('manage_guardian')
                        <div class="separator my-4"></div>
                        <form method="POST" action="{{ route('students.guardians.store', $student) }}">
                            @csrf
                            <span class="fw-semibold fs-7 d-block mb-3">Tambah wali</span>
                            <div class="row g-2 mb-2">
                                <div class="col-5">
                                    <select name="relation" class="form-select form-select-sm">
                                        @foreach (\App\Models\StudentGuardian::RELATIONS as $r)
                                            <option value="{{ $r }}">{{ ucfirst($r) }}</option>
                                        @endforeach
                                    </select>
                                </div>
                                <div class="col-7">
                                    <input type="text" name="full_name" class="form-control form-control-sm"
                                           placeholder="Nama wali" required maxlength="150">
                                </div>
                            </div>
                            <div class="row g-2 mb-2">
                                <div class="col-5">
                                    <select name="preferred_channel" class="form-select form-select-sm">
                                        @foreach (\App\Models\StudentGuardian::CHANNELS as $c)
                                            <option value="{{ $c }}">{{ $c === 'none' ? 'Tidak dikirimi' : ucfirst($c) }}</option>
                                        @endforeach
                                    </select>
                                </div>
                                <div class="col-7">
                                    <input type="text" name="whatsapp" class="form-control form-control-sm" placeholder="No. WhatsApp">
                                </div>
                            </div>
                            <div class="row g-2 mb-3">
                                <div class="col-7">
                                    <input type="email" name="email" class="form-control form-control-sm" placeholder="Email (opsional)">
                                </div>
                                <div class="col-5">
                                    <input type="text" name="telegram_chat_id" class="form-control form-control-sm" placeholder="Telegram ID">
                                </div>
                            </div>
                            <label class="form-check form-check-sm form-check-custom mb-3">
                                <input type="checkbox" class="form-check-input" name="is_primary" value="1"
                                       {{ $student->guardians->isEmpty() ? 'checked' : '' }}>
                                <span class="form-check-label fs-8">Jadikan kontak utama</span>
                            </label>
                            <button class="btn btn-sm btn-light-primary w-100">Tambah wali</button>
                        </form>
                    @endcan
                </div>
            </div>
        </div>
    </div>
@endsection

@push('scripts')
    @include('backend.attendance._hapus-script')
@endpush
