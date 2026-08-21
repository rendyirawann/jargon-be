@extends('backend.layout.app')
@section('title', 'Dashboard')

@section('content')
    @include('backend.partials._flash')

    {{-- ============================ Header ============================ --}}
    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">
                @if ($isProvince && ! $schoolId)
                    Ikhtisar Provinsi Sumatera Utara
                @else
                    {{ $school?->name ?? 'Dashboard' }}
                @endif
            </h2>
            <span class="text-muted fs-7">
                {{ \Illuminate\Support\Carbon::parse($date)->locale('id')->translatedFormat('l, d F Y') }}
            </span>
        </div>

        <div class="d-flex flex-wrap align-items-center gap-3">
            @include('backend.partials._school_picker', [
                'schools' => $schools,
                'schoolId' => $schoolId,
                'allowAll' => true,
            ])
            <form method="GET" class="d-flex align-items-center gap-2">
                @if ($schoolId)
                    <input type="hidden" name="school_id" value="{{ $schoolId }}">
                @endif
                <input type="date" name="date" value="{{ \Illuminate\Support\Carbon::parse($date)->toDateString() }}"
                       max="{{ now()->toDateString() }}"
                       class="form-control form-control-sm w-150px" onchange="this.form.submit()">
            </form>
        </div>
    </div>

    {{-- ==================== Kartu ringkasan absensi ==================== --}}
    <div class="row g-5 mb-5">
        @php
            // Elemen ke-5 = slug status; menentukan warna garis tepi kartu
            // (lihat .jg-stat--* di public/assets/css/jargon-theme.css).
            $cards = [
                ['Hadir', $summary['hadir'], 'success', 'ki-check-circle', 'hadir'],
                ['Terlambat', $summary['terlambat'], 'warning', 'ki-time', 'terlambat'],
                ['Izin / Sakit', $summary['izin'] + $summary['sakit'], 'info', 'ki-information', 'izin'],
                ['Tanpa Keterangan', $summary['alfa'], 'danger', 'ki-cross-circle', 'alfa'],
                ['Belum Absen', $summary['belum_absen'], 'secondary', 'ki-questionnaire-tablet', 'belum'],
            ];
        @endphp
        @foreach ($cards as [$label, $value, $color, $icon, $status])
            <div class="col-6 col-md-4 col-xl">
                <div class="card card-flush h-100 jg-stat jg-stat--{{ $status }}">
                    <div class="card-body p-5">
                        <div class="d-flex align-items-center justify-content-between mb-2">
                            <span class="jg-stat__label">{{ $label }}</span>
                            <i class="ki-duotone {{ $icon }} fs-2 jg-stat__icon">
                                <span class="path1"></span><span class="path2"></span>
                            </i>
                        </div>
                        <div class="jg-stat__value">{{ number_format($value) }}</div>
                    </div>
                </div>
            </div>
        @endforeach
    </div>

    <div class="row g-5 mb-5">
        {{-- Tingkat kehadiran --}}
        <div class="col-xl-3">
            <div class="card card-flush h-100 border border-gray-200">
                <div class="card-body p-5 text-center">
                    <span class="text-muted fs-8 text-uppercase fw-semibold d-block mb-3">Tingkat Kehadiran</span>
                    <div class="fs-3x fw-bold text-{{ $summary['rate'] >= 90 ? 'success' : ($summary['rate'] >= 75 ? 'warning' : 'danger') }}">
                        {{ $summary['rate'] }}%
                    </div>
                    <span class="text-muted fs-8">
                        dari {{ number_format($summary['total_students']) }} siswa aktif
                    </span>
                </div>
            </div>
        </div>

        {{-- Cakupan pendaftaran wajah --}}
        <div class="col-xl-3">
            <div class="card card-flush h-100 border border-gray-200">
                <div class="card-body p-5">
                    <span class="text-muted fs-8 text-uppercase fw-semibold d-block mb-3">Cakupan Data Wajah</span>
                    <div class="d-flex align-items-end justify-content-between mb-2">
                        <span class="fs-2hx fw-bold text-gray-900">{{ $biometric['percent'] }}%</span>
                        <span class="text-muted fs-8">{{ number_format($biometric['enrolled']) }} / {{ number_format($biometric['total']) }}</span>
                    </div>
                    <div class="jg-progress mb-3">
                        <div class="jg-progress__bar" style="width: {{ min(100, $biometric['percent']) }}%"></div>
                    </div>
                    @if ($biometric['total'] === 0)
                        {{-- Tanpa cabang ini, 0 dari 0 siswa tampil sebagai "Lengkap" —
                             menyesatkan, karena artinya belum ada data siswa sama sekali. --}}
                        <button type="button" class="btn btn-sm btn-light w-100 py-2"
                                data-jg-notify="Belum ada siswa aktif pada lingkup ini, jadi cakupan data wajah belum bisa dihitung. Tambahkan data siswa lebih dulu di menu Data Master &rsaquo; Siswa."
                                data-jg-notify-type="info">
                            Belum ada data siswa
                        </button>
                    @elseif ($biometric['not_enrolled'] > 0)
                        <a href="{{ route('biometric.index', ['school_id' => $schoolId, 'filter' => 'belum']) }}"
                           class="btn btn-sm btn-light-danger w-100 py-2">
                            {{ number_format($biometric['not_enrolled']) }} siswa belum terdaftar
                        </a>
                    @elseif ($biometric['under_sampled'] > 0)
                        <a href="{{ route('biometric.index', ['school_id' => $schoolId, 'filter' => 'kurang']) }}"
                           class="btn btn-sm btn-light-warning w-100 py-2">
                            {{ number_format($biometric['under_sampled']) }} sampel belum lengkap
                        </a>
                    @else
                        {{-- Tampak seperti tombol, dulu tidak bisa diklik. Sekarang
                             menjelaskan artinya saat ditekan. --}}
                        <button type="button" class="btn btn-sm btn-light-success w-100 py-2"
                                data-jg-notify="Seluruh {{ number_format($biometric['total']) }} siswa aktif sudah punya data wajah dengan jumlah sampel yang cukup. Tidak ada yang perlu didaftarkan."
                                data-jg-notify-type="success">
                            Lengkap
                        </button>
                    @endif
                </div>
            </div>
        </div>

        {{-- Perangkat --}}
        <div class="col-xl-3">
            <div class="card card-flush h-100 border border-gray-200">
                <div class="card-body p-5">
                    <span class="text-muted fs-8 text-uppercase fw-semibold d-block mb-3">Perangkat Tablet</span>
                    <div class="d-flex align-items-center gap-2 mb-2">
                        <span class="fs-2hx fw-bold text-gray-900">{{ $devices['online'] }}</span>
                        <span class="text-muted fs-6">/ {{ $devices['total'] }} online</span>
                    </div>
                    <div class="d-flex flex-column gap-1 fs-8">
                        @if ($devices['offline'] > 0)
                            <span class="text-warning">
                                <i class="ki-outline ki-information-2 fs-7"></i>
                                {{ $devices['offline'] }} perangkat offline
                            </span>
                        @endif
                        @if ($devices['never_paired'] > 0)
                            <span class="text-info">
                                <i class="ki-outline ki-information-2 fs-7"></i>
                                {{ $devices['never_paired'] }} belum dipasangkan
                            </span>
                        @endif
                        @if ($devices['offline'] === 0 && $devices['never_paired'] === 0 && $devices['total'] > 0)
                            <span class="text-success">Semua perangkat sehat</span>
                        @endif
                        @if ($devices['total'] === 0)
                            <span class="text-muted">Belum ada perangkat terdaftar</span>
                        @endif
                    </div>
                    @can('view_device')
                        <a href="{{ route('devices.index', ['school_id' => $schoolId]) }}"
                           class="btn btn-sm btn-light w-100 py-2 mt-3">
                            {{ $devices['total'] === 0 ? 'Daftarkan perangkat' : 'Kelola perangkat' }}
                        </a>
                    @endcan
                </div>
            </div>
        </div>

        {{-- Notifikasi --}}
        <div class="col-xl-3">
            <div class="card card-flush h-100 border border-gray-200">
                <div class="card-body p-5">
                    <span class="text-muted fs-8 text-uppercase fw-semibold d-block mb-3">Notifikasi Wali Murid</span>
                    <div class="d-flex justify-content-between mb-1">
                        <span class="fs-7 text-muted">Terkirim hari ini</span>
                        <span class="fs-6 fw-bold text-success">{{ number_format($notifications['sent']) }}</span>
                    </div>
                    <div class="d-flex justify-content-between mb-1">
                        <span class="fs-7 text-muted">Dalam antrean</span>
                        <span class="fs-6 fw-bold text-info">{{ number_format($notifications['queued']) }}</span>
                    </div>
                    <div class="d-flex justify-content-between">
                        <span class="fs-7 text-muted">Gagal</span>
                        <span class="fs-6 fw-bold text-{{ $notifications['failed'] > 0 ? 'danger' : 'gray-600' }}">
                            {{ number_format($notifications['failed']) }}
                        </span>
                    </div>
                    @can('view_notification')
                        <a href="{{ route('notifications.outbox', ['school_id' => $schoolId]) }}"
                           class="btn btn-sm btn-light w-100 py-2 mt-3">Lihat riwayat</a>
                    @endcan
                </div>
            </div>
        </div>
    </div>

    {{-- ===================== Tren + umpan langsung ===================== --}}
    <div class="row g-5 mb-5">
        <div class="col-xl-8">
            <div class="card card-flush h-100 border border-gray-200">
                <div class="card-header pt-5">
                    <h3 class="card-title fw-bold text-gray-900">Tren 7 Hari Terakhir</h3>
                </div>
                <div class="card-body pt-3">
                    <canvas id="trendChart" height="110"></canvas>
                </div>
            </div>
        </div>

        <div class="col-xl-4">
            <div class="card card-flush h-100 border border-gray-200">
                <div class="card-header pt-5">
                    <h3 class="card-title fw-bold text-gray-900">Scan Terbaru</h3>
                    <div class="card-toolbar">
                        <span class="badge badge-light-success">
                            <span class="bullet bullet-dot bg-success me-2"></span>langsung
                        </span>
                    </div>
                </div>
                <div class="card-body pt-3" style="max-height: 380px; overflow-y: auto;" id="liveFeed">
                    @forelse ($recent as $row)
                        <div class="d-flex align-items-center border-bottom border-gray-200 py-3">
                            <div class="flex-grow-1">
                                <span class="fw-semibold text-gray-800 fs-7 d-block">{{ $row->student_name }}</span>
                                <span class="text-muted fs-8">
                                    {{ $row->classroom_name ?? '-' }}
                                    @if ($isProvince && ! $schoolId) &middot; {{ $row->school_name }} @endif
                                </span>
                            </div>
                            <div class="text-end">
                                <span class="badge {{ $row->status_badge }} mb-1">{{ $row->status_label }}</span>
                                <span class="text-muted fs-8 d-block">
                                    {{ $row->check_out_at ? 'pulang '.$row->check_out_time : 'masuk '.$row->check_in_time }}
                                </span>
                            </div>
                        </div>
                    @empty
                        <div class="text-center text-muted py-10">
                            <i class="ki-duotone ki-information-5 fs-3x mb-3"><span class="path1"></span><span class="path2"></span><span class="path3"></span></i>
                            <p class="mb-0 fs-7">Belum ada scan absensi pada tanggal ini.</p>
                        </div>
                    @endforelse
                </div>
            </div>
        </div>
    </div>

    {{-- ============== Panel khusus: provinsi vs sekolah ============== --}}
    @if ($isProvince && ! $schoolId)
        <div class="row g-5 mb-5">
            <div class="col-xl-4">
                <div class="card card-flush h-100 border border-gray-200">
                    <div class="card-header pt-5"><h3 class="card-title fw-bold">Pelaporan Hari Ini</h3></div>
                    <div class="card-body pt-3">
                        <div class="d-flex justify-content-between mb-3">
                            <span class="text-muted fs-7">Sekolah aktif</span>
                            <span class="fw-bold">{{ number_format($provinceStats['active_schools']) }}</span>
                        </div>
                        <div class="d-flex justify-content-between mb-3">
                            <span class="text-muted fs-7">Sudah melapor absensi</span>
                            <span class="fw-bold text-success">{{ number_format($provinceStats['reporting_schools']) }}</span>
                        </div>
                        <div class="d-flex justify-content-between mb-4">
                            <span class="text-muted fs-7">Belum melapor</span>
                            <span class="fw-bold text-danger">
                                {{ number_format(max(0, $provinceStats['active_schools'] - $provinceStats['reporting_schools'])) }}
                            </span>
                        </div>

                        @if ($apiHealth)
                            <div class="separator mb-4"></div>
                            <span class="text-muted fs-8 text-uppercase fw-semibold d-block mb-2">Kesehatan Layanan API</span>
                            <div class="d-flex justify-content-between fs-8 mb-1">
                                <span class="text-muted">Status</span>
                                <span class="badge badge-light-{{ ($apiHealth['status'] ?? '') === 'ok' ? 'success' : 'danger' }}">
                                    {{ $apiHealth['status'] ?? 'tidak diketahui' }}
                                </span>
                            </div>
                            <div class="d-flex justify-content-between fs-8 mb-1">
                                <span class="text-muted">Database</span>
                                <span class="{{ ($apiHealth['database']['available'] ?? false) ? 'text-success' : 'text-danger' }}">
                                    {{ ($apiHealth['database']['available'] ?? false) ? 'terhubung' : 'gagal' }}
                                </span>
                            </div>
                            <div class="d-flex justify-content-between fs-8 mb-1">
                                <span class="text-muted">Index wajah ter-cache</span>
                                <span>{{ number_format($apiHealth['face_index']['cached_samples'] ?? 0) }} sampel</span>
                            </div>
                            <div class="d-flex justify-content-between fs-8">
                                <span class="text-muted">Versi model</span>
                                <span><code>{{ $apiHealth['face_index']['model_version'] ?? '-' }}</code></span>
                            </div>
                        @else
                            <div class="alert alert-warning d-flex align-items-center py-3 px-4 mb-0 mt-4">
                                <i class="ki-duotone ki-information-5 fs-2 me-3"><span class="path1"></span><span class="path2"></span><span class="path3"></span></i>
                                <span class="fs-8">Layanan API tidak dapat dihubungi. Absensi dari tablet tidak akan tercatat.</span>
                            </div>
                        @endif
                    </div>
                </div>
            </div>

            <div class="col-xl-4">
                <div class="card card-flush h-100 border border-gray-200">
                    <div class="card-header pt-5">
                        <h3 class="card-title fw-bold text-danger">Perlu Perhatian</h3>
                        <span class="text-muted fs-8">kehadiran terendah</span>
                    </div>
                    <div class="card-body pt-3 p-0">
                        <div class="table-responsive">
                            <table class="table table-row-dashed align-middle mb-0">
                                <tbody>
                                    @forelse ($lowestSchools as $s)
                                        <tr>
                                            <td class="ps-5">
                                                <a href="{{ route('dashboard', ['school_id' => $s->id]) }}"
                                                   class="fw-semibold fs-8 text-gray-800 text-hover-primary">{{ $s->name }}</a>
                                                <span class="text-muted fs-9 d-block">{{ $s->jenjang }} &middot; {{ $s->total_students }} siswa</span>
                                            </td>
                                            <td class="text-end pe-5">
                                                <span class="badge badge-light-danger">{{ $s->rate }}%</span>
                                            </td>
                                        </tr>
                                    @empty
                                        <tr><td class="text-center text-muted py-8 fs-8">Belum ada data absensi hari ini.</td></tr>
                                    @endforelse
                                </tbody>
                            </table>
                        </div>
                    </div>
                </div>
            </div>

            <div class="col-xl-4">
                <div class="card card-flush h-100 border border-gray-200">
                    <div class="card-header pt-5">
                        <h3 class="card-title fw-bold text-success">Kehadiran Terbaik</h3>
                    </div>
                    <div class="card-body pt-3 p-0">
                        <div class="table-responsive">
                            <table class="table table-row-dashed align-middle mb-0">
                                <tbody>
                                    @forelse ($topSchools as $s)
                                        <tr>
                                            <td class="ps-5">
                                                <a href="{{ route('dashboard', ['school_id' => $s->id]) }}"
                                                   class="fw-semibold fs-8 text-gray-800 text-hover-primary">{{ $s->name }}</a>
                                                <span class="text-muted fs-9 d-block">{{ $s->jenjang }} &middot; {{ $s->total_students }} siswa</span>
                                            </td>
                                            <td class="text-end pe-5">
                                                <span class="badge badge-light-success">{{ $s->rate }}%</span>
                                            </td>
                                        </tr>
                                    @empty
                                        <tr><td class="text-center text-muted py-8 fs-8">Belum ada data absensi hari ini.</td></tr>
                                    @endforelse
                                </tbody>
                            </table>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    @else
        <div class="row g-5">
            <div class="col-xl-8">
                <div class="card card-flush border border-gray-200">
                    <div class="card-header pt-5">
                        <h3 class="card-title fw-bold text-gray-900">Rekap per Kelas</h3>
                        <div class="card-toolbar">
                            <a href="{{ route('attendances.by-classroom', ['school_id' => $schoolId, 'date' => \Illuminate\Support\Carbon::parse($date)->toDateString()]) }}"
                               class="btn btn-sm btn-light">Lihat semua</a>
                        </div>
                    </div>
                    <div class="card-body pt-3 p-0">
                        <div class="table-responsive">
                            <table class="table table-row-dashed align-middle mb-0">
                                <thead>
                                    <tr class="fw-semibold fs-8 text-muted text-uppercase">
                                        <th class="ps-5">Kelas</th>
                                        <th class="text-center">Hadir</th>
                                        <th class="text-center">Telat</th>
                                        <th class="text-center">Izin/Sakit</th>
                                        <th class="text-center">Alfa</th>
                                        <th class="text-center pe-5">Belum Absen</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    @forelse ($classrooms as $c)
                                        <tr>
                                            <td class="ps-5">
                                                <span class="fw-semibold fs-7 text-gray-800">{{ $c->name }}</span>
                                                <span class="text-muted fs-9 d-block">
                                                    {{ $c->homeroom_teacher_name ?? 'Belum ada wali kelas' }}
                                                    &middot; {{ $c->total_students }} siswa
                                                </span>
                                            </td>
                                            <td class="text-center fw-bold text-success">{{ $c->hadir }}</td>
                                            <td class="text-center fw-bold text-warning">{{ $c->terlambat }}</td>
                                            <td class="text-center">{{ $c->izin + $c->sakit }}</td>
                                            <td class="text-center fw-bold text-danger">{{ $c->alfa }}</td>
                                            <td class="text-center pe-5">
                                                @if ($c->belum_absen > 0)
                                                    <span class="badge badge-light-secondary">{{ $c->belum_absen }}</span>
                                                @else
                                                    <i class="ki-outline ki-check text-success fs-4"></i>
                                                @endif
                                            </td>
                                        </tr>
                                    @empty
                                        <tr>
                                            <td colspan="6" class="text-center text-muted py-10 fs-7">
                                                Belum ada kelas aktif.
                                                @can('create_classroom')
                                                    <a href="{{ route('classrooms.index', ['school_id' => $schoolId]) }}">Buat kelas</a>
                                                @endcan
                                            </td>
                                        </tr>
                                    @endforelse
                                </tbody>
                            </table>
                        </div>
                    </div>
                </div>
            </div>

            <div class="col-xl-4">
                <div class="card card-flush border border-gray-200 h-100">
                    <div class="card-header pt-5">
                        <h3 class="card-title fw-bold text-gray-900">Belum Terdaftar Wajah</h3>
                    </div>
                    <div class="card-body pt-3">
                        @forelse ($pendingFaces as $s)
                            <div class="d-flex align-items-center justify-content-between border-bottom border-gray-200 py-3">
                                <div>
                                    <span class="fw-semibold fs-7 text-gray-800 d-block">{{ $s->full_name }}</span>
                                    <span class="text-muted fs-8">{{ $s->classroom?->name ?? 'Tanpa kelas' }} &middot; {{ $s->nis ?? '-' }}</span>
                                </div>
                                @can('create_face_enrollment')
                                    <a href="{{ route('biometric.capture', $s) }}" class="btn btn-sm btn-light-primary py-1 px-3">Daftarkan</a>
                                @endcan
                            </div>
                        @empty
                            <div class="text-center text-muted py-10">
                                <i class="ki-duotone ki-check-circle fs-3x text-success mb-3"><span class="path1"></span><span class="path2"></span></i>
                                <p class="mb-0 fs-7">Semua siswa aktif sudah terdaftar wajahnya.</p>
                            </div>
                        @endforelse

                        @if ($pendingFaces->isNotEmpty())
                            <a href="{{ route('biometric.index', ['school_id' => $schoolId, 'filter' => 'belum']) }}"
                               class="btn btn-sm btn-light w-100 mt-4">Lihat semua</a>
                        @endif
                    </div>
                </div>
            </div>
        </div>
    @endif
@endsection

@push('scripts')
    <script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.1/dist/chart.umd.min.js"></script>
    <script>
        (function () {
            const el = document.getElementById('trendChart');
            if (!el || typeof Chart === 'undefined') return;

            new Chart(el, {
                type: 'bar',
                data: {
                    labels: @json($trend['labels']),
                    datasets: [
                        { label: 'Hadir',     data: @json($trend['hadir']),     backgroundColor: '#50cd89' },
                        { label: 'Terlambat', data: @json($trend['terlambat']), backgroundColor: '#ffc700' },
                        { label: 'Alfa',      data: @json($trend['alfa']),      backgroundColor: '#f1416c' },
                    ],
                },
                options: {
                    responsive: true,
                    maintainAspectRatio: false,
                    scales: {
                        // Ditumpuk agar total kehadiran per hari langsung terbaca.
                        x: { stacked: true, grid: { display: false } },
                        y: { stacked: true, beginAtZero: true, ticks: { precision: 0 } },
                    },
                    plugins: { legend: { position: 'bottom' } },
                },
            });
        })();

        // Umpan scan disegarkan berkala. Hanya menyentuh endpoint ringan
        // (LIMIT 30, satu partisi) sehingga aman dijalankan tiap 20 detik.
        (function () {
            const feed = document.getElementById('liveFeed');
            if (!feed) return;

            const url = new URL(@json(route('attendances.live')), window.location.origin);
            @if ($schoolId)
                url.searchParams.set('school_id', @json($schoolId));
            @endif

            const isToday = @json(\Illuminate\Support\Carbon::parse($date)->isToday());
            if (!isToday) return;

            async function refresh() {
                try {
                    const res = await fetch(url, { headers: { 'Accept': 'application/json' } });
                    if (!res.ok) return;
                    const payload = await res.json();
                    if (!payload.items || payload.items.length === 0) return;

                    const showSchool = @json($isProvince && ! $schoolId);

                    // Baris dibangun lewat DOM API, bukan innerHTML: nama siswa
                    // dan nama sekolah adalah data, dan tidak boleh sampai
                    // dieksekusi sebagai markup.
                    const frag = document.createDocumentFragment();

                    payload.items.forEach(function (item) {
                        const row = document.createElement('div');
                        row.className = 'd-flex align-items-center border-bottom border-gray-200 py-3';

                        const left = document.createElement('div');
                        left.className = 'flex-grow-1';

                        const name = document.createElement('span');
                        name.className = 'fw-semibold text-gray-800 fs-7 d-block';
                        name.textContent = item.student_name;

                        const meta = document.createElement('span');
                        meta.className = 'text-muted fs-8';
                        meta.textContent = (item.classroom_name || '-')
                            + (showSchool ? ' · ' + item.school_name : '');

                        left.append(name, meta);

                        const right = document.createElement('div');
                        right.className = 'text-end';

                        const badge = document.createElement('span');
                        badge.className = 'badge ' + item.badge + ' mb-1';
                        badge.textContent = item.status_label;

                        const time = document.createElement('span');
                        time.className = 'text-muted fs-8 d-block';
                        time.textContent = item.direction === 'pulang'
                            ? 'pulang ' + item.check_out
                            : 'masuk ' + item.check_in;

                        right.append(badge, time);
                        row.append(left, right);
                        frag.append(row);
                    });

                    feed.replaceChildren(frag);
                } catch (e) {
                    // Jaringan sekolah sering tidak stabil; gagal menyegarkan
                    // bukan alasan menampilkan error ke pengguna.
                }
            }

            setInterval(refresh, 20000);
        })();
    </script>
@endpush
