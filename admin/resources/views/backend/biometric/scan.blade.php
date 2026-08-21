@extends('backend.layout.app')
@section('title', 'Absensi Wajah')

@section('content')

    {{-- Mode kios: halaman ini dipakai di gerbang dan dipandang dari jarak satu
         meter. Navbar, sidebar, dan footer tidak ada gunanya di sana — hanya
         memberi jalan salah-klik ke halaman lain saat tablet ditinggal tanpa
         penjaga. Disembunyikan lewat CSS, bukan layout terpisah, supaya seluruh
         aset dan JS layout (Swal, toastr, face-api) tetap termuat apa adanya. --}}
    @push('stylesheets')
        <style>
            #kt_header,
            #kt_app_sidebar,
            #kt_sidebar_overlay,
            .footer,
            #kt_footer { display: none !important; }

            #kt_content_container { padding: 0 !important; }
            .content { padding: 10px !important; }
            .header-fixed[data-kt-sticky-header="on"] .wrapper { padding-top: 0 !important; }

            /* Jalan keluar tetap ada, tetapi dibuat samar supaya tidak menarik
               tangan siswa yang sedang mengantre. */
            .jg-kios-keluar {
                position: fixed;
                right: 12px;
                bottom: 12px;
                z-index: 1050;
                opacity: 0.3;
                transition: opacity 0.15s ease;
            }
            .jg-kios-keluar:hover,
            .jg-kios-keluar:focus { opacity: 1; }
        </style>
    @endpush

    <a href="{{ route('dashboard') }}" class="btn btn-sm btn-light jg-kios-keluar">
        Keluar mode kios
    </a>
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

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-5">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Absensi Wajah</h2>
            <span class="text-muted fs-7">
                Cukup hadap lurus ke kamera. Kehadiran tercatat sendiri.
            </span>
        </div>
        <a href="{{ route('biometric.index') }}" class="btn btn-sm btn-light">Pendaftaran Wajah</a>
    </div>

    {{--
        Halaman ini bekerja sebagai PERANGKAT KIOS, sama seperti tablet.

        Bukan lewat endpoint khusus dashboard: ia memanggil
        POST /v1/kiosk/recognize yang sudah ada, dengan device token dari
        pairing. Konsekuensinya seluruh aturan yang sudah teruji ikut
        berlaku tanpa ditulis dua kali - jendela jam masuk/pulang, jeda
        antar-scan, ambang kemiripan, margin kembar, anti-replay nonce,
        pencatatan device_id, dan notifikasi wali murid.
    --}}

    <div id="pairPanel" class="card card-flush border border-gray-200 mb-5 d-none">
        <div class="card-header pt-5">
            <h3 class="card-title fw-bold">Pasangkan Komputer Ini</h3>
        </div>
        <div class="card-body pt-3">
            <div class="alert alert-light-info py-3 px-4 fs-8 mb-4">
                Komputer ini belum terdaftar sebagai perangkat absensi. Buat
                perangkat di <a href="{{ route('devices.index') }}">Perangkat Tablet</a>,
                lalu masukkan <strong>kode pairing 8 digit</strong>-nya di bawah.
                Cukup sekali; setelah itu komputer ini langsung siap absen.
            </div>
            <div class="row g-3 align-items-end">
                <div class="col-md-4">
                    <label class="form-label required">Kode Pairing</label>
                    <input type="text" id="pairCode" class="form-control"
                           inputmode="numeric" maxlength="8" placeholder="12345678"
                           autocomplete="off">
                </div>
                <div class="col-md-3">
                    <button id="btnPair" class="btn btn-primary w-100">Pasangkan</button>
                </div>
            </div>
            <div id="pairError" class="text-danger fs-8 mt-3 d-none"></div>
        </div>
    </div>

    <div id="scanPanel" class="row g-5 d-none">
        <div class="col-xl-7">
            <div class="card card-flush border border-gray-200">
                <div class="card-body p-0 position-relative bg-dark rounded overflow-hidden">
                    {{-- Dicerminkan seperti kaca, supaya "hadap kanan" terasa sesuai
                         dengan yang dilihat di layar. Perhitungan yaw tetap memakai
                         piksel asli, bukan tampilan ini. --}}
                    <video id="video" autoplay muted playsinline
                           class="w-100" style="max-height: 520px; object-fit: cover; transform: scaleX(-1);"></video>

                    <div id="guide"
                         class="position-absolute top-50 start-50 translate-middle border border-4 rounded-circle border-white opacity-50"
                         style="width: 40%; aspect-ratio: 1; pointer-events: none; transition: border-color .2s;"></div>

                    <div id="bigHint"
                         class="position-absolute top-0 start-0 end-0 text-center text-white fw-bold pt-4"
                         style="font-size: 2rem; text-shadow: 0 2px 10px rgba(0,0,0,.85); pointer-events: none;"></div>
                    <div id="arrowKiri" class="arah-panah kiri d-none">&#11013;</div>
                    <div id="arrowKanan" class="arah-panah kanan d-none">&#10145;</div>
                    <div id="arahTeks" class="arah-teks"></div>

                    <div id="camStatus"
                         class="position-absolute bottom-0 start-0 end-0 p-3 fs-8 text-white bg-dark bg-opacity-75">
                        Menyiapkan kamera...
                    </div>
                </div>
            </div>

            {{-- Tiga langkah, terlihat sepanjang waktu supaya siswa tahu
                 sedang di mana tanpa perlu dijelaskan petugas. --}}
            <div class="d-flex gap-3 mt-4 d-none">{{-- tiga langkah tidak dipakai lagi --}}
                <div id="stepDepan" class="flex-grow-1 rounded p-3 bg-light text-center">
                    <span class="fs-3 d-block">&#128100;</span>
                    <span class="fs-8 fw-bold">1. Hadap depan</span>
                </div>
                <div id="stepToleh" class="flex-grow-1 rounded p-3 bg-light text-center">
                    <span class="fs-3 d-block" id="stepTolehIcon">&#8596;</span>
                    <span class="fs-8 fw-bold" id="stepTolehLabel">2. Menoleh</span>
                </div>
                <div id="stepBalik" class="flex-grow-1 rounded p-3 bg-light text-center">
                    <span class="fs-3 d-block">&#9989;</span>
                    <span class="fs-8 fw-bold">3. Hadap depan lagi</span>
                </div>
            </div>

            <div class="d-flex flex-wrap gap-2 mt-4">
                <button id="btnStart" class="btn btn-primary" disabled>Mulai Absensi</button>
                <button id="btnStop" class="btn btn-light-danger d-none">Hentikan</button>
                <button id="btnUnpair" class="btn btn-light ms-auto">Lepas Perangkat</button>
            </div>
        </div>

        <div class="col-xl-5">
            {{-- Hasil terakhir dibuat BESAR: dibaca dari jarak satu meter
                 sambil siswa berikutnya sudah mengantre. --}}
            <div class="card card-flush border border-gray-200 mb-5">
                <div class="card-body p-5 text-center">
                    <div id="resultIcon" class="fs-3x mb-3">&#128100;</div>
                    <div id="resultName" class="fs-2 fw-bold text-gray-900">Menunggu</div>
                    <div id="resultMeta" class="fs-7 text-muted mt-2">Tekan "Mulai Absensi"</div>
                    <div id="resultBadge" class="mt-3"></div>
                </div>
            </div>

            <div class="card card-flush border border-gray-200 mb-5">
                <div class="card-body p-5">
                    <div class="fw-bold text-gray-800 mb-3">Arah kepala terdeteksi</div>
                    <div class="d-flex align-items-center gap-3 mb-3">
                        <span id="poseNow" class="badge badge-light fs-6 px-4 py-3">-</span>
                        <span class="text-muted fs-8">yaw <code id="yawNow">0.00</code></span>
                    </div>
                    <div class="progress h-8px mb-2">
                        <div id="yawBar" class="progress-bar bg-primary" style="width: 50%"></div>
                    </div>
                    <div class="d-flex justify-content-between fs-9 text-muted">
                        <span>kiri</span><span>depan</span><span>kanan</span>
                    </div>
                </div>
            </div>

            <div class="card card-flush border border-gray-200">
                <div class="card-header pt-5">
                    <h3 class="card-title fw-bold">Riwayat Sesi Ini</h3>
                    <div class="card-toolbar">
                        <span id="counter" class="badge badge-light">0 tercatat</span>
                    </div>
                </div>
                <div class="card-body pt-3 p-0">
                    <div class="table-responsive" style="max-height: 300px; overflow-y:auto;">
                        <table class="table table-row-bordered align-middle mb-0">
                            <tbody id="logBody">
                                <tr><td class="text-center text-muted py-8 fs-8">Belum ada scan.</td></tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>

            <div class="card card-flush border border-gray-200 mt-5">
                <div class="card-body p-5 fs-9 text-muted">
                    <div class="fw-bold text-gray-700 mb-2">Perangkat</div>
                    <div id="deviceInfo">-</div>
                    <div class="separator my-3"></div>
                    Model: <code>{{ $modelVersion }}</code> &middot; {{ $embeddingDim }} dimensi
                </div>
            </div>
        </div>
    </div>
@endsection

@push('scripts')
    <script src="{{ asset('assets/vendor/face-api/face-api.min.js') }}"></script>
    <script src="{{ asset('assets/js/jargon-face.js') }}?v={{ filemtime(public_path('assets/js/jargon-face.js')) }}"></script>
    <script>
    (function () {
        'use strict';

        const API        = @json($apiBase);
        const MODEL_BASE = @json(asset('assets/models/face-api'));
        const MODEL_VER  = @json($modelVersion);
        const EMB_DIM    = {{ $embeddingDim }};
        const STORE_KEY  = 'jargon.kiosk.device';

        const SCAN_INTERVAL_MS = 220;

        /**
         * Arah toleh DITENTUKAN SISTEM secara acak, bukan dipilih siswa.
         *
         * Itu yang membuat langkah ini berarti sebagai uji kehidupan:
         * tantangan yang bisa diramalkan dapat disiapkan lebih dulu — satu
         * video pendek berisi wajah menoleh ke kanan sudah cukup untuk
         * melewatinya setiap kali. Arah yang diundi pada saat itu tidak
         * bisa disiapkan.
         *
         * Setel false bila ingin menerima arah mana pun (lebih longgar,
         * lebih cepat untuk siswa, tetapi lebih mudah ditipu rekaman).
         */
        const TANTANGAN_ACAK = true;

        /**
         * Skor liveness yang dilaporkan setelah tantangan lolos.
         *
         * 0.9, bukan 1.0: gerak kepala membuktikan ini bukan foto cetak,
         * tetapi bukan bukti mutlak — rekaman video wajah menoleh masih
         * bisa menipu. Melaporkan 1.0 akan berbohong kepada server dan
         * membuat FACE_MIN_LIVENESS tidak berarti sebagai pengaman.
         */
        const LIVENESS_LULUS = 0.9;

        const el = (id) => document.getElementById(id);
        let device = null, stream = null, timer = null, busy = false, count = 0;
        let lastKey = '';

        // --- Mesin keadaan tantangan ---
        // depan -> toleh -> balik -> (kirim) -> tunggu_kosong -> depan
        let fase = 'depan';

        // Bacaan yaw MENTAH terakhir, untuk menyesuaikan titik nol kamera.
        // Halaman pendaftaran sudah memakai ini; halaman absensi belum, itulah
        // sebabnya "hadap depan" di sini terasa jauh lebih susah: wajah lurus
        // terbaca mis. 0.16 dan langsung dianggap menoleh.
        const yawBuf = [];
        let arah = null;                       // 'right' | 'left'
        const streak = new JargonFace.Streak(3);

        function setStatus(text, tone) {
            el('camStatus').textContent = text;
            el('camStatus').className =
                'position-absolute bottom-0 start-0 end-0 p-3 fs-8 text-white bg-opacity-75 '
                + (tone === 'error' ? 'bg-danger' : tone === 'ok' ? 'bg-success'
                   : tone === 'warn' ? 'bg-warning' : 'bg-dark');
        }

        function pilihArah() {
            if (!TANTANGAN_ACAK) return 'right';
            const b = new Uint8Array(1);
            (window.crypto || window.msCrypto).getRandomValues(b);
            return (b[0] & 1) ? 'right' : 'left';
        }

        function labelArah(a) { return a === 'right' ? 'KANAN' : 'KIRI'; }

        /**
         * Tunjukkan arah yang diminta: panah besar + teks.
         *
         * Video dicerminkan (scaleX(-1)) seperti kaca, sehingga sisi kanan
         * layar memang sisi kanan orang yang berdiri di depannya. Panah ke
         * kanan layar karena itu berarti "menoleh ke kanan Anda" - tidak
         * perlu dibalik.
         */
        function tunjukArah(target, cocok) {
            const kanan = el('arrowKanan');
            const kiri  = el('arrowKiri');
            const teks  = el('arahTeks');

            kanan.classList.toggle('d-none', target !== 'right');
            kiri.classList.toggle('d-none', target !== 'left');

            if (cocok) {
                teks.textContent = 'Tahan...';
                teks.style.color = '#50cd89';
            } else if (target === 'frontal') {
                teks.textContent = 'Hadap lurus ke kamera';
                teks.style.color = '#fff';
            } else if (target === 'right') {
                teks.textContent = 'Menoleh ke KANAN';
                teks.style.color = '#ffc700';
            } else if (target === 'left') {
                teks.textContent = 'Menoleh ke KIRI';
                teks.style.color = '#ffc700';
            } else {
                teks.textContent = '';
            }
        }

        function gambarLangkah() {
            const aktif = 'flex-grow-1 rounded p-3 text-center bg-light-primary';
            const beres = 'flex-grow-1 rounded p-3 text-center bg-light-success';
            const mati  = 'flex-grow-1 rounded p-3 text-center bg-light';

            el('stepDepan').className = fase === 'depan' ? aktif : beres;
            el('stepToleh').className = fase === 'toleh' ? aktif : (fase === 'depan' ? mati : beres);
            el('stepBalik').className = fase === 'balik' ? aktif : mati;

            if (arah) {
                el('stepTolehIcon').innerHTML = arah === 'right' ? '&#10145;' : '&#11013;';
                el('stepTolehLabel').textContent = '2. Menoleh ke ' + labelArah(arah).toLowerCase();
            } else {
                el('stepTolehIcon').innerHTML = '&#8596;';
                el('stepTolehLabel').textContent = '2. Menoleh';
            }
        }

        function mulaiTantangan() {
            fase = 'depan';
            arah = pilihArah();
            streak.reset();
            gambarLangkah();
        }

        function tampilkanArah(pose, yaw, target) {
            el('poseNow').textContent = JargonFace.poseLabel(pose);
            el('poseNow').className = 'badge fs-6 px-4 py-3 '
                + (pose === target ? 'badge-light-success' : 'badge-light');
            el('yawNow').textContent = yaw.toFixed(2);
            el('yawBar').style.width = Math.max(0, Math.min(100, (yaw + 1) * 50)) + '%';
        }

        // ---------------- Pairing ----------------

        function loadDevice() {
            try { return JSON.parse(localStorage.getItem(STORE_KEY) || 'null'); }
            catch (e) { return null; }
        }

        function showPanels() {
            const paired = !!(device && device.device_token);
            el('pairPanel').classList.toggle('d-none', paired);
            el('scanPanel').classList.toggle('d-none', !paired);
            if (paired) {
                el('deviceInfo').innerHTML =
                    '<strong>' + device.device_name + '</strong> (' + device.device_code + ')<br>'
                    + device.school_name
                    + (device.classroom_name ? ' &middot; ' + device.classroom_name : '')
                    + '<br>mode: ' + device.mode;
            }
        }

        el('btnPair').addEventListener('click', async function () {
            const code = el('pairCode').value.trim();
            el('pairError').classList.add('d-none');

            if (!/^[0-9]{8}$/.test(code)) {
                el('pairError').textContent = 'Kode pairing harus 8 digit angka.';
                el('pairError').classList.remove('d-none');
                return;
            }

            el('btnPair').disabled = true;
            try {
                const res = await fetch(API + '/v1/devices/pair', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json', 'Accept': 'application/json' },
                    body: JSON.stringify({
                        pairing_code: code,
                        app_version: 'web-admin',
                        os_version: navigator.userAgent.slice(0, 60),
                        hardware_id: 'browser-' + (navigator.userAgent.length * 7919)
                    })
                });
                const json = await res.json();
                if (!res.ok) throw new Error(json.message || ('Gagal (kode ' + res.status + ')'));

                device = json.data;
                localStorage.setItem(STORE_KEY, JSON.stringify(device));
                showPanels();
                boot();
            } catch (e) {
                el('pairError').textContent = e.message || String(e);
                el('pairError').classList.remove('d-none');
            } finally {
                el('btnPair').disabled = false;
            }
        });

        el('btnUnpair').addEventListener('click', function () {
            if (!confirm('Lepas perangkat ini? Perlu kode pairing baru untuk memakainya lagi.')) return;
            stop();
            localStorage.removeItem(STORE_KEY);
            device = null;
            showPanels();
        });

        // ---------------- Kamera + model ----------------

        async function boot() {
            try {
                stream = await navigator.mediaDevices.getUserMedia({
                    video: { facingMode: 'user', width: { ideal: 960 }, height: { ideal: 720 } },
                    audio: false
                });
                el('video').srcObject = stream;
            } catch (e) {
                setStatus('Kamera tidak dapat diakses: ' + e.message
                    + '. Halaman harus dibuka lewat HTTPS atau localhost.', 'error');
                return;
            }

            setStatus('Memuat model wajah (~7 MB, sekali saja)...', null);
            try {
                await JargonFace.load(MODEL_BASE);
                el('btnStart').disabled = false;
                setStatus('Siap. Tekan "Mulai Absensi".', 'ok');
            } catch (e) {
                setStatus(e.message || String(e), 'error');
            }
        }

        // ---------------- Loop ----------------

        el('btnStart').addEventListener('click', function () {
            el('btnStart').classList.add('d-none');
            el('btnStop').classList.remove('d-none');
            mulaiTantangan();
            setStatus('Berjalan. Berdiri di depan kamera.', 'ok');
            JargonFace.setYawBias(0);
            yawBuf.length = 0;
            timer = setInterval(tick, SCAN_INTERVAL_MS);
        });

        el('btnStop').addEventListener('click', stop);

        function stop() {
            if (timer) { clearInterval(timer); timer = null; }
            el('btnStop').classList.add('d-none');
            el('btnStart').classList.remove('d-none');
            el('bigHint').textContent = '';
            tunjukArah(null, false);
            setStatus('Dihentikan.', 'warn');
        }

        function targetFase() {
            // Absensi cukup SEKALI hadap depan. Tantangan tiga langkah
            // (depan - menoleh - depan lagi) dilepas atas permintaan: di lapangan
            // antrean pagi jadi lambat dan siswa bingung.
            //
            // PERHATIAN: tantangan itu satu-satunya penahan foto cetak/layar ponsel
            // (foto tidak bisa menoleh). Tanpa itu, liveness tinggal deteksi kedip
            // pasif di face_engine tablet — untuk produksi sebaiknya dihidupkan lagi
            // atau diganti model anti-spoof.
            return fase === 'tunggu_kosong' ? null : 'frontal';
        }

        async function tick() {
            if (busy) return;
            busy = true;
            try {
                const r = await JargonFace.describe(el('video'));

                // Menunggu orang sebelumnya menyingkir sebelum tantangan
                // baru dimulai. Tanpa ini, satu orang yang tetap berdiri di
                // depan kamera akan diminta menoleh terus-menerus.
                if (fase === 'tunggu_kosong') {
                    tampilkanArah(r.pose, r.yaw, null);
                    tunjukArah(null, false);
                    el('bigHint').textContent = '';
                    setStatus('Selesai. Silakan bergeser, siswa berikutnya maju.', 'ok');
                    return;
                }

                const target = targetFase();
                tampilkanArah(r.pose, r.yaw, target);

                // Geser titik nol ke posisi kepala yang sedang DIAM bila 'hadap depan'

                // belum juga cocok. Syarat diam menjaga kebenaran: kepala yang sedang

                // menoleh tidak boleh ikut dianggap lurus.

                if (r.yawRaw !== undefined) {

                    yawBuf.push(r.yawRaw);

                    if (yawBuf.length > 10) yawBuf.shift();

                }

                if (target === 'frontal' && r.pose !== 'frontal' && yawBuf.length >= 6) {

                    const urut = yawBuf.slice().sort(function (a, b) { return a - b; });

                    if (urut[urut.length - 1] - urut[0] < 0.10) {

                        JargonFace.setYawBias(urut[Math.floor(urut.length / 2)]);

                        yawBuf.length = 0;

                        streak.reset();

                        setStatus('Menyesuaikan titik nol kamera — tahan kepala sebentar...', null);

                        return;

                    }

                }


                const cocok = r.pose === target;
                el('guide').className =
                    'position-absolute top-50 start-50 translate-middle border border-4 rounded-circle opacity-50 '
                    + (cocok ? 'border-success' : 'border-white');

                tunjukArah(target, cocok);

                if (!cocok) {
                    streak.reset();
                    el('bigHint').textContent = 'Hadap lurus ke kamera';
                    setStatus('Terdeteksi ' + JargonFace.poseLabel(r.pose) + '.', 'warn');
                    return;
                }

                if (!streak.push(r.pose)) return;

                // fase === 'balik' dan sudah stabil menghadap depan.
                //
                // Embedding yang DIKIRIM diambil dari frame frontal ini —
                // bukan dari frame saat menoleh. Sampel frontal paling
                // sebanding dengan pendaftaran, dan wajah menoleh punya
                // embedding yang jauh berbeda.
                if (r.descriptor.length !== EMB_DIM) {
                    setStatus('Dimensi model (' + r.descriptor.length + ') tidak sesuai server ('
                        + EMB_DIM + ').', 'error');
                    stop();
                    return;
                }

                await send(r.descriptor);
                fase = 'tunggu_kosong';
                streak.reset();
                gambarLangkah();
            } catch (e) {
                const soft = ['no_face', 'many_faces', 'too_far'];
                if (e && soft.indexOf(e.code) >= 0) {
                    streak.reset();
                    el('poseNow').textContent = '-';

                    // Wajah menghilang = orang berikutnya. Tantangan diundi
                    // ulang, sehingga arahnya tidak sama untuk semua orang.
                    if (fase === 'tunggu_kosong' || fase !== 'depan') {
                        mulaiTantangan();
                    }
                    tunjukArah('frontal', false);
                    el('bigHint').textContent = '';
                    setStatus(e.message, e.code === 'no_face' ? null : 'warn');
                } else {
                    setStatus('Galat: ' + (e.message || e), 'error');
                }
            } finally {
                busy = false;
            }
        }

        async function send(descriptor) {
            setStatus('Mengirim...', null);
            const res = await fetch(API + '/v1/kiosk/recognize', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'Accept': 'application/json',
                    'Authorization': 'Device ' + device.device_token
                },
                body: JSON.stringify({
                    embedding: descriptor,
                    model_version: MODEL_VER,
                    liveness_score: LIVENESS_LULUS,
                    client_time: new Date().toISOString(),
                    nonce: JargonFace.nonce(),
                    classroom_id: device.classroom_id || null
                })
            });

            let json = null;
            try { json = await res.json(); } catch (e) { /* bukan JSON */ }

            if (res.status === 401) {
                setStatus('Perangkat tidak dikenal lagi. Pasangkan ulang.', 'error');
                stop();
                localStorage.removeItem(STORE_KEY);
                device = null;
                showPanels();
                return;
            }
            if (!res.ok) {
                setStatus((json && json.message) || ('Server menolak (kode ' + res.status + ')'), 'error');
                return;
            }

            render((json && json.data) || {}, (json && json.message) || '');
        }

        // ---------------- Tampilan hasil ----------------

        /**

         * Pop-up hasil absensi.

         *

         * Panel di samping saja tidak cukup: siswa berdiri di depan kamera dan

         * tidak selalu melihat ke sisi layar, sementara petugas perlu bukti yang

         * tidak bisa terlewat bahwa absensi BENAR tercatat dan untuk siapa.

         *

         * Yang berhasil menutup sendiri setelah 4 detik supaya antrean tidak

         * berhenti; yang gagal menunggu ditutup, karena perlu tindakan.

         */

        function popupHasil(matched, s, a, message) {

            if (!window.Swal) return;

        

            const baris = [];

            if (s) {

                const id = s.nisn || s.nis;

                if (id) baris.push('NISN/NIS: <b>' + id + '</b>');

                if (s.classroom_name) baris.push('Kelas: <b>' + s.classroom_name + '</b>');

                if (s.school_name) baris.push(s.school_name);

            }

            if (a) {

                if (a.status) baris.push('Status: <b>' + a.status + '</b>');

                if (a.late_minutes) baris.push('Terlambat ' + a.late_minutes + ' menit');

                const jam = a.check_out_at || a.check_in_at;

                if (jam) {

                    baris.push('Waktu: <b>'

                        + new Date(jam).toLocaleTimeString('id-ID', { hour: '2-digit', minute: '2-digit' })

                        + '</b>');

                }

            }

        

            Swal.fire({

                title: matched ? (s ? s.full_name : 'Absensi tercatat') : 'Tidak dikenali',

                html: (message ? '<div class="mb-3">' + message + '</div>' : '')

                      + (baris.length ? '<div class="text-start fs-7 text-muted">' + baris.join('<br>') + '</div>' : ''),

                icon: matched ? 'success' : 'error',

                confirmButtonText: matched ? 'Berikutnya' : 'Tutup',

                confirmButtonColor: matched ? '#0f766e' : '#dc2626',

                timer: matched ? 4000 : undefined,

                timerProgressBar: matched,

            });

        }


        function render(data, message) {
            const matched = !!data.matched;
            const s = data.student || null;

            el('resultIcon').innerHTML = matched ? '&#9989;' : '&#10060;';
            el('resultName').textContent = s ? s.full_name : 'Tidak dikenali';
            el('resultName').className = 'fs-2 fw-bold ' + (matched ? 'text-success' : 'text-danger');
            el('resultMeta').textContent = message || '';

            let badge = '';
            if (data.attendance) {
                const a = data.attendance;
                badge = '<span class="badge badge-light-primary">' + (a.status || '') + '</span>';
                if (a.late_minutes > 0) {
                    badge += ' <span class="badge badge-light-warning">terlambat '
                          + a.late_minutes + ' menit</span>';
                }
            }
            if (typeof data.similarity === 'number') {
                badge += ' <span class="badge badge-light">skor ' + data.similarity.toFixed(3) + '</span>';
            }
            el('resultBadge').innerHTML = badge;

            setStatus(message || (matched ? 'Tercatat.' : 'Tidak dikenali.'),
                      matched ? 'ok' : 'warn');


            popupHasil(matched, s, data.attendance || null, message);

            const key = (s ? s.id : 'unknown') + '|' + (data.action || '');
            if (key === lastKey) return;
            lastKey = key;

            if (matched) { count += 1; el('counter').textContent = count + ' tercatat'; }

            const body = el('logBody');
            if (body.dataset.filled !== '1') { body.innerHTML = ''; body.dataset.filled = '1'; }

            const tr = document.createElement('tr');
            const cell = document.createElement('td');
            cell.className = 'ps-4 py-3';

            const nm = document.createElement('span');
            nm.className = 'fw-semibold fs-7 d-block';
            nm.textContent = s ? s.full_name : 'Tidak dikenali';

            const meta = document.createElement('span');
            meta.className = 'text-muted fs-9';
            meta.textContent = (s && s.classroom_name ? s.classroom_name + ' - ' : '')
                + new Date().toLocaleTimeString('id-ID')
                + ' - toleh ' + labelArah(arah).toLowerCase()
                + (message ? ' - ' + message : '');

            cell.append(nm, meta);
            tr.append(cell);
            body.prepend(tr);
        }

        window.addEventListener('pagehide', function () {
            if (timer) clearInterval(timer);
            if (stream) stream.getTracks().forEach((t) => t.stop());
        });

        device = loadDevice();
        showPanels();
        gambarLangkah();
        if (device) boot();
    })();
    </script>
@endpush
