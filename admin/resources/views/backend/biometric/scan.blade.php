@extends('backend.layout.app')
@section('title', 'Absensi Wajah')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-5">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Absensi Wajah</h2>
            <span class="text-muted fs-7">
                Siswa berdiri di depan kamera; kehadiran tercatat sendiri.
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

        Menambah endpoint sendiri untuk web berarti menyalin semua itu, dan
        salinan yang menyimpang menghasilkan absensi yang berbeda hanya
        karena alatnya berbeda.
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
                <div class="card-body p-0 position-relative bg-dark rounded">
                    <video id="video" autoplay muted playsinline
                           class="w-100 rounded" style="max-height: 520px; object-fit: cover;"></video>
                    <div id="camStatus"
                         class="position-absolute bottom-0 start-0 end-0 p-3 fs-8 text-white bg-dark bg-opacity-75">
                        Menyiapkan kamera...
                    </div>
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

            <div class="card card-flush border border-gray-200">
                <div class="card-header pt-5">
                    <h3 class="card-title fw-bold">Riwayat Sesi Ini</h3>
                    <div class="card-toolbar">
                        <span id="counter" class="badge badge-light">0 tercatat</span>
                    </div>
                </div>
                <div class="card-body pt-3 p-0">
                    <div class="table-responsive" style="max-height: 340px; overflow-y:auto;">
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
    <script src="{{ asset('assets/js/jargon-face.js') }}"></script>
    <script>
    (function () {
        'use strict';

        const API        = @json($apiBase);
        const MODEL_BASE = @json(asset('assets/models/face-api'));
        const MODEL_VER  = @json($modelVersion);
        const EMB_DIM    = {{ $embeddingDim }};
        const STORE_KEY  = 'jargon.kiosk.device';

        // Jeda antar-scan di SISI KLIEN. Server punya jeda sendiri
        // (FACE_SCAN_COOLDOWN_SECS); yang ini semata agar satu orang yang
        // berdiri diam tidak menembak server berkali-kali per detik.
        const SCAN_INTERVAL_MS = 900;

        const el = (id) => document.getElementById(id);
        let device = null, stream = null, timer = null, busy = false, count = 0;
        let lastKey = '';

        function setStatus(text, tone) {
            el('camStatus').textContent = text;
            el('camStatus').className =
                'position-absolute bottom-0 start-0 end-0 p-3 fs-8 text-white bg-opacity-75 '
                + (tone === 'error' ? 'bg-danger' : tone === 'ok' ? 'bg-success'
                   : tone === 'warn' ? 'bg-warning' : 'bg-dark');
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

        // ---------------- Loop absensi ----------------

        el('btnStart').addEventListener('click', function () {
            el('btnStart').classList.add('d-none');
            el('btnStop').classList.remove('d-none');
            setStatus('Berjalan. Silakan berdiri di depan kamera.', 'ok');
            timer = setInterval(tick, SCAN_INTERVAL_MS);
        });

        el('btnStop').addEventListener('click', stop);

        function stop() {
            if (timer) { clearInterval(timer); timer = null; }
            el('btnStop').classList.add('d-none');
            el('btnStart').classList.remove('d-none');
            setStatus('Dihentikan.', 'warn');
        }

        async function tick() {
            if (busy) return;
            busy = true;
            try {
                const r = await JargonFace.describe(el('video'));

                if (r.descriptor.length !== EMB_DIM) {
                    setStatus('Dimensi model (' + r.descriptor.length + ') tidak sesuai server ('
                        + EMB_DIM + ').', 'error');
                    stop();
                    return;
                }

                await send(r.descriptor);
            } catch (e) {
                // Galat deteksi bukan kegagalan - itu keadaan normal saat
                // tidak ada orang di depan kamera. Ditampilkan sebagai
                // petunjuk, bukan sebagai error.
                const soft = ['no_face', 'many_faces', 'too_far'];
                if (e && soft.indexOf(e.code) >= 0) {
                    setStatus(e.message, e.code === 'no_face' ? null : 'warn');
                } else {
                    setStatus('Galat: ' + (e.message || e), 'error');
                }
            } finally {
                busy = false;
            }
        }

        async function send(descriptor) {
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
                    // Liveness dari browser TIDAK diklaim tinggi.
                    //
                    // Halaman ini tidak memeriksa kedip atau gerak kepala,
                    // jadi mengirim 1.0 akan berbohong kepada server dan
                    // membuat ambang FACE_MIN_LIVENESS tidak berarti.
                    // Operator hadir mengawasi layar - itu penjaganya,
                    // bukan angka ini.
                    liveness_score: 0.5,
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

            // Riwayat hanya menambah baris untuk PERUBAHAN. Orang yang
            // berdiri diam menghasilkan balasan yang sama berkali-kali, dan
            // mencatat semuanya membuat riwayat tidak terbaca.
            const key = (s ? s.id : 'unknown') + '|' + (data.action || '');
            if (key === lastKey) return;
            lastKey = key;

            if (matched) { count += 1; el('counter').textContent = count + ' tercatat'; }

            const body = el('logBody');
            if (body.dataset.filled !== '1') { body.innerHTML = ''; body.dataset.filled = '1'; }

            const tr = document.createElement('tr');
            const nameCell = document.createElement('td');
            nameCell.className = 'ps-4 py-3';

            const nm = document.createElement('span');
            nm.className = 'fw-semibold fs-7 d-block';
            nm.textContent = s ? s.full_name : 'Tidak dikenali';

            const meta = document.createElement('span');
            meta.className = 'text-muted fs-9';
            meta.textContent = (s && s.classroom_name ? s.classroom_name + ' - ' : '')
                + new Date().toLocaleTimeString('id-ID')
                + (message ? ' - ' + message : '');

            nameCell.append(nm, meta);
            tr.append(nameCell);
            body.prepend(tr);
        }

        window.addEventListener('pagehide', function () {
            if (timer) clearInterval(timer);
            if (stream) stream.getTracks().forEach((t) => t.stop());
        });

        device = loadDevice();
        showPanels();
        if (device) boot();
    })();
    </script>
@endpush
