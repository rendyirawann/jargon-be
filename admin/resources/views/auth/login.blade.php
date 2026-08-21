@extends('auth.app')
@section('title', 'Login')

{{-- Manual Book dibuang atas permintaan: PDF yang dirujuknya
     `Panduan_DineSyncPOS.pdf` adalah panduan aplikasi POS DineSync,
     bukan panduan sistem absensi ini — sisa template. Selain itu ia
     memuat pdf.js dari CDN Cloudflare di halaman login. Yang ikut
     dibuang: blok @push('stylesheets'), tombol mengapung, dua modal,
     dan blok JS PDF.JS. Alur login (AJAX) tidak disentuh. --}}

@section('content')
    <div class="jg-auth__form">

        <div class="w-100" style="max-width: 430px;">

            <div class="w-100 jg-auth__inner">

                <div class="mb-2">
                    <div class="jg-auth__brand">
                        <img alt="Logo" src="{{ asset('assets/media/logos/' . ($appSettings['site_logo'] ?? 'base-logo.png')) }}" />
                        <div>
                            <div class="jg-auth__brand-name">{{ $appSettings['site_name'] ?? 'Jargon GO' }}</div>
                            <div class="jg-auth__brand-sub">Dinas Pendidikan Provinsi Sumatera Utara</div>
                        </div>
                    </div>

                    <h1 class="jg-auth__title">Masuk Dashboard</h1>
                    <p class="jg-auth__sub">
                        Akun dibuat oleh administrator. Bila lupa kata sandi, hubungi
                        operator dinas.
                    </p>
                </div>

                <div class="w-100 jg-auth__formwrap">

                    <form class="form w-100" id="kt_sign_in_form" method="POST" action="{{ route('login') }}">
                        @csrf

                        <div class="fv-row mb-6">
                            <label class="form-label" for="jg_login_identifier">Email, No. WhatsApp, atau Nama Pengguna</label>
                            <input type="text" id="jg_login_identifier" placeholder="mis. operator@disdik.sumutprov.go.id"
                                name="email" autocomplete="username"
                                class="form-control bg-transparent" />
                        </div>

                        <div class="fv-row mb-3 position-relative" data-kt-password-meter="true">
                            <input type="password" placeholder="Password" name="password" autocomplete="off"
                                class="form-control bg-transparent" id="passwordInput" />

                            <span class="btn btn-sm btn-icon position-absolute translate-middle top-50 end-0 me-n2"
                                id="togglePassword">
                                <i class="ki-outline ki-eye-slash fs-2" id="toggleIcon"></i>
                            </span>
                        </div>

                        <div class="d-flex flex-stack flex-wrap gap-3 fs-base fw-semibold mb-8">
                            <div></div>
                            <a href="{{ route('password.request') }}" class="link-primary">Lupa Password ?</a>
                        </div>

                        <div class="d-grid mb-10">
                            <button type="submit" id="kt_sign_in_submit" class="btn btn-primary">
                                <span class="indicator-label">Masuk</span>
                                <span class="indicator-progress">Harap tunggu...
                                    <span class="spinner-border spinner-border-sm align-middle ms-2"></span></span>
                            </button>
                        </div>

                        {{-- Blok "Belum punya akun? Daftar" dibuang karena
                             pendaftaran mandiri dimatikan di routes/auth.php.

                             Jangan menonaktifkannya dengan komentar HTML: Blade
                             tetap mengevaluasi isi komentar HTML, jadi
                             route('register') di dalamnya ikut dijalankan dan
                             melempar RouteNotFoundException begitu rutenya
                             hilang - halaman login pun jadi 500. Komentar Blade
                             seperti blok ini memang tidak dieksekusi. --}}
                    </form>

                    {{-- Social Login Section --}}
                    @php
                        $socialProviders = [
                            'google' => [
                                'enabled' => ($appSettings['social_google_enabled'] ?? '0') === '1',
                                'label' => 'Google',
                                'driver' => 'google',
                                'icon' => '<svg width="20" height="20" viewBox="0 0 24 24"><path d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92a5.06 5.06 0 01-2.2 3.32v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.1z" fill="#4285F4"/><path d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" fill="#34A853"/><path d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z" fill="#FBBC05"/><path d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" fill="#EA4335"/></svg>',
                                'color' => '#fff',
                                'border' => '#dadce0',
                                'text' => '#3c4043',
                            ],
                            'facebook' => [
                                'enabled' => ($appSettings['social_facebook_enabled'] ?? '0') === '1',
                                'label' => 'Facebook',
                                'driver' => 'facebook',
                                'icon' => '<svg width="20" height="20" viewBox="0 0 24 24"><path d="M24 12.073c0-6.627-5.373-12-12-12s-12 5.373-12 12c0 5.99 4.388 10.954 10.125 11.854v-8.385H7.078v-3.47h3.047V9.43c0-3.007 1.792-4.669 4.533-4.669 1.312 0 2.686.235 2.686.235v2.953H15.83c-1.491 0-1.956.925-1.956 1.874v2.25h3.328l-.532 3.47h-2.796v8.385C19.612 23.027 24 18.062 24 12.073z" fill="#1877F2"/></svg>',
                                'color' => '#1877F2',
                                'border' => '#1877F2',
                                'text' => '#fff',
                            ],
                            'github' => [
                                'enabled' => ($appSettings['social_github_enabled'] ?? '0') === '1',
                                'label' => 'GitHub',
                                'driver' => 'github',
                                'icon' => '<svg width="20" height="20" viewBox="0 0 24 24"><path d="M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61C4.422 18.07 3.633 17.7 3.633 17.7c-1.087-.744.084-.729.084-.729 1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776.417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 12.297c0-6.627-5.373-12-12-12" fill="#333"/></svg>',
                                'color' => '#24292f',
                                'border' => '#24292f',
                                'text' => '#fff',
                            ],
                            'linkedin' => [
                                'enabled' => ($appSettings['social_linkedin_enabled'] ?? '0') === '1',
                                'label' => 'LinkedIn',
                                'driver' => 'linkedin-openid',
                                'icon' => '<svg width="20" height="20" viewBox="0 0 24 24"><path d="M20.447 20.452h-3.554v-5.569c0-1.328-.027-3.037-1.852-3.037-1.853 0-2.136 1.445-2.136 2.939v5.667H9.351V9h3.414v1.561h.046c.477-.9 1.637-1.85 3.37-1.85 3.601 0 4.267 2.37 4.267 5.455v6.286zM5.337 7.433c-1.144 0-2.063-.926-2.063-2.065 0-1.138.92-2.063 2.063-2.063 1.14 0 2.064.925 2.064 2.063 0 1.139-.925 2.065-2.064 2.065zm1.782 13.019H3.555V9h3.564v11.452zM22.225 0H1.771C.792 0 0 .774 0 1.729v20.542C0 23.227.792 24 1.771 24h20.451C23.2 24 24 23.227 24 22.271V1.729C24 .774 23.2 0 22.222 0h.003z" fill="#0A66C2"/></svg>',
                                'color' => '#0A66C2',
                                'border' => '#0A66C2',
                                'text' => '#fff',
                            ],
                        ];
                        $hasAnySocial = collect($socialProviders)->contains('enabled', true);
                    @endphp

                    @if($hasAnySocial)
                        <div class="separator separator-content my-8">
                            <span class="text-gray-500 fw-semibold fs-7">Atau masuk dengan</span>
                        </div>

                        <div class="d-flex flex-wrap justify-content-center gap-3">
                            @foreach($socialProviders as $key => $provider)
                                @if($provider['enabled'])
                                    <a href="{{ route('social.redirect', $provider['driver']) }}"
                                        class="btn btn-flex btn-outline btn-text-gray-700 btn-active-color-primary bg-state-light flex-center"
                                        style="border-color: {{ $provider['border'] }}; min-width: 140px;">
                                        {!! $provider['icon'] !!}
                                        <span class="ms-2 fs-7 fw-bold">{{ $provider['label'] }}</span>
                                    </a>
                                @endif
                            @endforeach
                        </div>
                    @endif

                </div>
            </div>
        </div>
    </div>


    @push('scripts')
        <script>
            const togglePassword = document.querySelector('#togglePassword');
            const passwordInput = document.querySelector('#passwordInput');
            const toggleIcon = document.querySelector('#toggleIcon');

            if (togglePassword && passwordInput) {
                togglePassword.addEventListener('click', function(e) {
                    // Toggle type attribute
                    const type = passwordInput.getAttribute('type') === 'password' ? 'text' : 'password';
                    passwordInput.setAttribute('type', type);

                    // Toggle icon class (Metronic Icons)
                    if (type === 'text') {
                        toggleIcon.classList.remove('ki-eye-slash');
                        toggleIcon.classList.add('ki-eye');
                    } else {
                        toggleIcon.classList.remove('ki-eye');
                        toggleIcon.classList.add('ki-eye-slash');
                    }
                });
            }
        </script>

        <script>
            document.getElementById('kt_sign_in_form').addEventListener('submit', function(e) {
                e.preventDefault();

                // 1. Ambil Elemen Tombol
                const submitButton = document.getElementById('kt_sign_in_submit');

                // 2. Aktifkan Loading State
                submitButton.setAttribute("data-kt-indicator", "on");
                submitButton.classList.add("disabled");
                submitButton.disabled = true;

                const label = submitButton.querySelector('.indicator-label');
                const progress = submitButton.querySelector('.indicator-progress');
                if (label) label.style.display = 'none';
                if (progress) progress.style.display = 'inline-block';

                let formData = new FormData(this);

                fetch("{{ route('login') }}", {
                        method: "POST",
                        headers: {
                            "X-CSRF-TOKEN": "{{ csrf_token() }}",
                            "Accept": "application/json"
                        },
                        body: formData
                    })
                    .then(async response => {
                        let result = await response.json();

                        if (!response.ok) {
                            // Reset Tombol
                            submitButton.removeAttribute("data-kt-indicator");
                            submitButton.classList.remove("disabled");
                            submitButton.disabled = false;
                            if (label) label.style.display = 'block';
                            if (progress) progress.style.display = 'none';

                            // === KASUS 1: LOCKOUT (Status 429) ===
                            if (response.status === 429) {
                                let seconds = 60;
                                if (result.errors && result.errors.seconds) {
                                    seconds = result.errors.seconds[0];
                                } else if (result.errors && result.errors.email) {
                                    let match = result.errors.email[0].match(/(\d+)/);
                                    if (match) seconds = match[0];
                                }
                                showLockoutCountdown(seconds);
                                return;
                            }

                            // === KASUS 2: ERROR BIASA ===
                            let errorMessage = "Terjadi kesalahan sistem.";
                            if (result.errors && result.errors.email) {
                                errorMessage = result.errors.email[0];
                            } else if (result.message) {
                                errorMessage = result.message;
                            }

                            Swal.fire({
                                icon: "error",
                                title: "Login Gagal!",
                                text: errorMessage,
                                confirmButtonColor: "#d33",
                                buttonsStyling: false,
                                customClass: {
                                    confirmButton: "btn btn-danger"
                                }
                            });

                            return;
                        }

                        // 4. JIKA SUKSES
                        let redirectUrl = result.redirect || "{{ route('dashboard') }}";
                        superPremiumThreeDotLoader(redirectUrl);
                    })
                    .catch(err => {
                        console.error("Fetch Error:", err);
                        submitButton.removeAttribute("data-kt-indicator");
                        submitButton.disabled = false;
                        if (label) label.style.display = 'block';
                        if (progress) progress.style.display = 'none';

                        Swal.fire({
                            icon: "error",
                            title: "Error Jaringan",
                            text: "Tidak dapat terhubung ke server.",
                            confirmButtonColor: "#d33"
                        });
                    });
            });

            // ==========================================
            // FUNGSI 1: COUNTDOWN LOCKOUT
            // ==========================================
            function showLockoutCountdown(seconds) {
                let originalSeconds = seconds;
                const submitButton = document.getElementById('kt_sign_in_submit');
                submitButton.disabled = true;

                Swal.fire({
                    icon: "warning",
                    title: "Terlalu Banyak Percobaan!",
                    html: `
                    Anda telah gagal login berulang kali.<br>
                    Coba lagi dalam <b id="countdown" class="text-danger fs-1">${seconds}</b> detik.
                    <br><br>
                    <div class="progress bg-secondary" style="height: 10px; border-radius: 20px;">
                        <div id="lock-progress" class="progress-bar bg-danger" style="width: 100%; transition: width 1s linear;"></div>
                    </div>
                `,
                    allowOutsideClick: false,
                    allowEscapeKey: false,
                    showConfirmButton: false,
                    timer: seconds * 1000,
                    didOpen: () => {
                        let countdownEl = document.getElementById("countdown");
                        let bar = document.getElementById("lock-progress");

                        let interval = setInterval(() => {
                            seconds--;
                            if (countdownEl) countdownEl.textContent = seconds;
                            if (bar) {
                                let percent = Math.floor((seconds / originalSeconds) * 100);
                                bar.style.width = percent + "%";
                            }
                            if (seconds <= 0) {
                                clearInterval(interval);
                                submitButton.disabled = false;
                                submitButton.classList.remove("disabled");
                            }
                        }, 1000);
                    }
                });
            }

            // ==========================================
            // FUNGSI 2: SUPER PREMIUM LOADER
            // ==========================================
            function superPremiumThreeDotLoader(targetUrl) {
                let timerInterval;
                if (!document.getElementById('dot-loader-style')) {
                    const styleDots = document.createElement('style');
                    styleDots.id = 'dot-loader-style';
                    styleDots.textContent = `
                    .dot-loader { width: 12px; height: 12px; background-color: #22c55e; border-radius: 50%; animation: bounceDot 0.6s infinite alternate; }
                    .dot-loader--2 { animation-delay: 0.15s; }
                    .dot-loader--3 { animation-delay: 0.3s; }
                    @keyframes bounceDot { 0% { transform: translateY(0); opacity: 1; } 100% { transform: translateY(-10px); opacity: 0.4; } }
                `;
                    document.head.appendChild(styleDots);
                }

                Swal.fire({
                    icon: "success",
                    title: `<span class="fw-bold">Login Berhasil</span>`,
                    html: `
                    <div class="text-muted mb-3">Menyiapkan aplikasi untuk Anda...</div>
                    <div class="my-12" style="display:flex; justify-content:center; align-items:center; gap:10px; margin-bottom:22px;">
                        <div class="dot-loader"></div>
                        <div class="dot-loader dot-loader--2"></div>
                        <div class="dot-loader dot-loader--3"></div>
                    </div>
                    <div class='progress bg-secondary mt-3' style='height: 12px; border-radius: 20px; width: 100%; overflow: hidden;'>
                        <div id="sa-progress-premium" class='progress-bar bg-success' style='width: 0%; border-radius: 20px'></div>
                    </div>
                    <div id="sa-percent" class="mt-2 fw-bold text-gray-700">0%</div>
                `,
                    width: 400,
                    padding: "2em",
                    timer: 2000,
                    showConfirmButton: false,
                    allowOutsideClick: false,
                    didOpen: () => {
                        let bar = document.getElementById("sa-progress-premium");
                        let percentText = document.getElementById("sa-percent");
                        let width = 0;
                        timerInterval = setInterval(() => {
                            width += Math.floor(Math.random() * 5) + 1;
                            if (width > 100) width = 100;
                            bar.style.width = width + "%";
                            percentText.innerHTML = width + "%";
                            if (width >= 100) clearInterval(timerInterval);
                        }, 50);
                    },
                    willClose: () => {
                        clearInterval(timerInterval);
                    }
                }).then(() => {
                    const container = document.querySelector(".d-flex.flex-column-fluid");
                    if (container) {
                        container.style.opacity = 0;
                        container.style.transition = "opacity .5s ease-in-out";
                    }
                    setTimeout(() => {
                        window.location.href = targetUrl;
                    }, 300);
                });
            }

        </script>
    @endpush
@endsection
