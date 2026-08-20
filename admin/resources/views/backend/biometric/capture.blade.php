@extends('backend.layout.app')
@section('title', 'Daftarkan Wajah - '.$student->full_name)

@section('content')
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

    <div class="row g-5">
        <div class="col-xl-7">
            <div class="card card-flush border border-gray-200">
                <div class="card-header pt-5">
                    <h3 class="card-title fw-bold">Ambil Foto</h3>
                    <div class="card-toolbar">
                        <span class="badge badge-light-primary fs-8">
                            {{ $student->face_sample_count }} / {{ $recommended }} sampel
                        </span>
                    </div>
                </div>
                <div class="card-body pt-3">
                    <div class="alert alert-light-primary py-3 px-4 fs-8 mb-4">
                        <span class="fw-semibold d-block mb-1">Cara mendapatkan hasil terbaik</span>
                        Wajah menghadap kamera, pencahayaan merata (jangan membelakangi jendela),
                        tanpa masker/kacamata gelap. Ambil {{ $recommended }} foto dari sudut berbeda:
                        depan, sedikit miring kiri, dan sedikit miring kanan.
                    </div>

                    <div class="position-relative bg-dark rounded overflow-hidden mb-4"
                         style="aspect-ratio: 4/3;">
                        <video id="video" autoplay playsinline muted
                               class="w-100 h-100" style="object-fit: cover;"></video>
                        <canvas id="canvas" class="d-none"></canvas>

                        {{-- Panduan posisi wajah --}}
                        <div class="position-absolute top-50 start-50 translate-middle border border-3 border-white rounded-circle opacity-50"
                             style="width: 45%; aspect-ratio: 1; pointer-events: none;"></div>

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

                        <div class="row g-3 align-items-end">
                            <div class="col-md-5">
                                <label class="form-label">Pose</label>
                                <select name="pose" class="form-select">
                                    <option value="frontal">Depan (frontal)</option>
                                    <option value="left">Miring kiri</option>
                                    <option value="right">Miring kanan</option>
                                    <option value="up">Menengadah</option>
                                    <option value="down">Menunduk</option>
                                </select>
                            </div>
                            <div class="col-md-7 d-flex gap-2">
                                <button type="button" class="btn btn-primary flex-grow-1" id="btnCapture" disabled>
                                    <i class="ki-outline ki-camera fs-4 me-1"></i>Ambil &amp; Simpan
                                </button>
                                <button type="button" class="btn btn-light" id="btnRetry">Ulangi kamera</button>
                            </div>
                        </div>
                    </form>

                    <div id="preview" class="mt-4 d-none">
                        <span class="text-muted fs-8 d-block mb-2">Foto yang akan dikirim:</span>
                        <img id="previewImg" class="rounded border border-gray-300" style="max-height: 160px;" alt="Pratinjau">
                    </div>
                </div>
            </div>
        </div>

        <div class="col-xl-5">
            <div class="card card-flush border border-gray-200 mb-5">
                <div class="card-header pt-5"><h3 class="card-title fw-bold">Sampel Tersimpan</h3></div>
                <div class="card-body pt-3">
                    @if ($samples->isEmpty())
                        <div class="text-center text-muted py-8 fs-8">
                            Belum ada sampel. Siswa belum bisa absen dengan wajah.
                        </div>
                    @else
                        <div class="row g-3">
                            @foreach ($samples as $s)
                                <div class="col-6">
                                    <div class="border border-gray-200 rounded p-2">
                                        <img src="{{ $s->image_url }}" class="rounded w-100 mb-2" alt="Sampel"
                                             style="aspect-ratio: 1; object-fit: cover;" loading="lazy">
                                        <div class="d-flex align-items-center justify-content-between">
                                            <span class="fs-9 text-muted">{{ $s->pose_label }}</span>
                                            <span class="badge {{ $s->quality_badge }} fs-9">
                                                {{ $s->quality_score !== null ? number_format($s->quality_score, 2) : '?' }}
                                            </span>
                                        </div>
                                        @can('delete_face_enrollment')
                                            <form method="POST" action="{{ route('biometric.destroy', $s) }}" class="mt-2"
                                                  onsubmit="return confirm('Hapus sampel ini?');">
                                                @csrf @method('DELETE')
                                                <button class="btn btn-sm btn-light-danger w-100 py-1 fs-9">Hapus</button>
                                            </form>
                                        @endcan
                                    </div>
                                </div>
                            @endforeach
                        </div>
                    @endif
                </div>
            </div>

            <div class="card card-flush border border-gray-200">
                <div class="card-body p-5">
                    <span class="fw-semibold fs-7 d-block mb-3">Apa yang disimpan?</span>
                    <ul class="fs-8 text-gray-700 ps-4 mb-4">
                        <li class="mb-2">
                            <span class="fw-semibold">Saat pendaftaran ini:</span>
                            foto wajah (arsip, agar bisa dihitung ulang bila model di-upgrade)
                            dan vektor biometrik.
                        </li>
                        <li class="mb-2">
                            <span class="fw-semibold">Saat absen harian:</span>
                            tidak ada gambar dan tidak ada vektor yang disimpan — hanya
                            nama, kelas, sekolah, dan jam masuk/pulang.
                        </li>
                        <li>
                            Menghapus siswa akan memusnahkan seluruh data wajahnya secara permanen.
                        </li>
                    </ul>
                    <div class="separator mb-3"></div>
                    <div class="fs-9 text-muted">
                        Model: <code>{{ $modelVersion }}</code> &middot; dimensi {{ $embeddingDim }}
                    </div>
                </div>
            </div>
        </div>
    </div>
@endsection

@push('scripts')
    {{--
        Ekstraksi embedding dilakukan DI BROWSER, bukan di server.

        Alasannya konsistensi: vektor pendaftaran harus dihasilkan model yang
        sama dengan yang dipakai tablet saat absen. Satu model di sisi klien,
        satu `model_version` yang divalidasi server — kalau berbeda, request
        ditolak alih-alih menghasilkan pengenalan acak.

        Model diletakkan di public/assets/models/facenet/ (lihat docs).
    --}}
    <script src="{{ asset('assets/vendor/tfjs/tf.min.js') }}" defer></script>
    <script>
        (function () {
            const video = document.getElementById('video');
            const canvas = document.getElementById('canvas');
            const status = document.getElementById('camStatus');
            const btnCapture = document.getElementById('btnCapture');
            const btnRetry = document.getElementById('btnRetry');
            const form = document.getElementById('enrollForm');
            const EMBEDDING_DIM = {{ $embeddingDim }};

            let stream = null;
            let model = null;

            function setStatus(text, tone) {
                status.textContent = text;
                status.className = 'position-absolute bottom-0 start-0 end-0 p-3 fs-8 text-white bg-opacity-75 '
                    + (tone === 'error' ? 'bg-danger' : tone === 'ok' ? 'bg-success' : 'bg-dark');
            }

            async function startCamera() {
                try {
                    stream = await navigator.mediaDevices.getUserMedia({
                        video: { facingMode: 'user', width: { ideal: 640 }, height: { ideal: 480 } },
                        audio: false,
                    });
                    video.srcObject = stream;
                    setStatus('Kamera siap. Posisikan wajah di dalam lingkaran.', 'ok');
                    await loadModel();
                } catch (e) {
                    setStatus('Kamera tidak dapat diakses: ' + e.message
                        + '. Pastikan izin kamera diberikan dan halaman diakses lewat HTTPS atau localhost.', 'error');
                }
            }

            async function loadModel() {
                if (typeof tf === 'undefined') {
                    setStatus('Pustaka model belum tersedia. Lihat docs/DEPLOYMENT.md '
                        + 'bagian "Model embedding di browser".', 'error');
                    return;
                }
                try {
                    setStatus('Memuat model pengenalan wajah...', null);
                    model = await tf.loadGraphModel(@json(asset('assets/models/facenet/model.json')));
                    btnCapture.disabled = false;
                    setStatus('Model siap. Klik "Ambil & Simpan".', 'ok');
                } catch (e) {
                    setStatus('Model gagal dimuat: ' + e.message, 'error');
                }
            }

            function grabFrame() {
                // Crop persegi di tengah frame, lalu skala ke 160x160 sesuai
                // input model. Rasio dijaga agar wajah tidak terdistorsi.
                const size = Math.min(video.videoWidth, video.videoHeight);
                const sx = (video.videoWidth - size) / 2;
                const sy = (video.videoHeight - size) / 2;

                canvas.width = 160;
                canvas.height = 160;
                canvas.getContext('2d').drawImage(video, sx, sy, size, size, 0, 0, 160, 160);
                return canvas;
            }

            async function extractEmbedding(frame) {
                const tensor = tf.tidy(function () {
                    // Normalisasi ke [-1, 1] — konvensi FaceNet/MobileFaceNet.
                    return tf.browser.fromPixels(frame)
                        .toFloat()
                        .sub(127.5)
                        .div(127.5)
                        .expandDims(0);
                });

                try {
                    const output = model.predict(tensor);
                    const raw = Array.from(await output.data());
                    output.dispose();

                    if (raw.length !== EMBEDDING_DIM) {
                        throw new Error('Dimensi model ' + raw.length + ' tidak sesuai ' + EMBEDDING_DIM);
                    }

                    // L2-normalize di klien: server juga menormalkan, tetapi
                    // mengirim vektor yang sudah normal membuat nilai yang
                    // tersimpan dan yang dibandingkan pasti identik.
                    let norm = Math.sqrt(raw.reduce((s, v) => s + v * v, 0));
                    if (!isFinite(norm) || norm < 1e-9) throw new Error('Vektor tidak valid');

                    return raw.map((v) => v / norm);
                } finally {
                    tensor.dispose();
                }
            }

            btnCapture.addEventListener('click', async function () {
                btnCapture.disabled = true;
                setStatus('Memproses wajah...', null);

                try {
                    const frame = grabFrame();
                    const jpeg = frame.toDataURL('image/jpeg', 0.92);
                    const embedding = await extractEmbedding(frame);

                    document.getElementById('imageBase64').value = jpeg.split(',')[1];
                    document.getElementById('embeddingJson').value = '';

                    // Embedding dikirim sebagai array field agar Laravel
                    // memvalidasinya sebagai array, bukan string JSON.
                    document.querySelectorAll('input[name^="embedding["]').forEach((el) => el.remove());
                    embedding.forEach(function (v, i) {
                        const input = document.createElement('input');
                        input.type = 'hidden';
                        input.name = 'embedding[' + i + ']';
                        input.value = v;
                        form.append(input);
                    });

                    document.getElementById('previewImg').src = jpeg;
                    document.getElementById('preview').classList.remove('d-none');

                    setStatus('Mengirim ke server...', 'ok');
                    form.submit();
                } catch (e) {
                    setStatus('Gagal memproses: ' + e.message, 'error');
                    btnCapture.disabled = false;
                }
            });

            btnRetry.addEventListener('click', function () {
                if (stream) stream.getTracks().forEach((t) => t.stop());
                startCamera();
            });

            // Kamera dilepas saat halaman ditinggalkan supaya lampu indikator
            // tidak tetap menyala di perangkat sekolah.
            window.addEventListener('pagehide', function () {
                if (stream) stream.getTracks().forEach((t) => t.stop());
            });

            startCamera();
        })();
    </script>
@endpush
