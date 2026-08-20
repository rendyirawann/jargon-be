@extends('backend.layout.app')
@section('title', 'Detail Pengaduan')

@section('content')
    @include('backend.partials._flash')

    {{-- Identitas yang baru dibuka hanya ditampilkan sekali lewat flash
         session; tidak disimpan di tabel dashboard mana pun. --}}
    @if (session('unmasked'))
        @php $u = session('unmasked'); @endphp
        <div class="alert alert-danger mt-5 mb-0">
            <div class="d-flex align-items-start">
                <i class="ki-duotone ki-eye fs-2x me-4"><span class="path1"></span><span class="path2"></span><span class="path3"></span></i>
                <div class="flex-grow-1">
                    <span class="fw-bold d-block mb-2">Identitas Pelapor</span>
                    <div class="row g-2 fs-7">
                        <div class="col-md-4"><span class="text-muted">Nama:</span> <strong>{{ $u['name'] }}</strong></div>
                        <div class="col-md-3"><span class="text-muted">NIK/NISN:</span> <strong>{{ $u['identity_number'] }}</strong></div>
                        <div class="col-md-2"><span class="text-muted">Peran:</span> <strong>{{ $u['role'] }}</strong></div>
                        <div class="col-md-3"><span class="text-muted">Sekolah:</span> <strong>{{ $u['school_name'] }}</strong></div>
                    </div>
                    <span class="fs-8 d-block mt-3">{{ $u['notice'] }}</span>
                </div>
            </div>
        </div>
    @endif

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">{{ $report->title }}</h2>
            <span class="text-muted fs-7">
                {{ $report->anonymous_handle }} &middot; {{ $report->category->name ?? '-' }}
                &middot; {{ $report->school->name ?? '-' }}
                &middot; {{ $report->created_at->timezone(config('app.timezone'))->format('d/m/Y H:i') }}
            </span>
        </div>
        <a href="{{ route('panic.index') }}" class="btn btn-sm btn-light">Kembali</a>
    </div>

    <div class="row g-5">
        <div class="col-xl-8">
            <div class="card card-flush border border-gray-200 mb-5">
                <div class="card-body p-5">
                    <div class="d-flex flex-wrap gap-2 mb-4">
                        <span class="badge {{ $report->severity_badge }}">Keparahan: {{ ucfirst($report->severity) }}</span>
                        <span class="badge {{ $report->status_badge }}">{{ $report->status_label }}</span>
                        <span class="badge badge-light">{{ $report->support_count }} orang mengalami serupa</span>
                    </div>
                    <p class="fs-6 text-gray-800 mb-0" style="white-space: pre-line; line-height: 1.7;">{{ $report->body }}</p>
                </div>
            </div>

            @if ($report->resolution)
                <div class="alert alert-success mb-5">
                    <span class="fw-bold d-block mb-1">Hasil Penanganan</span>
                    <span class="fs-7">{{ $report->resolution }}</span>
                </div>
            @endif

            {{-- Lini masa --}}
            <div class="card card-flush border border-gray-200 mb-5">
                <div class="card-header pt-5"><h3 class="card-title fw-bold">Lini Masa Penanganan</h3></div>
                <div class="card-body pt-3">
                    @forelse ($report->events as $e)
                        <div class="d-flex border-bottom border-gray-200 py-3">
                            <div class="flex-grow-1">
                                <span class="fw-semibold fs-7 d-block">{{ ucfirst($e->status) }}</span>
                                @if ($e->note)
                                    <span class="text-gray-700 fs-8 d-block mt-1">{{ $e->note }}</span>
                                @endif
                                <span class="text-muted fs-9 d-block mt-1">
                                    {{ $e->actor_label ?? 'Sistem' }} &middot;
                                    {{ $e->created_at->timezone(config('app.timezone'))->format('d/m/Y H:i') }}
                                    @unless ($e->is_public)
                                        &middot; <em>catatan internal</em>
                                    @endunless
                                </span>
                            </div>
                        </div>
                    @empty
                        <span class="text-muted fs-7">Belum ada tindak lanjut.</span>
                    @endforelse
                </div>
            </div>

            {{-- Komentar --}}
            <div class="card card-flush border border-gray-200">
                <div class="card-header pt-5">
                    <h3 class="card-title fw-bold">Komentar ({{ $report->comments->count() }})</h3>
                </div>
                <div class="card-body pt-3">
                    @forelse ($report->comments as $c)
                        <div class="border border-gray-200 rounded p-3 mb-3 {{ $c->is_official ? 'bg-light-info' : '' }}">
                            <div class="d-flex align-items-center gap-2 mb-2">
                                <span class="fw-semibold fs-8">{{ $c->display_name }}</span>
                                @if ($c->is_official)
                                    <span class="badge badge-light-info fs-9">{{ $c->official_title ?? 'Petugas' }}</span>
                                @endif
                                <span class="text-muted fs-9 ms-auto">
                                    {{ $c->created_at->timezone(config('app.timezone'))->format('d/m H:i') }}
                                </span>
                            </div>
                            <span class="fs-8 text-gray-800">{{ $c->body }}</span>
                        </div>
                    @empty
                        <span class="text-muted fs-7">Belum ada komentar.</span>
                    @endforelse

                    @can('handle_panic_report')
                        <form method="POST" action="{{ route('panic.comment', $report->id) }}" class="mt-4">
                            @csrf
                            <label class="form-label">Balas sebagai petugas</label>
                            <textarea name="body" class="form-control mb-3" rows="3" required
                                      minlength="2" maxlength="2000"
                                      placeholder="Balasan Anda akan menampilkan nama dan jabatan, agar pelapor tahu laporannya ditangani."></textarea>
                            <input type="hidden" name="as_official" value="1">
                            <button class="btn btn-sm btn-primary">Kirim Balasan</button>
                        </form>
                    @endcan
                </div>
            </div>
        </div>

        <div class="col-xl-4">
            {{-- Moderasi --}}
            @can('moderate_panic_report')
                <div class="card card-flush border border-gray-200 mb-5">
                    <div class="card-header pt-5"><h3 class="card-title fw-bold">Moderasi Tampilan</h3></div>
                    <form method="POST" action="{{ route('panic.moderate', $report->id) }}" class="card-body pt-3">
                        @csrf
                        <div class="alert alert-light-info py-3 px-4 fs-9 mb-4">
                            Moderasi hanya menentukan apakah laporan tampil di beranda aplikasi.
                            Penanganannya tetap berjalan apa pun hasilnya.
                        </div>
                        <div class="mb-3">
                            <span class="text-muted fs-8">Status saat ini:</span>
                            <span class="badge badge-light-{{ $report->moderation_status === 'approved' ? 'success' : ($report->moderation_status === 'rejected' ? 'danger' : 'warning') }}">
                                {{ $report->moderation_status }}
                            </span>
                        </div>
                        <div class="mb-3">
                            <label class="form-label required">Keputusan</label>
                            <select name="moderation_status" class="form-select form-select-sm" required>
                                <option value="approved">Setujui tampil di beranda</option>
                                <option value="rejected">Tolak tampil</option>
                            </select>
                        </div>
                        <div class="mb-3">
                            <label class="form-label">Catatan</label>
                            <input type="text" name="note" class="form-control form-control-sm" maxlength="300">
                        </div>
                        <button class="btn btn-sm btn-light-primary w-100">Simpan Moderasi</button>
                    </form>
                </div>
            @endcan

            {{-- Tindak lanjut --}}
            @can('handle_panic_report')
                <div class="card card-flush border border-gray-200 mb-5">
                    <div class="card-header pt-5"><h3 class="card-title fw-bold">Tindak Lanjut</h3></div>
                    <form method="POST" action="{{ route('panic.status', $report->id) }}" class="card-body pt-3">
                        @csrf
                        <div class="mb-3">
                            <label class="form-label required">Status baru</label>
                            <select name="status" class="form-select form-select-sm" required>
                                @foreach ($statuses as $s)
                                    <option value="{{ $s }}" {{ $report->status === $s ? 'selected' : '' }}>
                                        {{ ucfirst($s) }}
                                    </option>
                                @endforeach
                            </select>
                        </div>
                        <div class="mb-3">
                            <label class="form-label required">Catatan tindak lanjut</label>
                            <textarea name="note" class="form-control form-control-sm" rows="3" required
                                      minlength="3" maxlength="500"></textarea>
                        </div>
                        <div class="mb-3">
                            <label class="form-label">Hasil penanganan (wajib bila selesai)</label>
                            <textarea name="resolution" class="form-control form-control-sm" rows="3" maxlength="2000">{{ $report->resolution }}</textarea>
                        </div>
                        <label class="form-check form-check-sm form-check-custom mb-4">
                            <input type="checkbox" class="form-check-input" name="visible_to_reporter" value="1" checked>
                            <span class="form-check-label fs-8">Tampilkan catatan ini kepada pelapor</span>
                        </label>
                        <button class="btn btn-sm btn-primary w-100">Simpan</button>
                    </form>
                </div>
            @endcan

            {{-- Buka identitas --}}
            @can('unmask_panic_report')
                <div class="card card-flush border border-danger">
                    <div class="card-header pt-5">
                        <h3 class="card-title fw-bold text-danger">Buka Identitas Pelapor</h3>
                    </div>
                    <form method="POST" action="{{ route('panic.unmask', $report->id) }}" class="card-body pt-3">
                        @csrf
                        <div class="alert alert-light-danger py-3 px-4 fs-9 mb-4">
                            <span class="fw-semibold d-block mb-1">Gunakan hanya bila benar-benar diperlukan.</span>
                            Anonimitas adalah satu-satunya yang membuat siswa berani melaporkan
                            perundungan dan pungli. Setiap pembukaan tercatat permanen beserta
                            nama dan alasan Anda, dan tidak dapat dihapus.
                        </div>
                        <div class="mb-3">
                            <label class="form-label required">Alasan (minimal 20 karakter)</label>
                            <textarea name="reason" class="form-control form-control-sm" rows="3" required
                                      minlength="20" maxlength="500"
                                      placeholder="mis. Permintaan penyidik Polrestabes Medan nomor B/123/VIII/2026"></textarea>
                        </div>
                        <button class="btn btn-sm btn-danger w-100"
                                onclick="return confirm('Buka identitas pelapor?\n\nTindakan ini dicatat permanen beserta nama dan alasan Anda.');">
                            Buka Identitas
                        </button>
                    </form>
                </div>

                @if ($unmaskLogs->isNotEmpty())
                    <div class="card card-flush border border-gray-200 mt-5">
                        <div class="card-header pt-5"><h3 class="card-title fw-bold">Riwayat Pembukaan</h3></div>
                        <div class="card-body pt-3">
                            @foreach ($unmaskLogs as $log)
                                <div class="border-bottom border-gray-200 py-3">
                                    <span class="fw-semibold fs-8 d-block">{{ $log->actor_label }}</span>
                                    <span class="text-muted fs-9 d-block">
                                        {{ $log->created_at->timezone(config('app.timezone'))->format('d/m/Y H:i') }}
                                    </span>
                                    <span class="fs-9 text-gray-700 d-block mt-1">{{ $log->reason }}</span>
                                </div>
                            @endforeach
                        </div>
                    </div>
                @endif
            @endcan
        </div>
    </div>
@endsection
