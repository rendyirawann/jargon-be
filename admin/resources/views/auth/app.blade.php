<!DOCTYPE html>
<html lang="en">
<!--begin::Head-->

<head>

    <title>@yield('title') — {{ $appSettings['site_name'] ?? 'StarterTemp' }}</title>
    <meta charset="utf-8" />
    <meta name="description" content="{{ $appSettings['site_name'] ?? 'StarterTemp' }} — Authentication" />
    <meta name="author" content="Rendy Irawan" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <meta property="og:locale" content="id_ID" />
    <meta property="og:type" content="website" />
    <meta property="og:title" content="{{ $appSettings['site_name'] ?? 'StarterTemp' }} — Login" />
    <meta property="og:url" content="{{ url()->current() }}" />
    <meta property="og:site_name" content="{{ $appSettings['site_name'] ?? 'StarterTemp' }}" />
    <link rel="canonical" href="{{ url()->current() }}" />
    @php
        $siteLogo = $appSettings['site_logo'] ?? 'base-logo.png';
        $siteFont = $appSettings['site_font'] ?? 'Plus Jakarta Sans';
        $siteName = $appSettings['site_name'] ?? 'StarterTemp';
    @endphp
    <link rel="shortcut icon" href="{{ asset('assets/media/logos/' . $siteLogo) }}" />
    <!--begin::Fonts-->
    <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family={{ str_replace(' ', '+', $siteFont) }}:wght@300;400;500;600;700;800&display=swap" />
    <!--end::Fonts-->
    <!--begin::Global Stylesheets Bundle(mandatory for all pages)-->
    <link href="{{ asset('assets/plugins/global/plugins.bundle.css') }}" rel="stylesheet" type="text/css" />
    <link href="{{ asset('assets/css/style.bundle.css') }}" rel="stylesheet" type="text/css" />
    <link href="{{ asset('assets/css/jargon-theme.css') }}?v={{ filemtime(public_path('assets/css/jargon-theme.css')) }}" rel="stylesheet" type="text/css" />
    <!--end::Global Stylesheets Bundle-->
    <style>
        :root {
            --bs-font-sans-serif: '{{ $siteFont }}', sans-serif;
            --bs-body-font-family: '{{ $siteFont }}', sans-serif;
        }
        body { 
            font-family: '{{ $siteFont }}', sans-serif !important; 
        }
        h1, h2, h3, h4, h5, h6, .h1, .h2, .h3, .h4, .h5, .h6 {
            font-family: '{{ $siteFont }}', sans-serif !important;
        }
    </style>
    <script>
        // Frame-busting to prevent site from being loaded within a frame without permission (click-jacking)
        if (window.top != window.self) {
            window.top.location.replace(window.self.location.href);
        }
    </script>

    @stack('stylesheets')
</head>
<!--end::Head-->
<!--begin::Body-->

<body id="kt_body" class="app-blank bgi-size-cover bgi-attachment-fixed bgi-position-center">
    <!--begin::Theme mode setup on page load-->
    <script>
        var defaultThemeMode = "light";
        var themeMode;
        if (document.documentElement) {
            if (document.documentElement.hasAttribute("data-bs-theme-mode")) {
                themeMode = document.documentElement.getAttribute("data-bs-theme-mode");
            } else {
                if (localStorage.getItem("data-bs-theme") !== null) {
                    themeMode = localStorage.getItem("data-bs-theme");
                } else {
                    themeMode = defaultThemeMode;
                }
            }
            if (themeMode === "system") {
                themeMode = window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
            }
            document.documentElement.setAttribute("data-bs-theme", themeMode);
        }
    </script>
    <!--end::Theme mode setup on page load-->
    <!--begin::Root-->
    <div class="d-flex flex-column flex-root" id="kt_app_root">
        <!--begin::Authentication - Sign-in -->
        {{-- Dua kolom: form di kiri, panel identitas di kanan.
             Panel kanan disembunyikan di layar kecil oleh CSS
             (.jg-auth__aside) supaya form mendapat ruang penuh. --}}
        <div class="jg-auth">
            @yield('content')

            <div class="jg-auth__aside">
                <img class="jg-auth__logo-watermark" src="{{ asset('assets/media/logos/' . $siteLogo) }}" alt="" />
                <h2>Absensi Wajah<br>Dinas Pendidikan Sumatera Utara</h2>
                <p>
                    Satu pintu untuk kehadiran siswa, perangkat tablet sekolah, dan
                    laporan harian se-provinsi.
                </p>
                <ul class="jg-auth__points">
                    <li>
                        <span class="jg-dot">1</span>
                        <span>Kehadiran dipindai di tablet sekolah, tercatat seketika di provinsi.</span>
                    </li>
                    <li>
                        <span class="jg-dot">2</span>
                        <span>Wali murid menerima pemberitahuan pada hari yang sama.</span>
                    </li>
                    <li>
                        <span class="jg-dot">3</span>
                        <span>Rekap per kelas, per siswa, dan per sekolah siap diunduh.</span>
                    </li>
                </ul>
            </div>
        </div>
        <!--end::Authentication - Sign-in-->
    </div>
    <!--end::Root-->
    <!--begin::Javascript-->

    <!--begin::Global Javascript Bundle(mandatory for all pages)-->
    <script src="{{ asset('assets/plugins/global/plugins.bundle.js') }}"></script>
    <script src="{{ asset('assets/js/scripts.bundle.js') }}"></script>
    <!--end::Global Javascript Bundle-->
    @stack('scripts')
    <!--end::Javascript-->
</body>
<!--end::Body-->

</html>
