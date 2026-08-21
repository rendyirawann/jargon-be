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
        sama dengan yang dipakai saat absen. Satu model di sisi klien, satu
        `model_version` yang divalidasi server — kalau berbeda, request
        ditolak alih-alih menghasilkan pengenalan acak.

        Kode ekstraksinya TIDAK ada di halaman ini, melainkan di
        assets/js/jargon-face.js yang dipakai bersama halaman absensi. Kalau
        masing-masing punya salinannya sendiri, satu perubahan kecil di
        salah satunya membuat wajah yang sudah terdaftar tidak lagi
        dikenali — dan kegagalannya tidak tampak sebagai bug, hanya sebagai
        "sistemnya kurang akurat".
    --}}
    <script src="{{ asset('assets/vendor/face-api/face-api.min.js') }}"></script>
    <script src="{{ asset('assets/js/jargon-face.js') }}"></script>
    <script>
        (function () {
            const video = document.getElementById('video');
            const canvas = document.getElementById('canvas');
            const status = document.getElementById('camStatus');
            const btnCapture = document.getElementById('btnCapture');
            const btnRetry = document.getElementById('btnRetry');
            const form = document.getElementById('enrollForm');

            const EMBEDDING_DIM = {{ $embeddingDim }};
            const MODEL_BASE = @json(asset('assets/models/face-api'));

            let stream = null;

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
                    setStatus('Kamera siap. Memuat model...', null);
                } catch (e) {
                    setStatus('Kamera tidak dapat diakses: ' + e.message
                        + '. Pastikan izin kamera diberikan dan halaman diakses lewat HTTPS atau localhost.', 'error');
                    return;
                }

                try {
                    await JargonFace.load(MODEL_BASE);
                    btnCapture.disabled = false;
                    setStatus('Model siap. Posisikan wajah lalu klik "Ambil & Simpan".', 'ok');
                } catch (e) {
                    setStatus(e.message || String(e), 'error');
                }
            }

            /** Simpan frame utuh sebagai arsip foto pendaftaran. */
            function grabFrame() {
                canvas.width = video.videoWidth;
                canvas.height = video.videoHeight;
                canvas.getContext('2d').drawImage(video, 0, 0);
                return canvas;
            }

            btnCapture.addEventListener('click', async function () {
                btnCapture.disabled = true;
                setStatus('Memproses wajah...', null);

                try {
                    // Deteksi dijalankan pada elemen VIDEO, bukan pada frame
                    // yang sudah dipotong: face-api.js melakukan pemotongan
                    // dan penyelarasan sendiri berdasarkan landmark, dan
                    // memotong lebih dulu justru membuang informasi yang
                    // dibutuhkannya.
                    const result = await JargonFace.describe(video);

                    if (result.descriptor.length !== EMBEDDING_DIM) {
                        throw new Error('Dimensi model ' + result.descriptor.length
                            + ' tidak sesuai konfigurasi server (' + EMBEDDING_DIM + ')');
                    }

                    const jpeg = grabFrame().toDataURL('image/jpeg', 0.92);
                    document.getElementById('imageBase64').value = jpeg.split(',')[1];
                    document.getElementById('embeddingJson').value = '';

                    // Embedding dikirim sebagai array field agar Laravel
                    // memvalidasinya sebagai array, bukan string JSON.
                    document.querySelectorAll('input[name^="embedding["]').forEach((el) => el.remove());
                    result.descriptor.forEach(function (v, i) {
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
                    setStatus('Gagal: ' + (e.message || e), 'error');
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
