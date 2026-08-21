<!DOCTYPE html>
<html lang="en">
<!--begin::Head-->
<head>
    <base href="{{ url('/') }}/" />
    <title>@yield('title') — {{ $appSettings['site_name'] ?? 'StarterTemp' }}</title>
    <meta charset="utf-8" />
    <meta name="description" content="{{ $appSettings['site_name'] ?? 'StarterTemp' }} — Admin Dashboard" />
    <meta name="author" content="Rendy Irawan" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <meta property="og:locale" content="id_ID" />
    <meta property="og:type" content="website" />
    <meta property="og:title" content="{{ $appSettings['site_name'] ?? 'StarterTemp' }} — Dashboard" />
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
    <!--begin::Vendor Stylesheets-->
    <link href="{{ asset('assets/plugins/custom/fullcalendar/fullcalendar.bundle.css') }}" rel="stylesheet" type="text/css" />
    <link href="{{ asset('assets/plugins/custom/datatables/datatables.bundle.css') }}" rel="stylesheet" type="text/css" />
    <!--end::Vendor Stylesheets-->
    <!--begin::Global Stylesheets Bundle(mandatory for all pages)-->
    <link href="{{ asset('assets/plugins/global/plugins.bundle.css') }}" rel="stylesheet" type="text/css" />
    <link href="{{ asset('assets/css/style.bundle.css') }}" rel="stylesheet" type="text/css" />
    {{-- Tema Jargon GO — dimuat SETELAH style.bundle.css supaya menimpa Metronic
         tanpa menyunting berkas vendor. Lihat public/assets/css/jargon-theme.css --}}
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
        /* Sidebar overlay for mobile */
        .sidebar-overlay {
            display: none;
            position: fixed;
            top: 0; left: 0; right: 0; bottom: 0;
            background: rgba(0,0,0,0.4);
            z-index: 104;
        }
        .sidebar-overlay.active { display: block; }
        /* Aturan #kt_app_sidebar dipindahkan ke public/assets/css/jargon-theme.css.
           Versi lama di sini membuat sidebar `position: fixed` dengan
           `translateX(-100%)` pada SEMUA lebar layar, sementara tombol
           pembukanya (#kt_app_sidebar_toggle) ber-kelas d-lg-none — jadi di
           desktop sidebar tidak pernah bisa muncul sama sekali. Karena blok
           <style> inline ini berada setelah <link> tema, ia selalu menang;
           itulah sebabnya aturan sidebar tidak boleh tinggal di sini. */
    </style>
    <script>
        if (window.top != window.self) { window.top.location.replace(window.self.location.href); }
    </script>
    @stack('stylesheets')
</head>
<!--end::Head-->
<!--begin::Body-->
<body id="kt_body" class="header-fixed header-tablet-and-mobile-fixed toolbar-enabled">
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
    <!--begin::Main-->
    <!--begin::Root-->
    <div class="d-flex flex-column flex-root">
        <!--begin::Page-->
        <div class="page d-flex flex-row flex-column-fluid">
            <!--begin::Wrapper-->
            <div class="wrapper d-flex flex-column flex-row-fluid" id="kt_wrapper">
                <!--begin::Header-->
                <div id="kt_header" class="header" data-kt-sticky="true" data-kt-sticky-name="header" data-kt-sticky-offset="{default: '200px', lg: '300px'}">
                    <!--begin::Container (Top Bar)-->
                    <div class="container-fluid d-flex flex-grow-1 flex-stack">
                        <!--begin::Header Logo-->
                        <div class="d-flex align-items-center me-5">
                            <!--begin::Sidebar toggle (mobile)-->
                            <div class="d-lg-none btn btn-icon btn-active-color-primary w-30px h-30px ms-n2 me-3" id="kt_app_sidebar_toggle">
                                <i class="ki-duotone ki-abstract-14 fs-2"><span class="path1"></span><span class="path2"></span></i>
                            </div>
                            <!--end::Sidebar toggle-->
                            <!--begin::Header menu toggle (mobile)-->
                            <div class="d-lg-none btn btn-icon btn-active-color-primary w-30px h-30px me-3" id="kt_header_menu_toggle">
                                <i class="ki-duotone ki-text-align-left fs-2"><span class="path1"></span><span class="path2"></span><span class="path3"></span><span class="path4"></span></i>
                            </div>
                            <!--end::Header menu toggle-->
                            <a href="{{ route('dashboard') }}">
                                <img alt="Logo" src="{{ asset('assets/media/logos/' . $siteLogo) }}" class="theme-light-show h-20px h-lg-30px" />
                                <img alt="Logo" src="{{ asset('assets/media/logos/' . $siteLogo) }}" class="theme-dark-show h-20px h-lg-30px" />
                            </a>
                        </div>
                        <!--end::Header Logo-->

                        {{-- Menu sebaris dengan logo. Metronic memindahkan elemen menu ke sini
                             lewat data-kt-swapper saat >=lg (lihat data-kt-swapper-parent di
                             menu.blade.php); di layar kecil ia dipindah ke #kt_body sbg drawer. --}}
                        <div class="d-none d-lg-flex align-items-center flex-grow-1" id="kt_header_menu_wrapper">
                            @include('backend.layout.menu')
                        </div>

                        <!--begin::Topbar-->
                        @include('backend.layout.navbar')
                        <!--end::Topbar-->
                    </div>
                    <!--end::Container-->
                        {{-- Baris menu terpisah (#kt_header_nav) dihapus: menunya kini sebaris
                             dengan logo di atas, sesuai permintaan. --}}
                </div>
                <!--end::Header-->

                <!--begin::Container-->
                <div id="kt_content_container" class="d-flex flex-column-fluid align-items-start container-fluid">
                    <!--begin::Sidebar (Custom for Demo 11)-->
                    @include('backend.layout.sidebar')
                    <!--end::Sidebar-->
                    <!--begin::Content-->
                    <div class="content flex-row-fluid" id="kt_content">
                        @yield('content')
                    </div>
                    <!--end::Content-->
                </div>
                <!--end::Container-->

                <!--begin::Footer-->
                @include('backend.layout.footer')
                <!--end::Footer-->
            </div>
            <!--end::Wrapper-->
        </div>
        <!--end::Page-->
    </div>
    <!--end::Root-->
    <!--end::Main-->

    <!--begin::Sidebar Overlay (mobile)-->
    <div class="sidebar-overlay" id="kt_sidebar_overlay"></div>

    <!--begin::Javascript-->
    <script>
        var hostUrl = "{{ asset('assets/') }}";
    </script>
    <script src="{{ asset('assets/plugins/global/plugins.bundle.js') }}"></script>
    <script src="{{ asset('assets/js/scripts.bundle.js') }}"></script>
    <script src="{{ asset('assets/plugins/custom/fullcalendar/fullcalendar.bundle.js') }}"></script>
    <script src="{{ asset('assets/plugins/custom/datatables/datatables.bundle.js') }}"></script>
    <script src="{{ asset('assets/js/widgets.bundle.js') }}"></script>
    <script src="{{ asset('assets/js/custom/widgets.js') }}"></script>
    <script src="{{ asset('assets/js/custom/apps/chat/chat.js') }}"></script>
    <script src="{{ asset('assets/js/custom/utilities/modals/create-campaign.js') }}"></script>
    <script>
        document.addEventListener('DOMContentLoaded', function() {
            // --- Sidebar Toggle ---
            const sidebar = document.getElementById('kt_app_sidebar');
            const overlay = document.getElementById('kt_sidebar_overlay');
            const toggleBtns = document.querySelectorAll('#kt_app_sidebar_toggle, #kt_sidebar_toggle_desktop');

            toggleBtns.forEach(btn => {
                btn.addEventListener('click', function() {
                    sidebar.classList.toggle('active');
                    overlay.classList.toggle('active');
                });
            });
            if (overlay) {
                overlay.addEventListener('click', function() {
                    sidebar.classList.remove('active');
                    overlay.classList.remove('active');
                });
            }

            // --- Global Toastr ---
            toastr.options = {
                "closeButton": true,
                "progressBar": true,
                "positionClass": "toastr-top-right",
                "timeOut": "5000"
            };
            @if(session('success')) toastr.success("{{ session('success') }}"); @endif
            @if(session('error')) toastr.error("{{ session('error') }}"); @endif
            @if(session('warning')) toastr.warning("{{ session('warning') }}"); @endif
            @if(session('info')) toastr.info("{{ session('info') }}"); @endif

            // --- Notifikasi untuk elemen yang tidak menavigasi ---
            // Beberapa elemen di dashboard tampak seperti tombol tetapi hanya
            // menyatakan keadaan (mis. "Lengkap"). Daripada diam saat diklik,
            // elemen ber-atribut data-jg-notify menjelaskan dirinya. Memakai
            // toastr yang sudah dimuat tema — tanpa pustaka tambahan.
            document.addEventListener('click', function (e) {
                const el = e.target.closest('[data-jg-notify]');
                if (! el) return;
                e.preventDefault();
                const pesan = el.getAttribute('data-jg-notify');
                const jenis = el.getAttribute('data-jg-notify-type') || 'info';
                if (window.toastr && typeof toastr[jenis] === 'function') {
                    toastr[jenis](pesan);
                } else if (window.Swal) {
                    Swal.fire({ text: pesan.replace(/&rsaquo;/g, '>'), icon: jenis, confirmButtonText: 'Oke' });
                } else {
                    alert(pesan);
                }
            });

            // --- Force Logout Listener ---
            @auth
            const userId = "{{ auth()->id() }}";
            const waitForEchoLogout = setInterval(() => {
                if (window.Echo) {
                    clearInterval(waitForEchoLogout);
                    window.Echo.private(`App.Models.User.${userId}`)
                        .listen('ForceLogoutNotification', (e) => {
                            Swal.fire({
                                title: 'Keamanan Akun', text: e.message, icon: 'warning',
                                allowOutsideClick: false, allowEscapeKey: false,
                                confirmButtonText: 'OK, Logout', confirmButtonColor: '#d33'
                            }).then(() => { window.location.href = "{{ route('login') }}"; });
                        });
                }
            }, 500);
            @endauth
        });
    </script>
    <!--end::Javascript-->
    @stack('scripts')
</body>
<!--end::Body-->
</html>
