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
    $sudah = $samples->pluck('pose')->unique()->all();
    $berikut = collect(array_keys($urutan))->first(fn ($p) => ! in_array($p, $sudah, true));
    $lengkap = $berikut === null;
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
                    <div class="flex-grow-1 d-flex align-items-center gap-3 rounded p-3
                                {{ $selesai ? 'bg-light-success' : ($aktif ? 'bg-light-primary' : 'bg-light') }}">
                        <span class="fs-2">{!! $selesai ? '&#9989;' : ($aktif ? '&#128248;' : '&#9675;') !!}</span>
                        <div>
                            <span class="fw-bold fs-7 d-block
                                {{ $selesai ? 'text-success' : ($aktif ? 'text-primary' : 'text-muted') }}">
                                {{ $loop->iteration }}. {{ $info['label'] }}
                            </span>
                            <span class="fs-9 text-muted">
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
                        </div>
                    </form>

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
            const EMB_DIM     = {{ $embeddingDim }};
            let TARGET        = @json($berikut);        // null bila sudah lengkap
            let LENGKAP      = @json($lengkap);

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
            const streak = new JargonFace.Streak(5);

            let stream = null, timer = null, busy = false, dikirim = false;

            function setStatus(text, tone) {
                status.textContent = text;
                status.className = 'position-absolute bottom-0 start-0 end-0 p-3 fs-8 text-white bg-opacity-75 '
                    + (tone === 'error' ? 'bg-danger' : tone === 'ok' ? 'bg-success'
                       : tone === 'warn' ? 'bg-warning' : 'bg-dark');
            }

            function wanted() {
                return LENGKAP ? poseSel.value : TARGET;
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

            async function kirim(descriptor, pose) {
                dikirim = true;
                if (timer) { clearInterval(timer); timer = null; }

                const jpeg = grabFrame().toDataURL('image/jpeg', 0.92);
                document.getElementById('imageBase64').value = jpeg.split(',')[1];
                document.getElementById('embeddingJson').value = '';
                document.getElementById('poseField').value = pose;

                // Embedding dikirim sebagai array field agar Laravel
                // memvalidasinya sebagai array, bukan string JSON.
                document.querySelectorAll('input[name^="embedding["]').forEach((el) => el.remove());
                descriptor.forEach(function (v, i) {
                    const input = document.createElement('input');
                    input.type = 'hidden';
                    input.name = 'embedding[' + i + ']';
                    input.value = v;
                    form.append(input);
                });

                document.getElementById('previewImg').src = jpeg;
                document.getElementById('preview').classList.remove('d-none');

                hint.textContent = '';
                tunjukArah(null, false);
                setStatus('Menyimpan sampel ' + JargonFace.poseLabel(pose) + '...', 'ok');
                await simpanSampel();
            }

            /**
             * Simpan sampel lewat AJAX.
             *
             * Sebelumnya memakai form.submit(), sehingga setiap sampel memuat
             * ulang halaman: kamera mati lalu dinyalakan lagi, model face-api
             * dimuat ulang, dan operator menunggu 2-3 detik di antara pose.
             * Untuk tiga pose per siswa dan ratusan siswa, itu menumpuk.
             *
             * Bila fetch gagal (jaringan mati, JS diblokir), jatuh ke
             * form.submit() seperti semula — hidden input embedding masih ada
             * di form, jadi jalur lama tetap utuh.
             */
            async function simpanSampel() {
                try {
                    const res = await fetch(form.action, {
                        method: 'POST',
                        body: new FormData(form),
                        headers: { 'Accept': 'application/json', 'X-Requested-With': 'XMLHttpRequest' },
                        credentials: 'same-origin',
                    });
                    const data = await res.json().catch(() => ({}));

                    if (!res.ok) {
                        const pesan = data.message
                            || (data.errors ? Object.values(data.errors).flat().join(' ')
                                            : 'Gagal menyimpan sampel (HTTP ' + res.status + ').');
                        setStatus('Gagal: ' + pesan, 'error');
                        lanjutkan(false);
                        return;
                    }

                    const badge = document.getElementById('badgeSampel');
                    if (badge && data.sample_count !== undefined) {
                        badge.textContent = data.sample_count + ' sampel';
                    }
                    if (window.toastr) {
                        toastr.success(data.message || 'Sampel tersimpan.');
                    }
                    await segarkanPanel();
                    lanjutkan(true);
                } catch (e) {
                    form.submit();
                }
            }

            /**
             * Ambil ulang halaman ini lalu tukar dua kartu yang berubah:
             * penanda langkah dan daftar sampel.
             *
             * Sengaja memakai HTML dari server, bukan membangun ulang di JS:
             * logika "pose mana yang berikutnya" ada di Blade, dan menduplikasinya
             * di dua tempat adalah cara termudah membuat keduanya menyimpang.
             * Pose berikutnya dibaca dari data-attribute kartu yang baru.
             */
            async function segarkanPanel() {
                try {
                    const r = await fetch(location.href, {
                        headers: { 'X-Requested-With': 'XMLHttpRequest' },
                        credentials: 'same-origin',
                    });
                    const doc = new DOMParser().parseFromString(await r.text(), 'text/html');
                    ['kartuLangkah', 'kartuSampel'].forEach(function (id) {
                        const baru = doc.getElementById(id);
                        const lama = document.getElementById(id);
                        if (baru && lama) lama.replaceWith(baru);
                    });
                    const k = document.getElementById('kartuLangkah');
                    if (k) {
                        TARGET  = k.dataset.target || null;
                        LENGKAP = k.dataset.lengkap === '1';
                    }
                } catch (e) {
                    /* Panel gagal disegarkan bukan alasan menghentikan kamera. */
                }
            }

            /** Siapkan putaran berikutnya tanpa memuat ulang halaman. */
            function lanjutkan(sukses) {
                streak.reset();
                document.getElementById('preview').classList.add('d-none');
                document.querySelectorAll('input[name^="embedding["]').forEach((el) => el.remove());
                dikirim = false;

                if (LENGKAP) {
                    poseSel.classList.remove('d-none');
                    btnMan.disabled = false;
                    setStatus('Semua pose tersimpan. Pilih pose bila ingin menambah sampel.', 'ok');
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
