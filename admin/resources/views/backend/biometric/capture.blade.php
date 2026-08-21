@extends('backend.layout.app')
@section('title', 'Daftarkan Wajah - '.$student->full_name)

@php
    /**
     * Urutan pose yang diminta, dan mana yang sudah tersimpan.
     *
     * Kemajuan dibaca dari DATA, bukan disimpan di state JavaScript.
     * Akibatnya alurnya tahan terhadap refresh, tombol kembali, dan
     * penutupan tab: operator yang berhenti setelah pose depan akan
     * melanjutkan tepat di pose berikutnya, bukan mulai dari awal.
     */
    $urutan = [
        'frontal' => ['label' => 'Menghadap depan',   'petunjuk' => 'Lihat lurus ke kamera.'],
        'right'   => ['label' => 'Menoleh ke kanan',  'petunjuk' => 'Putar kepala ke KANAN Anda, tahan.'],
        'left'    => ['label' => 'Menoleh ke kiri',   'petunjuk' => 'Putar kepala ke KIRI Anda, tahan.'],
    ];
    $semuaPose = array_keys($urutan);
    $poseTersimpan = $samples->pluck('pose')->unique()->all();
    $lengkap = count(array_intersect($semuaPose, $poseTersimpan)) === count($semuaPose);

    /*
     * Satu sesi pendaftaran selalu utuh tiga pose, dan menimpa yang lama.
     *
     * Versi sebelumnya MELANJUTKAN dari pose yang belum ada, sehingga sampel
     * separuh jalan ikut terpakai. Itu bertabrakan dengan aturan "belum lengkap
     * berarti tidak ada yang tersimpan": sisa satu-dua pose dari percobaan yang
     * ditinggalkan bukan bahan yang layak dipakai melanjutkan, karena bisa
     * berasal dari pencahayaan, jarak, bahkan orang yang berbeda.
     *
     * Jadi: bila belum lengkap, urutan selalu dimulai dari pose pertama dan
     * sampel lama ditimpa saat sesi ini selesai (lihat storeBatch).
     */
    $sudah = $lengkap ? $poseTersimpan : [];
    $berikut = $lengkap ? null : $semuaPose[0];
@endphp

@section('content')
    <style>
        /* Panduan arah dibuat BESAR dan bergerak.
           Teks kecil tidak terbaca dari jarak satu meter, dan siswa yang
           tidak paham harus berbuat apa akan berdiri diam sampai petugas
           menjelaskan - itu yang membuat antrean pagi menumpuk. */
        .arah-panah {
            position: absolute;
            top: 50%;
            transform: translateY(-50%);
            font-size: 5rem;
            line-height: 1;
            color: #ffc700;
            text-shadow: 0 3px 14px rgba(0,0,0,.9);
            pointer-events: none;
            animation: arah-denyut 1s ease-in-out infinite;
        }
        .arah-panah.kanan { right: 6%; }
        .arah-panah.kiri  { left: 6%; }
        @keyframes arah-denyut {
            0%, 100% { opacity: .45; transform: translateY(-50%) scale(1); }
            50%      { opacity: 1;   transform: translateY(-50%) scale(1.18); }
        }
        .arah-teks {
            position: absolute;
            left: 0; right: 0;
            bottom: 4.5rem;
            text-align: center;
            font-weight: 800;
            font-size: 1.6rem;
            color: #fff;
            text-shadow: 0 2px 10px rgba(0,0,0,.9);
            pointer-events: none;
        }
    </style>
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Pendaftaran Wajah</h2>
            <span class="text-muted fs-7">
                {{ $student->full_name }} &middot; {{ $student->classroom?->name ?? 'Belum ditempatkan' }}
                &middot; {{ $student->school->name }}
            </span>
        </div>
        <a href="{{ route('students.show', $student) }}" class="btn btn-sm btn-light">Kembali ke detail siswa</a>
    </div>

    {{-- Langkah-langkah, dengan yang sudah selesai ditandai. --}}
    <div class="card card-flush border border-gray-200 mb-5" id="kartuLangkah"
         data-target="{{ $berikut }}" data-lengkap="{{ $lengkap ? 1 : 0 }}">
        <div class="card-body p-4">
            <div class="d-flex flex-wrap gap-3">
                @foreach ($urutan as $pose => $info)
                    @php
                        $selesai = in_array($pose, $sudah, true);
                        $aktif = $pose === $berikut;
                    @endphp
                    <div data-pose="{{ $pose }}" class="flex-grow-1 d-flex align-items-center gap-3 rounded p-3
                                {{ $selesai ? 'bg-light-success' : ($aktif ? 'bg-light-primary' : 'bg-light') }}">
                        <span class="fs-2" data-ikon>{!! $selesai ? '&#9989;' : ($aktif ? '&#128248;' : '&#9675;') !!}</span>
                        <div>
                            <span class="fw-bold fs-7 d-block
                                {{ $selesai ? 'text-success' : ($aktif ? 'text-primary' : 'text-muted') }}">
                                {{ $loop->iteration }}. {{ $info['label'] }}
                            </span>
                            <span class="fs-9 text-muted" data-status>
                                {{ $selesai ? 'tersimpan' : ($aktif ? $info['petunjuk'] : 'menunggu') }}
                            </span>
                        </div>
                    </div>
                @endforeach
            </div>
        </div>
    </div>

    @if ($lengkap)
        <div class="alert alert-success d-flex align-items-center">
            <span class="fs-2x me-4">&#127881;</span>
            <div>
                <span class="fw-bold d-block mb-1">Ketiga pose sudah tersimpan.</span>
                <span class="fs-8">
                    {{ $student->full_name }} siap dikenali. Untuk mengganti salah satu pose,
                    hapus sampelnya di <a href="{{ route('biometric.show', $student) }}">daftar sampel</a>
                    lalu ambil ulang.
                </span>
            </div>
        </div>
    @endif

    <div class="row g-5">
        <div class="col-xl-7">
            <div class="card card-flush border border-gray-200">
                <div class="card-header pt-5">
                    <h3 class="card-title fw-bold">
                        {{ $lengkap ? 'Tambah Sampel' : 'Langkah: '.$urutan[$berikut]['label'] }}
                    </h3>
                    <div class="card-toolbar">
                        <span class="badge badge-light-primary fs-8" id="badgeSampel">
                            {{ $student->face_sample_count }} sampel
                        </span>
                    </div>
                </div>
                <div class="card-body pt-3">
                    <div class="alert alert-light-primary py-3 px-4 fs-8 mb-4">
                        <span class="fw-semibold d-block mb-1">Cara mendapatkan hasil terbaik</span>
                        Pencahayaan merata (jangan membelakangi jendela), tanpa masker atau
                        kacamata gelap. Foto diambil <strong>otomatis</strong> begitu posisi
                        kepala benar dan tertahan sebentar &mdash; tidak perlu menekan tombol.
                    </div>

                    <div class="position-relative bg-dark rounded overflow-hidden mb-4"
                         style="aspect-ratio: 4/3;">
                        {{-- Dicerminkan seperti kaca, supaya "putar ke kanan" terasa
                             sesuai dengan yang dilihat di layar. Perhitungan yaw
                             tetap memakai piksel asli, bukan tampilan ini. --}}
                        <video id="video" autoplay playsinline muted
                               class="w-100 h-100" style="object-fit: cover; transform: scaleX(-1);"></video>
                        <canvas id="canvas" class="d-none"></canvas>

                        <div id="guide"
                             class="position-absolute top-50 start-50 translate-middle border border-4 rounded-circle border-white opacity-50"
                             style="width: 46%; aspect-ratio: 1; pointer-events: none; transition: border-color .2s;"></div>

                        <div id="bigHint"
                             class="position-absolute top-0 start-0 end-0 text-center text-white fw-bold fs-3 pt-4"
                             style="text-shadow: 0 2px 8px rgba(0,0,0,.8); pointer-events: none;"></div>
                        <div id="arrowKiri" class="arah-panah kiri d-none">&#11013;</div>
                        <div id="arrowKanan" class="arah-panah kanan d-none">&#10145;</div>
                        <div id="arahTeks" class="arah-teks"></div>

                        <div id="camStatus"
                             class="position-absolute bottom-0 start-0 end-0 bg-dark bg-opacity-75 text-white p-3 fs-8">
                            Menyiapkan kamera...
                        </div>
                    </div>

                    <form method="POST" action="{{ route('biometric.store', $student) }}" id="enrollForm">
                        @csrf
                        <input type="hidden" name="image_base64" id="imageBase64">
                        <input type="hidden" name="embedding" id="embeddingJson">
                        <input type="hidden" name="model_version" value="{{ $modelVersion }}">
                        {{-- Pose ditentukan LANGKAH, bukan dipilih operator: operator
                             yang salah memilih akan menyimpan foto depan sebagai
                             sampel "kanan", dan kekeliruan itu baru terasa nanti
                             sebagai pengenalan yang buruk. --}}
                        <input type="hidden" name="pose" id="poseField"
                               value="{{ $berikut ?? 'frontal' }}">

                        <div class="d-flex flex-wrap gap-2">
                            <select id="poseSelect" class="form-select w-auto {{ $lengkap ? '' : 'd-none' }}">
                                @foreach ($urutan as $pose => $info)
                                    <option value="{{ $pose }}">{{ $info['label'] }}</option>
                                @endforeach
                            </select>
                            <button type="button" class="btn btn-light-primary" id="btnManual" disabled>
                                Ambil sekarang
                            </button>
                            <button type="button" class="btn btn-light" id="btnRetry">Ulangi kamera</button>
                            @can('delete_face_enrollment')
                            <button type="button" class="btn btn-light-danger" id="btnReset"
                                    data-url="{{ route('biometric.reset', $student) }}">
                                Ulangi dari awal
                            </button>
                            @endcan
                        </div>
                    </form>

                    {{-- Ditampilkan JS begitu ketiga pose tersimpan. Kamera dimatikan di
                         titik itu: menangkap terus setelah lengkap hanya menumpuk sampel
                         pose yang sama (pernah terjadi: 17 sampel frontal berurutan). --}}
                    @if (! $lengkap && $samples->isNotEmpty())
                        <div class="alert alert-warning d-flex align-items-center gap-3 mt-4">
                            <span class="fs-2">&#9888;</span>
                            <div>
                                <span class="fw-bold d-block">Ada {{ $samples->count() }} sampel lama yang belum lengkap.</span>
                                <span class="fs-8">Pendaftaran ini dimulai dari pose pertama dan akan MENIMPA sampel lama itu setelah ketiga pose selesai.</span>
                            </div>
                        </div>
                    @endif

                    <div id="kotakSelesai" class="alert alert-success d-flex flex-wrap align-items-center gap-3 mt-4 d-none">
                        <span class="fs-2">&#9989;</span>
                        <div class="flex-grow-1">
                            <span class="fw-bold d-block">Selesai. Ketiga pose sudah tersimpan.</span>
                            <span class="fs-8">Kamera dimatikan. Siswa ini sudah bisa dipindai di Absensi Wajah.</span>
                        </div>
                        <a href="{{ route('biometric.scan') }}" class="btn btn-sm btn-primary">Ke Absensi Wajah</a>
                        <a href="{{ route('biometric.show', $student) }}" class="btn btn-sm btn-light">Detail siswa</a>
                    </div>

                    <div id="preview" class="mt-4 d-none">
                        <span class="text-muted fs-8 d-block mb-2">Foto yang akan dikirim:</span>
                        <img id="previewImg" class="rounded border border-gray-300" style="max-height: 160px;" alt="">
                    </div>
                </div>
            </div>
        </div>

        <div class="col-xl-5">
            <div class="card card-flush border border-gray-200 mb-5">
                <div class="card-body p-5">
                    <div class="fw-bold text-gray-800 mb-3">Arah kepala terdeteksi</div>
                    <div class="d-flex align-items-center gap-3 mb-3">
                        <span id="poseNow" class="badge badge-light fs-6 px-4 py-3">-</span>
                        <span class="text-muted fs-8">yaw <code id="yawNow">0.00</code></span>
                    </div>
                    {{-- Angka yaw ditampilkan bukan untuk dilihat operator sehari-hari,
                         melainkan supaya arah tandanya bisa dipastikan dalam hitungan
                         detik bila petunjuk kanan/kiri ternyata terbalik di kamera
                         tertentu. Cara memperbaikinya ada di jargon-face.js
                         (YAW_SIGN). --}}
                    <div class="progress h-8px mb-2">
                        <div id="yawBar" class="progress-bar bg-primary" style="width: 50%"></div>
                    </div>
                    <div class="d-flex justify-content-between fs-9 text-muted">
                        <span>kiri</span><span>depan</span><span>kanan</span>
                    </div>
                </div>
            </div>

            <div class="card card-flush border border-gray-200" id="kartuSampel">
                <div class="card-header pt-5"><h3 class="card-title fw-bold">Sampel Tersimpan</h3></div>
                <div class="card-body pt-3">
                    @forelse ($samples as $s)
                        <div class="d-flex align-items-center justify-content-between border-bottom border-gray-200 py-3">
                            <div>
                                <span class="badge badge-light-success fs-9">{{ $s->pose }}</span>
                                <span class="text-muted fs-9 ms-2">
                                    {{ $s->created_at->timezone(config('app.timezone'))->format('d/m/Y H:i') }}
                                </span>
                            </div>
                            @if ($s->quality_score)
                                <span class="fs-9 text-muted">mutu {{ number_format($s->quality_score, 2) }}</span>
                            @endif
                            @can('delete_face_enrollment')
                                {{-- Hapus satuan langsung dari halaman pengambilan; sebelumnya hanya
                                     ada di halaman detail siswa. --}}
                                <button type="button" title="Hapus sampel ini"
                                        class="btn btn-sm btn-icon btn-light-danger w-25px h-25px ms-2 jg-hapus-sampel"
                                        data-url="{{ route('biometric.destroy', $s) }}">&times;</button>
                            @endcan
                        </div>
                    @empty
                        <span class="text-muted fs-8">Belum ada sampel.</span>
                    @endforelse
                    <div class="separator my-3"></div>
                    <div class="fs-9 text-muted">
                        Model: <code>{{ $modelVersion }}</code> &middot; {{ $embeddingDim }} dimensi
                    </div>
                </div>
            </div>
        </div>
    </div>
@endsection

@push('scripts')
    {{--
        Ekstraksi embedding dan penentuan arah kepala TIDAK ada di halaman
        ini, melainkan di assets/js/jargon-face.js yang dipakai bersama
        halaman absensi. Kalau masing-masing punya salinannya sendiri, satu
        perubahan kecil di salah satunya membuat wajah yang sudah terdaftar
        tidak lagi dikenali — dan kegagalannya tidak tampak sebagai bug,
        hanya sebagai "sistemnya kurang akurat".
    --}}
    <script src="{{ asset('assets/vendor/face-api/face-api.min.js') }}"></script>
    <script src="{{ asset('assets/js/jargon-face.js') }}?v={{ filemtime(public_path('assets/js/jargon-face.js')) }}"></script>
    <script>
        (function () {
            'use strict';

            const MODEL_BASE  = @json(asset('assets/models/face-api'));
            const MODEL_VERSION = @json($modelVersion);
            const EMB_DIM     = {{ $embeddingDim }};
            let TARGET        = @json($berikut);        // null bila sudah lengkap
            let LENGKAP      = @json($lengkap);
            const URUTAN    = @json(array_keys($urutan));
            const SUDAH     = @json(array_values($sudah));
            const BATCH_URL = @json(route('biometric.store-batch', $student));

            const video   = document.getElementById('video');
            const canvas  = document.getElementById('canvas');
            const status  = document.getElementById('camStatus');
            const hint    = document.getElementById('bigHint');
            const guide   = document.getElementById('guide');
            const form    = document.getElementById('enrollForm');
            const poseNow = document.getElementById('poseNow');
            const yawNow  = document.getElementById('yawNow');
            const yawBar  = document.getElementById('yawBar');
            const btnMan  = document.getElementById('btnManual');
            const poseSel = document.getElementById('poseSelect');

            // Pose harus bertahan 5 frame berurutan sebelum foto diambil.
            // Satu frame yang kebetulan salah deteksi tidak boleh cukup —
            // sampel yang tertangkap pada posisi setengah menoleh akan
            // merusak pengenalan siswa itu, bukan hanya sekali.
            const streak = new JargonFace.Streak(3)   // 3 frame (~0,7 detik), dari 5: menahan pose
                                                       // menoleh lebih lama terasa melelahkan;

            let stream = null, timer = null, busy = false, dikirim = false;

            // Bacaan yaw MENTAH terakhir, untuk menyesuaikan titik nol kamera.
            const yawBuf = [];

            // Sampel DITAHAN di sini sampai ketiga pose lengkap, baru dikirim sekali.
            // Operator yang berhenti di tengah tidak meninggalkan wajah setengah
            // terdaftar: data seperti itu tampak sudah ada di dashboard tetapi tidak
            // cukup untuk mengenali siapa pun.
            const terkumpul = {};

            function setStatus(text, tone) {
                status.textContent = text;
                status.className = 'position-absolute bottom-0 start-0 end-0 p-3 fs-8 text-white bg-opacity-75 '
                    + (tone === 'error' ? 'bg-danger' : tone === 'ok' ? 'bg-success'
                       : tone === 'warn' ? 'bg-warning' : 'bg-dark');
            }

            function wanted() {
                if (LENGKAP) return null;
                for (let i = 0; i < URUTAN.length; i++) {
                    const x = URUTAN[i];
                    if (SUDAH.indexOf(x) < 0 && !terkumpul[x]) return x;
                }
                return null;
            }

            function petunjuk(p) {
                return {
                    frontal: 'Lihat lurus ke kamera',
                    right:   'Putar kepala ke KANAN Anda',
                    left:    'Putar kepala ke KIRI Anda'
                }[p] || '';
            }


            /**
             * Tunjukkan arah yang diminta: panah besar + teks.
             *
             * Video dicerminkan (scaleX(-1)) seperti kaca, sehingga sisi
             * kanan layar memang sisi kanan orang di depannya. Panah ke
             * kanan layar berarti "putar ke kanan Anda" - tidak dibalik.
             */
            function tunjukArah(target, cocok) {
                document.getElementById('arrowKanan').classList.toggle('d-none', target !== 'right');
                document.getElementById('arrowKiri').classList.toggle('d-none', target !== 'left');
                const teks = document.getElementById('arahTeks');
                if (cocok) {
                    teks.textContent = 'Tahan...';
                    teks.style.color = '#50cd89';
                } else if (target === 'frontal') {
                    teks.textContent = 'Lihat lurus ke kamera';
                    teks.style.color = '#fff';
                } else if (target === 'right') {
                    teks.textContent = 'Putar kepala ke KANAN';
                    teks.style.color = '#ffc700';
                } else if (target === 'left') {
                    teks.textContent = 'Putar kepala ke KIRI';
                    teks.style.color = '#ffc700';
                } else {
                    teks.textContent = '';
                }
            }

            function tampilkanArah(pose, yaw) {
                poseNow.textContent = JargonFace.poseLabel(pose);
                poseNow.className = 'badge fs-6 px-4 py-3 ' + (
                    pose === wanted() ? 'badge-light-success' : 'badge-light'
                );
                yawNow.textContent = yaw.toFixed(2);
                // -1..1 dipetakan ke 0..100
                const pct = Math.max(0, Math.min(100, (yaw + 1) * 50));
                yawBar.style.width = pct + '%';
            }

            async function startCamera() {

                // Mulai dari nol: bias milik kamera + wajah yang sedang dipakai.

                JargonFace.setYawBias(0);

                yawBuf.length = 0;
                dikirim = false;
                try {
                    stream = await navigator.mediaDevices.getUserMedia({
                        video: { facingMode: 'user', width: { ideal: 960 }, height: { ideal: 720 } },
                        audio: false,
                    });
                    video.srcObject = stream;
                } catch (e) {
                    setStatus('Kamera tidak dapat diakses: ' + e.message
                        + '. Pastikan izin kamera diberikan dan halaman diakses lewat HTTPS atau localhost.', 'error');
                    return;
                }

                setStatus('Memuat model wajah (~7 MB, sekali saja)...', null);
                try {
                    await JargonFace.load(MODEL_BASE);
                } catch (e) {
                    setStatus(e.message || String(e), 'error');
                    return;
                }

                btnMan.disabled = false;
                tunjukArah(wanted(), false);

                if (LENGKAP) {
                    // Dibuka saat data sudah lengkap: tidak ada yang perlu ditangkap.
                    selesai();
                    return;
                }
                setStatus('Ikuti petunjuk di layar. Foto diambil otomatis.', 'ok');
                timer = setInterval(tick, 220);
            }

            async function tick() {
                if (busy || dikirim) return;
                busy = true;
                try {
                    const r = await JargonFace.describe(video);
                    tampilkanArah(r.pose, r.yaw);

                    const target = wanted();


                    // Titik nol yang menyesuaikan diri.

                    //

                    // yawOf() mengukur posisi hidung relatif titik tengah mata, jadi kamera

                    // yang tidak tepat di depan wajah atau wajah yang tidak simetris memberi

                    // bias TETAP: "lurus" bisa terbaca 0.20, dan langkah pertama tidak pernah

                    // lolos betapapun lurusnya orangnya. Kalibrasi sekali di awal sempat

                    // dicoba, tetapi memblokir ~3 detik tanpa umpan balik dan terasa macet.

                    //

                    // Sekarang: selama langkah 'menghadap depan' belum cocok TETAPI kepala

                    // terlihat DIAM (sebaran bacaan kecil), titik nol digeser ke nilai tengah

                    // bacaan terakhir. Syarat diam itu yang menjaga kebenaran — tanpa itu,

                    // kepala yang sedang menoleh pun akan dianggap lurus.

                    if (r.yawRaw !== undefined) {

                        yawBuf.push(r.yawRaw);

                        if (yawBuf.length > 10) yawBuf.shift();

                    }

                    if (target === 'frontal' && r.pose !== 'frontal' && yawBuf.length >= 6) {

                        const urut = yawBuf.slice().sort((a, b) => a - b);

                        if (urut[urut.length - 1] - urut[0] < 0.10) {

                            JargonFace.setYawBias(urut[Math.floor(urut.length / 2)]);

                            yawBuf.length = 0;

                            streak.reset();

                            setStatus('Menyesuaikan titik nol kamera — tahan kepala sebentar...', null);

                            return;

                        }

                    }
                    const cocok = r.pose === target;
                    guide.className = 'position-absolute top-50 start-50 translate-middle border border-4 rounded-circle opacity-50 '
                        + (cocok ? 'border-success' : 'border-white');

                    tunjukArah(target, cocok);

                    if (!cocok) {
                        streak.reset();
                        hint.textContent = '';
                        setStatus('Terdeteksi ' + JargonFace.poseLabel(r.pose)
                            + ' — ' + petunjuk(target).toLowerCase() + '.', 'warn');
                        return;
                    }

                    const stabil = streak.push(r.pose);
                    if (!stabil) {
                        setStatus('Posisi benar. Tahan sebentar (' + streak.n + '/' + streak.needed + ')', 'ok');
                        return;
                    }

                    if (r.descriptor.length !== EMB_DIM) {
                        throw new Error('Dimensi model ' + r.descriptor.length
                            + ' tidak sesuai konfigurasi server (' + EMB_DIM + ')');
                    }

                    kirim(r.descriptor, target);
                } catch (e) {
                    const soft = ['no_face', 'many_faces', 'too_far'];
                    if (e && soft.indexOf(e.code) >= 0) {
                        streak.reset();
                        poseNow.textContent = '-';
                        setStatus(e.message, e.code === 'no_face' ? null : 'warn');
                    } else {
                        setStatus('Gagal: ' + (e.message || e), 'error');
                    }
                } finally {
                    busy = false;
                }
            }

            /** Simpan frame utuh sebagai arsip foto pendaftaran. */
            function grabFrame() {
                canvas.width = video.videoWidth;
                canvas.height = video.videoHeight;
                canvas.getContext('2d').drawImage(video, 0, 0);
                return canvas;
            }

            /**
             * Tangkap satu pose — DITAHAN di browser, belum dikirim.
             *
             * Pengiriman baru terjadi setelah ketiga pose terkumpul (lihat kirimSemua).
             * Sebelumnya setiap pose langsung di-POST, sehingga berhenti di tengah
             * meninggalkan 1-2 sampel yang tidak berguna untuk pengenalan.
             */
            async function kirim(descriptor, pose) {
                dikirim = true;
                if (timer) { clearInterval(timer); timer = null; }
            
                const jpeg = grabFrame().toDataURL('image/jpeg', 0.92);
                terkumpul[pose] = {
                    pose: pose,
                    image_base64: jpeg.split(',')[1],
                    embedding: descriptor,
                };
                tandaiLangkah(pose);
            
                document.getElementById('previewImg').src = jpeg;
                document.getElementById('preview').classList.remove('d-none');
                hint.textContent = '';
                tunjukArah(null, false);
            
                const sisa = URUTAN.filter(function (x) {
                    return SUDAH.indexOf(x) < 0 && !terkumpul[x];
                });
            
                if (sisa.length === 0) {
                    await kirimSemua();
                    return;
                }
            
                const jml = Object.keys(terkumpul).length;
                setStatus('Pose ' + JargonFace.poseLabel(pose) + ' terkumpul (' + jml + '/' + URUTAN.length
                          + '). Belum dikirim — lanjut: ' + petunjuk(sisa[0]) + '.', 'ok');
                lanjutkan(true);
            }
            
            /** Tandai satu kotak langkah sebagai sudah terkumpul (belum tersimpan). */
            function tandaiLangkah(pose) {
                const kotak = document.querySelector('[data-pose="' + pose + '"]');
                if (!kotak) return;
                kotak.classList.remove('bg-light', 'bg-light-primary');
                kotak.classList.add('bg-light-success');
                const ikon = kotak.querySelector('[data-ikon]');
                if (ikon) ikon.innerHTML = '&#9989;';
                const st = kotak.querySelector('[data-status]');
                if (st) st.textContent = 'terkumpul';
            }
            
            /**
             * Kirim ketiga pose sekaligus. Semua berhasil, atau tidak ada yang tersimpan:
             * pembatalan bila satu pose gagal dilakukan di sisi server (storeBatch).
             */
            async function kirimSemua() {
                setStatus('Menyimpan ketiga pose...', 'ok');
                const isi = URUTAN.map(function (x) { return terkumpul[x]; }).filter(Boolean);
                try {
                    const res = await fetch(BATCH_URL, {
                        method: 'POST',
                        headers: {
                            'Content-Type': 'application/json',
                            'Accept': 'application/json',
                            'X-Requested-With': 'XMLHttpRequest',
                            'X-CSRF-TOKEN': document.querySelector('input[name=_token]').value,
                        },
                        credentials: 'same-origin',
                        body: JSON.stringify({ model_version: MODEL_VERSION, samples: isi }),
                    });
                    const data = await res.json().catch(function () { return {}; });
                    if (!res.ok) {
                        const pesan = data.message
                            || (data.errors ? Object.values(data.errors).flat().join(' ') : 'HTTP ' + res.status);
                        setStatus('Gagal: ' + pesan, 'error');
                        // Sampel tetap ditahan supaya bisa dicoba kirim ulang tanpa mengambil
                        // ulang ketiga pose.
                        dikirim = false;
                        btnMan.disabled = false;
                        if (!timer) timer = setInterval(tick, 220);
                        return;
                    }
                    // Pop-up DITUNGGU sampai ditutup. Versi sebelumnya memanggil toastr lalu
                    // langsung memuat ulang halaman, sehingga notifikasinya hilang sebelum
                    // terbaca — operator tidak pernah melihat konfirmasi apa pun.
                    if (window.Swal) {
                        await Swal.fire({
                            title: 'Pendaftaran wajah selesai',
                            html: (data.message || 'Ketiga pose tersimpan.')
                                  + '<br><span class="text-muted fs-8">Siswa ini sudah bisa dipindai di Absensi Wajah.</span>',
                            icon: 'success',
                            confirmButtonText: 'Selesai',
                            confirmButtonColor: '#0f766e',
                            allowOutsideClick: false,
                            allowEscapeKey: false,
                        });
                    } else if (window.toastr) {
                        toastr.success(data.message || 'Ketiga pose tersimpan.');
                    }
                    // Muat ulang sekali di akhir: daftar sampel dan status lengkap datang
                    // dari server, jadi tidak perlu ditebak di JS.
                    location.href = location.pathname;
                } catch (e) {
                    setStatus('Gagal mengirim: ' + (e.message || e), 'error');
                    dikirim = false;
                    if (!timer) timer = setInterval(tick, 220);
                }
            }
            

            /** Berhenti total: kamera dimatikan dan panel Selesai ditampilkan. */

            function selesai() {

                if (timer) { clearInterval(timer); timer = null; }

                if (stream) { stream.getTracks().forEach((t) => t.stop()); stream = null; }

                dikirim = true;

                busy = false;

                btnMan.disabled = true;

                poseSel.classList.add('d-none');

                tunjukArah(null, false);

                hint.textContent = '';

                setStatus('Selesai. Ketiga pose sudah tersimpan.', 'ok');

                const box = document.getElementById('kotakSelesai');

                if (box) box.classList.remove('d-none');

            }


            /** Siapkan putaran berikutnya tanpa memuat ulang halaman. */
            function lanjutkan(sukses) {
                streak.reset();
                document.getElementById('preview').classList.add('d-none');
                document.querySelectorAll('input[name^="embedding["]').forEach((el) => el.remove());
                dikirim = false;

                if (LENGKAP) {
                    selesai();
                    return;
                } else if (sukses) {
                    setStatus('Lanjut: ' + petunjuk(wanted()) + '.', 'ok');
                }
                tunjukArah(wanted(), false);
                if (!timer) timer = setInterval(tick, 220);
            }

            // Jalan darurat bila deteksi arah tidak mau lolos — mis. kamera
            // dengan sudut ekstrem. Operator tetap bisa menyimpan, dan pose
            // yang tersimpan adalah pose yang sedang diminta.
            btnMan.addEventListener('click', async function () {
                btnMan.disabled = true;
                try {
                    const r = await JargonFace.describe(video);
                    kirim(r.descriptor, wanted());
                } catch (e) {
                    setStatus('Gagal: ' + (e.message || e), 'error');
                    btnMan.disabled = false;
                }
            });

            /**

             * Konfirmasi tindakan yang merusak.

             *

             * Memakai SweetAlert2 yang sudah dimuat tema — confirm() bawaan browser

             * mudah terlewat, dan di sini yang dikonfirmasi adalah penghapusan data

             * biometrik yang tidak bisa dibatalkan. confirm() tetap dipakai sebagai

             * cadangan bila pustakanya tidak ada.

             */

            async function konfirmasi(judul, teks, tombol) {

                if (window.Swal) {

                    const r = await Swal.fire({

                        title: judul,

                        text: teks,

                        icon: 'warning',

                        showCancelButton: true,

                        confirmButtonText: tombol,

                        cancelButtonText: 'Batal',

                        confirmButtonColor: '#dc2626',

                        reverseButtons: true,

                    });

                    return r.isConfirmed === true;

                }

                return confirm(judul + '\n\n' + teks);

            }


            const btnResetEl = document.getElementById('btnReset');
            // Tombol hanya dirender untuk pemilik izin delete_face_enrollment,
            // jadi ketiadaannya normal - bukan galat.
            if (btnResetEl) btnResetEl.addEventListener('click', async function (ev) {

                const btn = ev.currentTarget;

                const setuju = await konfirmasi(
                    'Ulangi dari awal?',
                    'Semua sampel wajah siswa ini akan DIHAPUS dan pendaftaran dimulai dari pose pertama. Sampel lama ditimpa oleh pendaftaran baru, dan tindakan ini tidak bisa dibatalkan.',
                    'Ya, hapus & mulai ulang'
                );
                if (!setuju) return;

                btn.disabled = true;

                setStatus('Menghapus sampel...', null);

                try {

                    const res = await fetch(btn.dataset.url, {

                        method: 'DELETE',

                        headers: {

                            'Accept': 'application/json',

                            'X-Requested-With': 'XMLHttpRequest',

                            'X-CSRF-TOKEN': document.querySelector('input[name=_token]').value,

                        },

                        credentials: 'same-origin',

                    });

                    const data = await res.json().catch(() => ({}));

                    if (!res.ok) {

                        setStatus('Gagal: ' + (data.message || res.status), 'error');

                        btn.disabled = false;

                        return;

                    }

                    // Muat ulang DI SINI memang yang diinginkan: state pendaftaran

                    // harus benar-benar kembali ke pose pertama.

                    if (window.Swal) {
                        await Swal.fire({
                            title: 'Sampel dihapus',
                            text: data.message || 'Pendaftaran dimulai dari pose pertama.',
                            icon: 'success',
                            confirmButtonText: 'Mulai',
                            confirmButtonColor: '#0f766e',
                        });
                    }

                    location.href = location.pathname;

                } catch (e) {

                    setStatus('Gagal menghapus: ' + (e.message || e), 'error');

                    btn.disabled = false;

                }

            });


            // Hapus satu sampel. Setelah terhapus halaman dimuat ulang: keadaan
            // pose berikutnya ditentukan server, dan sampel yang hilang bisa
            // membuat pendaftaran kembali belum lengkap.
            document.addEventListener('click', async function (ev) {
                const b = ev.target.closest('.jg-hapus-sampel');
                if (!b) return;
                const yakin = await konfirmasi(
                    'Hapus sampel ini?',
                    'Sampel wajah ini dihapus permanen beserta gambarnya. Bila setelah ini pose menjadi belum lengkap, pengambilan dilanjutkan dari pose itu.',
                    'Ya, hapus'
                );
                if (!yakin) return;
                b.disabled = true;
                try {
                    const res = await fetch(b.dataset.url, {
                        method: 'DELETE',
                        headers: {
                            'Accept': 'application/json',
                            'X-Requested-With': 'XMLHttpRequest',
                            'X-CSRF-TOKEN': document.querySelector('input[name=_token]').value,
                        },
                        credentials: 'same-origin',
                    });
                    if (!res.ok) {
                        const d = await res.json().catch(() => ({}));
                        setStatus('Gagal: ' + (d.message || res.status), 'error');
                        b.disabled = false;
                        return;
                    }
                    location.href = location.pathname;
                } catch (e) {
                    setStatus('Gagal menghapus: ' + (e.message || e), 'error');
                    b.disabled = false;
                }
            });

            document.getElementById('btnRetry').addEventListener('click', function () {
                if (timer) { clearInterval(timer); timer = null; }
                if (stream) stream.getTracks().forEach((t) => t.stop());
                streak.reset();
                startCamera();
            });

            if (poseSel) {
                poseSel.addEventListener('change', function () {
                    streak.reset();
                    tunjukArah(wanted(), false);
                });
            }

            // Kamera dilepas saat halaman ditinggalkan supaya lampu indikator
            // tidak tetap menyala di perangkat sekolah.
            window.addEventListener('pagehide', function () {
                if (timer) clearInterval(timer);
                if (stream) stream.getTracks().forEach((t) => t.stop());
            });

            startCamera();
        })();
    </script>
@endpush
