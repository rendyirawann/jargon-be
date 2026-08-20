@extends('backend.layout.app')
@section('title', 'Notifikasi Wali Murid')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Notifikasi Wali Murid</h2>
            <span class="text-muted fs-7">
                Pesan dikirim lewat WhatsApp, Telegram, atau Email sesuai kanal pilihan tiap wali.
            </span>
        </div>
        @include('backend.partials._school_picker', ['schools' => $schools, 'schoolId' => $schoolId, 'allowAll' => false])
    </div>

    <div class="row g-3 mb-5">
        @foreach ([
            ['Dalam Antrean', $stats['queued'], 'info'],
            ['Terkirim Hari Ini', $stats['sent_today'], 'success'],
            ['Gagal Hari Ini', $stats['failed_today'], 'danger'],
        ] as [$label, $value, $color])
            <div class="col-4">
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
        <div class="col-xl-7">
            @can('send_notification')
                <div class="card card-flush border border-gray-200">
                    <div class="card-header pt-5"><h3 class="card-title fw-bold">Kirim Pesan</h3></div>
                    <form method="POST" action="{{ route('notifications.send') }}" class="card-body pt-3">
                        @csrf

                        <div class="mb-4">
                            <label class="form-label required">Tujuan</label>
                            <div class="d-flex gap-4">
                                <label class="form-check form-check-custom">
                                    <input type="radio" class="form-check-input" name="target" value="classroom" checked>
                                    <span class="form-check-label fs-7">Satu kelas</span>
                                </label>
                                <label class="form-check form-check-custom">
                                    <input type="radio" class="form-check-input" name="target" value="students">
                                    <span class="form-check-label fs-7">Siswa tertentu</span>
                                </label>
                            </div>
                        </div>

                        <div class="mb-4">
                            <label class="form-label">Kelas</label>
                            <select name="classroom_id" class="form-select">
                                <option value="">Pilih kelas</option>
                                @foreach ($classrooms as $c)
                                    <option value="{{ $c->id }}">{{ $c->name }}</option>
                                @endforeach
                            </select>
                            <span class="form-text fs-9">
                                Pesan dikirim ke wali utama tiap siswa aktif di kelas ini (maksimum 500).
                            </span>
                        </div>

                        <div class="mb-4">
                            <label class="form-label">Kanal</label>
                            <select name="channel" class="form-select">
                                <option value="">Ikuti pilihan tiap wali (disarankan)</option>
                                <option value="whatsapp">Paksa WhatsApp</option>
                                <option value="telegram">Paksa Telegram</option>
                                <option value="email">Paksa Email</option>
                            </select>
                        </div>

                        <div class="mb-4">
                            <label class="form-label">Subjek (untuk email)</label>
                            <input type="text" name="subject" class="form-control" maxlength="200"
                                   value="{{ old('subject') }}" placeholder="Pengumuman dari sekolah">
                        </div>

                        <div class="mb-4">
                            <label class="form-label required">Isi pesan</label>
                            {{-- `@{{...}}` adalah cara Blade menampilkan kurung kurawal ganda
                                 secara literal; tanpa `@`, Blade akan mencoba mengevaluasinya. --}}
                            <textarea name="body" class="form-control" rows="5" required minlength="5" maxlength="4000"
                                      placeholder="Assalamualaikum Bapak/Ibu wali dari @{{nama_siswa}}, ...">{{ old('body') }}</textarea>
                            <span class="form-text fs-9">
                                Placeholder tersedia:
                                <code>@{{nama_siswa}}</code>,
                                <code>@{{kelas}}</code>,
                                <code>@{{sekolah}}</code>,
                                <code>@{{tanggal}}</code>,
                                <code>@{{nama_wali}}</code>
                            </span>
                        </div>

                        <div class="alert alert-light-warning py-3 px-4 fs-8 mb-4">
                            Wali yang kontaknya belum lengkap akan dilaporkan sebagai dilewati —
                            periksa daftar itu setelah pengiriman agar tidak ada orang tua yang
                            dianggap sudah menerima padahal tidak.
                        </div>

                        <button class="btn btn-primary" {{ $schoolId ? '' : 'disabled' }}>
                            <i class="ki-outline ki-send fs-5 me-1"></i>Kirim
                        </button>
                    </form>
                </div>
            @endcan
        </div>

        <div class="col-xl-5">
            @if ($policy)
                <div class="card card-flush border border-gray-200">
                    <div class="card-header pt-5"><h3 class="card-title fw-bold">Kebijakan Notifikasi Otomatis</h3></div>
                    <form method="POST" action="{{ route('notifications.policy') }}" class="card-body pt-3">
                        @csrf
                        <input type="hidden" name="school_id" value="{{ $schoolId }}">

                        @foreach ([
                            'notify_on_check_in' => 'Kirim saat siswa absen masuk',
                            'notify_on_late' => 'Kirim saat siswa terlambat',
                            'notify_on_absent' => 'Kirim saat siswa tidak hadir',
                            'notify_on_check_out' => 'Kirim saat siswa absen pulang',
                        ] as $field => $label)
                            <label class="form-check form-check-custom form-check-solid mb-4">
                                <input type="checkbox" class="form-check-input" name="{{ $field }}" value="1"
                                       {{ $policy->{$field} ? 'checked' : '' }}>
                                <span class="form-check-label fs-7">{{ $label }}</span>
                            </label>
                        @endforeach

                        <div class="separator my-4"></div>

                        <div class="mb-4">
                            <label class="form-label required">Kirim notifikasi alfa setelah jam</label>
                            <input type="time" name="absent_notify_after" class="form-control form-control-sm"
                                   value="{{ substr($policy->absent_notify_after, 0, 5) }}" required>
                            <span class="form-text fs-9">
                                Setelah jam ini, siswa yang belum discan ditandai tanpa keterangan
                                dan walinya diberi tahu. Isi setelah gerbang absen masuk ditutup.
                            </span>
                        </div>

                        <div class="row g-3 mb-4">
                            <div class="col-6">
                                <label class="form-label">Jam tenang mulai</label>
                                <input type="time" name="quiet_hours_start" class="form-control form-control-sm"
                                       value="{{ $policy->quiet_hours_start ? substr($policy->quiet_hours_start, 0, 5) : '' }}">
                            </div>
                            <div class="col-6">
                                <label class="form-label">Jam tenang selesai</label>
                                <input type="time" name="quiet_hours_end" class="form-control form-control-sm"
                                       value="{{ $policy->quiet_hours_end ? substr($policy->quiet_hours_end, 0, 5) : '' }}">
                            </div>
                        </div>

                        @can('manage_notification_template')
                            <button class="btn btn-primary w-100">Simpan kebijakan</button>
                        @endcan
                    </form>
                </div>
            @else
                <div class="card card-flush border border-gray-200">
                    <div class="card-body p-5 text-center text-muted fs-7">
                        Pilih sekolah untuk mengatur kebijakan notifikasi.
                    </div>
                </div>
            @endif
        </div>
    </div>
@endsection
