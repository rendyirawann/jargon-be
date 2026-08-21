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
 * Karena itu ekstraksi embedding DAN penentuan arah kepala hanya ada di
 * sini, dan kedua halaman memanggil fungsi yang sama.
 *
 * MODEL
 *
 * face-api.js (@vladmandic/face-api), tiga model:
 *   * tiny_face_detector  — deteksi wajah, ringan, cukup cepat untuk video
 *   * face_landmark_68    — titik wajah, dipakai MENYELARASKAN wajah
 *                           sebelum embedding, sekaligus memperkirakan
 *                           arah kepala (yaw)
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

    /**
     * Batas yaw untuk menyatakan kepala MENGHADAP DEPAN.
     *
     * Dibuat longgar: orang duduk di depan kamera hampir tidak pernah
     * benar-benar simetris, dan menuntut 0 akan membuat langkah "hadap
     * depan" terasa mustahil diselesaikan.
     */
    var YAW_FRONTAL_MAX = 0.08;
                                  // webcam laptop terbaca ~0.20 (kamera tidak
                                  // tepat di tengah + wajah tidak simetris),
                                  // sehingga 0.14 membuat langkah "hadap depan"
                                  // tidak pernah bisa diselesaikan.

    /**
     * Batas yaw untuk menyatakan kepala SUDAH menoleh.
     *
     * Cukup jauh dari YAW_FRONTAL_MAX agar tidak ada wilayah
     * ambigu yang membuat status berkedip-kedip antara "depan" dan
     * "menoleh".
     */
    var YAW_TURN_MIN = 0.12;

    /**
     * Titik nol yaw untuk kamera + wajah yang sedang dipakai.
     *
     * Webcam laptop jarang tepat di tengah wajah, dan wajah manusia tidak
     * simetris. Akibatnya "lurus" bisa terbaca 0.20, bukan 0.00 — dan menoleh
     * ke arah yang berlawanan dengan bias itu jadi terasa jauh lebih berat
     * karena harus melewati bias dulu sebelum ambang tercapai.
     *
     * Nilai ini diisi halaman pengambilan lewat setYawBias() dari beberapa
     * bacaan saat operator diminta melihat lurus. Bawaannya 0, jadi pemakai
     * yang tidak mengalibrasi tetap berperilaku seperti sebelumnya.
     */
    var YAW_BIAS = 0;

    function setYawBias(v) {
        YAW_BIAS = (typeof v === 'number' && isFinite(v)) ? v : 0;
    }

    function yawBias() { return YAW_BIAS; }
                                  // "antara" tetap lebar (0.22 -> 0.36) dan
                                  // status tidak berkedip saat kepala bergerak.

    /**
     * Arah tanda yaw.
     *
     * BACA INI kalau petunjuk "hadap kanan/kiri" terasa terbalik.
     *
     * yaw dihitung dari piksel kamera yang BELUM dicerminkan:
     *
     *   yaw = SIGN * (hidung.x - tengahMata.x) / jarakAntarMata
     *
     * Ketika seseorang menoleh ke kanannya sendiri, hidungnya bergerak ke
     * KIRI gambar (kamera melihat orang itu sebagaimana orang lain
     * melihatnya). Karena itu tandanya dibalik, sehingga yaw POSITIF
     * berarti "menoleh ke kanan menurut orang yang difoto".
     *
     * Bila di lapangan ternyata terbalik — mis. karena kamera tertentu
     * sudah mencerminkan sendiri keluarannya — ubah nilai ini menjadi 1.
     * Angka yaw ditampilkan di layar kedua halaman supaya hal itu bisa
     * dipastikan dalam hitungan detik, bukan ditebak.
     */
    var YAW_SIGN = -1;

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

    function mean(points, axis) {
        var s = 0;
        for (var i = 0; i < points.length; i++) s += points[i][axis];
        return s / points.length;
    }

    /**
     * Perkiraan arah kepala kiri-kanan (yaw), dinormalkan.
     *
     * Dibagi jarak antar-mata, BUKAN lebar kotak wajah: jarak antar-mata
     * ikut mengecil ketika kepala menoleh, sehingga nilainya lebih stabil
     * terhadap jarak orang ke kamera.
     *
     * Titik hidung diambil dari tengah rangkaian titik hidung, bukan indeks
     * tetap dalam 68 titik — supaya tidak bergantung pada penomoran yang
     * bisa berbeda antar-versi pustaka.
     */
    function yawOf(landmarks) {
        var nose = landmarks.getNose();
        var le = landmarks.getLeftEye();
        var re = landmarks.getRightEye();

        var tip = nose[Math.floor(nose.length / 2)];
        var leX = mean(le, 'x');
        var reX = mean(re, 'x');
        var eyeMidX = (leX + reX) / 2;
        var interocular = Math.abs(reX - leX);

        if (!isFinite(interocular) || interocular < 1) return 0;

        return YAW_SIGN * ((tip.x - eyeMidX) / interocular);
    }

    /**
     * Terjemahkan yaw menjadi pose: `frontal`, `right`, `left`, atau
     * `antara` untuk wilayah di tengah-tengah.
     *
     * `antara` sengaja ADA, bukan dipaksa masuk salah satu: memaksanya
     * membuat status berkedip ketika kepala sedang bergerak, dan langkah
     * pendaftaran akan tertangkap pada posisi setengah jalan.
     */
    function poseOf(yaw) {
        var a = Math.abs(yaw);
        if (a <= YAW_FRONTAL_MAX) return 'frontal';
        if (a >= YAW_TURN_MIN) return yaw > 0 ? 'right' : 'left';
        return 'antara';
    }

    /** Nama pose dalam bahasa Indonesia, untuk ditampilkan. */
    function poseLabel(pose) {
        return {
            frontal: 'menghadap depan',
            right: 'menoleh ke kanan',
            left: 'menoleh ke kiri',
            antara: 'setengah menoleh'
        }[pose] || pose;
    }

    /**
     * Ambil satu embedding + arah kepala dari elemen video/canvas/img.
     *
     * Menolak — bukan menebak — bila wajahnya lebih dari satu, terlalu
     * kecil, atau skor deteksinya rendah. Pada absensi, menebak berarti
     * mencatat kehadiran orang yang salah; lebih baik meminta siswa
     * mengulang.
     *
     * @returns {Promise<{descriptor: number[], score: number, box: object,
     *                    ratio: number, yaw: number, pose: string}>}
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

                var yawMentah = yawOf(r.landmarks);
                // Pose ditentukan dari yaw yang SUDAH dikurangi bias; nilai mentah
                // tetap dikembalikan supaya halaman bisa mengalibrasi ulang.
                var yaw = yawMentah - YAW_BIAS;

                return {
                    descriptor: Array.prototype.slice.call(r.descriptor),
                    score: r.detection.score,
                    box: r.detection.box,
                    ratio: ratio,
                    yaw: yaw,
                    yawRaw: yawMentah,
                    pose: poseOf(yaw)
                };
            });
    }

    /**
     * Penahan kestabilan: pose harus bertahan beberapa frame berurutan.
     *
     * Tanpa ini, satu frame yang kebetulan salah deteksi sudah cukup untuk
     * meloloskan sebuah langkah — dan pada tantangan liveness itu berarti
     * lolos tanpa benar-benar menoleh.
     */
    function Streak(needed) {
        this.needed = needed || 3;
        this.pose = null;
        this.n = 0;
    }

    Streak.prototype.push = function (pose) {
        if (pose === this.pose) {
            this.n += 1;
        } else {
            this.pose = pose;
            this.n = 1;
        }
        return this.n >= this.needed;
    };

    Streak.prototype.reset = function () {
        this.pose = null;
        this.n = 0;
    };

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
        poseOf: poseOf,
        setYawBias: setYawBias,
        yawBias: yawBias,
        poseLabel: poseLabel,
        nonce: nonce,
        Streak: Streak,
        get ready() { return MODELS_LOADED; },
        MIN_FACE_RATIO: MIN_FACE_RATIO,
        MIN_DETECTOR_SCORE: MIN_DETECTOR_SCORE,
        YAW_FRONTAL_MAX: YAW_FRONTAL_MAX,
        YAW_TURN_MIN: YAW_TURN_MIN
    };
})(window);
