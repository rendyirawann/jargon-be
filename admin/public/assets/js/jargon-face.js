/**
 * Pengenalan wajah di browser untuk dashboard Jargon GO.
 *
 * MENGAPA SATU BERKAS UNTUK DUA HALAMAN
 *
 * Pendaftaran wajah (biometric/capture) dan absensi (biometric/scan) HARUS
 * menghasilkan vektor yang sebanding. Kalau keduanya punya salinan kode
 * pra-pemrosesan sendiri, satu perubahan kecil di salah satunya — ukuran
 * input, pemakaian landmark, normalisasi — membuat wajah yang sudah
 * terdaftar tidak lagi dikenali. Kegagalannya pun tidak terlihat sebagai
 * bug: ia terlihat sebagai "sistemnya kurang akurat".
 *
 * Karena itu ekstraksi embedding hanya ada di sini, dan kedua halaman
 * memanggil fungsi yang sama.
 *
 * MODEL
 *
 * face-api.js (@vladmandic/face-api), tiga model:
 *   * tiny_face_detector  — deteksi wajah, ringan, cukup cepat untuk video
 *   * face_landmark_68    — titik wajah, dipakai MENYELARASKAN wajah
 *                           sebelum embedding. Tanpa penyelarasan, wajah
 *                           yang sedikit miring menghasilkan vektor yang
 *                           jauh berbeda.
 *   * face_recognition    — embedding 128 dimensi
 *
 * Descriptor face-api.js ber-norm 1. Server tetap menormalkannya lagi
 * (idempoten) dan membandingkan dengan cosine similarity.
 */
(function (global) {
    'use strict';

    var MODELS_LOADED = false;
    var LOADING = null;

    /** Ambang mutu deteksi sebelum embedding diambil. */
    var MIN_DETECTOR_SCORE = 0.5;

    /**
     * Lebar wajah minimum relatif terhadap lebar frame.
     *
     * Wajah yang terlalu kecil menghasilkan embedding buruk — dan embedding
     * buruk yang tersimpan sebagai pendaftaran akan merusak pengenalan
     * siswa itu selamanya, bukan hanya sekali.
     */
    var MIN_FACE_RATIO = 0.16;

    function JargonFaceError(code, message) {
        this.code = code;
        this.message = message;
    }

    /**
     * Muat ketiga model. Aman dipanggil berkali-kali — pemanggilan kedua
     * mengembalikan promise yang sama.
     */
    function load(modelBase) {
        if (MODELS_LOADED) return Promise.resolve();
        if (LOADING) return LOADING;

        if (typeof faceapi === 'undefined') {
            return Promise.reject(new JargonFaceError(
                'lib_missing',
                'Pustaka face-api.js tidak termuat. Periksa ' +
                'assets/vendor/face-api/face-api.min.js'
            ));
        }

        LOADING = Promise.all([
            faceapi.nets.tinyFaceDetector.loadFromUri(modelBase),
            faceapi.nets.faceLandmark68Net.loadFromUri(modelBase),
            faceapi.nets.faceRecognitionNet.loadFromUri(modelBase)
        ]).then(function () {
            MODELS_LOADED = true;
            LOADING = null;
        }).catch(function (e) {
            LOADING = null;
            throw new JargonFaceError(
                'model_missing',
                'Model wajah gagal dimuat dari ' + modelBase + '. ' +
                'Pastikan berkas *-weights_manifest.json dan *.bin ada. (' +
                (e && e.message ? e.message : e) + ')'
            );
        });

        return LOADING;
    }

    /**
     * Ambil satu embedding dari elemen video/canvas/img.
     *
     * Menolak — bukan menebak — bila wajahnya lebih dari satu, terlalu
     * kecil, atau skor deteksinya rendah. Pada absensi, menebak berarti
     * mencatat kehadiran orang yang salah; lebih baik meminta siswa
     * mengulang.
     *
     * @returns {Promise<{descriptor: number[], score: number, box: object,
     *                    ratio: number, faces: number}>}
     */
    function describe(input) {
        if (!MODELS_LOADED) {
            return Promise.reject(new JargonFaceError(
                'not_loaded', 'Model belum dimuat.'
            ));
        }

        var opts = new faceapi.TinyFaceDetectorOptions({
            inputSize: 320,
            scoreThreshold: MIN_DETECTOR_SCORE
        });

        return faceapi
            .detectAllFaces(input, opts)
            .withFaceLandmarks()
            .withFaceDescriptors()
            .then(function (results) {
                if (!results || results.length === 0) {
                    throw new JargonFaceError(
                        'no_face', 'Wajah tidak terdeteksi.'
                    );
                }
                // Dua wajah dalam frame absensi berarti tidak jelas siapa
                // yang sedang absen.
                if (results.length > 1) {
                    throw new JargonFaceError(
                        'many_faces',
                        results.length + ' wajah terdeteksi. Pastikan hanya ' +
                        'satu orang di depan kamera.'
                    );
                }

                var r = results[0];
                var frameWidth = input.videoWidth || input.width;
                var ratio = r.detection.box.width / frameWidth;

                if (ratio < MIN_FACE_RATIO) {
                    throw new JargonFaceError(
                        'too_far', 'Wajah terlalu jauh. Dekatkan ke kamera.'
                    );
                }

                return {
                    descriptor: Array.prototype.slice.call(r.descriptor),
                    score: r.detection.score,
                    box: r.detection.box,
                    ratio: ratio,
                    faces: results.length
                };
            });
    }

    /**
     * Nonce sekali pakai untuk melindungi dari pengiriman ulang payload.
     *
     * crypto.getRandomValues, bukan Math.random: nilai yang bisa diramalkan
     * membuat proteksi replay-nya tidak berarti.
     */
    function nonce() {
        var b = new Uint8Array(16);
        (global.crypto || global.msCrypto).getRandomValues(b);
        return Array.prototype.map
            .call(b, function (x) { return ('0' + x.toString(16)).slice(-2); })
            .join('');
    }

    global.JargonFace = {
        load: load,
        describe: describe,
        nonce: nonce,
        get ready() { return MODELS_LOADED; },
        MIN_FACE_RATIO: MIN_FACE_RATIO,
        MIN_DETECTOR_SCORE: MIN_DETECTOR_SCORE
    };
})(window);
