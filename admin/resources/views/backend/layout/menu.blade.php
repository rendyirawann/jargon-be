{{-- Bar menu atas (navy) — akses cepat ke seluruh bagian.

     Ditampilkan berdasarkan IZIN, bukan peran. Versi sebelumnya memakai
     @role('Superadmin|superadmin') sehingga peran lain hanya melihat
     "Dashboard" — operator sekolah tidak punya jalan ke menu apa pun dari
     bar atas. Dengan izin, menambah/mengubah peran di /admin/roles langsung
     memengaruhi navigasi tanpa menyunting view ini.

     Butir menu di sini SAMA dengan sidebar (backend/layout/sidebar.blade.php)
     dan hanya menunjuk rute yang benar-benar terdaftar di routes/web.php. --}}
@php
    $u = auth()->user();
    $can = fn (string $perm) => $u && $u->can($perm);
    $is = fn (string $pattern) => request()->is($pattern);

    // Grup dianggap "sedang dibuka" bila salah satu anaknya aktif — dipakai
    // untuk menyorot butir induk di bar navy.
    $aktif = fn (array $pola) => collect($pola)->contains(fn ($p) => request()->is($p));
@endphp

<div class="header-menu flex-column flex-lg-row"
    data-kt-drawer="true" data-kt-drawer-name="header-menu"
    data-kt-drawer-activate="{default: true, lg: false}" data-kt-drawer-overlay="true"
    data-kt-drawer-width="{default:'200px', '300px': '250px'}" data-kt-drawer-direction="start"
    data-kt-drawer-toggle="#kt_header_menu_toggle"
    data-kt-swapper="true" data-kt-swapper-mode="prepend"
    data-kt-swapper-parent="{default: '#kt_body', lg: '#kt_header_menu_wrapper'}">

    <!--begin::Menu-->
    <div class="menu menu-rounded menu-column menu-lg-row menu-root-here-bg-desktop menu-active-bg menu-state-primary menu-title-gray-800 menu-arrow-gray-500 align-items-stretch flex-grow-1 my-5 my-lg-0 px-2 px-lg-0 fw-semibold fs-6"
        id="#kt_header_menu" data-kt-menu="true">

        {{-- --------------------------------------------------- Dashboard --}}
        <div class="menu-item me-0 me-lg-1 {{ request()->routeIs('dashboard') ? 'here show menu-here-bg' : '' }}">
            <a class="menu-link py-3" href="{{ route('dashboard') }}">
                <span class="menu-icon">
                    <i class="ki-duotone ki-element-11 fs-4"><span class="path1"></span><span class="path2"></span><span class="path3"></span><span class="path4"></span></i>
                </span>
                <span class="menu-title">Dashboard</span>
            </a>
        </div>

        {{-- ---------------------------------------------------- Absensi --}}
        @if ($can('view_attendance'))
            <div data-kt-menu-trigger="{default: 'click', lg: 'hover'}" data-kt-menu-placement="bottom-start"
                class="menu-item menu-lg-down-accordion me-0 me-lg-1 {{ $aktif(['admin/attendances*', 'admin/attendance-rules*']) ? 'here show menu-here-bg' : '' }}">
                <span class="menu-link py-3">
                    <span class="menu-icon">
                        <i class="ki-duotone ki-calendar-tick fs-4"><span class="path1"></span><span class="path2"></span><span class="path3"></span><span class="path4"></span><span class="path5"></span><span class="path6"></span></i>
                    </span>
                    <span class="menu-title">Absensi</span>
                    <span class="menu-arrow d-lg-none"></span>
                </span>
                <div class="menu-sub menu-sub-lg-down-accordion menu-sub-lg-dropdown py-3 w-lg-250px">
                    <div class="menu-item">
                        <a class="menu-link {{ request()->routeIs('attendances.index') ? 'active' : '' }}" href="{{ route('attendances.index') }}">
                            <span class="menu-title">Data Absensi</span>
                        </a>
                    </div>
                    <div class="menu-item">
                        <a class="menu-link {{ request()->routeIs('attendances.by-classroom') ? 'active' : '' }}" href="{{ route('attendances.by-classroom') }}">
                            <span class="menu-title">Rekap per Kelas</span>
                        </a>
                    </div>
                    @if ($can('view_report'))
                        <div class="menu-item">
                            <a class="menu-link {{ request()->routeIs('attendances.recap') ? 'active' : '' }}" href="{{ route('attendances.recap') }}">
                                <span class="menu-title">Rekap per Siswa</span>
                            </a>
                        </div>
                    @endif
                    @if ($can('manage_attendance_rule'))
                        <div class="separator my-2"></div>
                        <div class="menu-item">
                            <a class="menu-link {{ request()->routeIs('attendance-rules.*') ? 'active' : '' }}" href="{{ route('attendance-rules.index') }}">
                                <span class="menu-title">Jam Masuk &amp; Pulang</span>
                            </a>
                        </div>
                    @endif
                </div>
            </div>
        @endif

        {{-- ------------------------------------------------- Data Master --}}
        @if ($can('view_student') || $can('view_classroom') || $can('view_school'))
            <div data-kt-menu-trigger="{default: 'click', lg: 'hover'}" data-kt-menu-placement="bottom-start"
                class="menu-item menu-lg-down-accordion me-0 me-lg-1 {{ $aktif(['admin/students*', 'admin/classrooms*', 'admin/schools*']) ? 'here show menu-here-bg' : '' }}">
                <span class="menu-link py-3">
                    <span class="menu-icon">
                        <i class="ki-duotone ki-book fs-4"><span class="path1"></span><span class="path2"></span><span class="path3"></span><span class="path4"></span></i>
                    </span>
                    <span class="menu-title">Data Master</span>
                    <span class="menu-arrow d-lg-none"></span>
                </span>
                <div class="menu-sub menu-sub-lg-down-accordion menu-sub-lg-dropdown py-3 w-lg-250px">
                    @if ($can('view_student'))
                        <div class="menu-item">
                            <a class="menu-link {{ $is('admin/students*') ? 'active' : '' }}" href="{{ route('students.index') }}">
                                <span class="menu-title">Siswa</span>
                            </a>
                        </div>
                    @endif
                    @if ($can('view_classroom'))
                        <div class="menu-item">
                            <a class="menu-link {{ $is('admin/classrooms*') ? 'active' : '' }}" href="{{ route('classrooms.index') }}">
                                <span class="menu-title">Kelas / Rombel</span>
                            </a>
                        </div>
                    @endif
                    @if ($can('view_school'))
                        <div class="menu-item">
                            <a class="menu-link {{ $is('admin/schools*') ? 'active' : '' }}" href="{{ route('schools.index') }}">
                                <span class="menu-title">Sekolah</span>
                            </a>
                        </div>
                    @endif
                </div>
            </div>
        @endif

        {{-- --------------------------------------- Biometrik & Perangkat --}}
        @if ($can('view_face_enrollment') || $can('view_device'))
            <div data-kt-menu-trigger="{default: 'click', lg: 'hover'}" data-kt-menu-placement="bottom-start"
                class="menu-item menu-lg-down-accordion me-0 me-lg-1 {{ $aktif(['admin/biometric*', 'admin/devices*']) ? 'here show menu-here-bg' : '' }}">
                <span class="menu-link py-3">
                    <span class="menu-icon">
                        <i class="ki-duotone ki-scan-barcode fs-4"><span class="path1"></span><span class="path2"></span><span class="path3"></span><span class="path4"></span><span class="path5"></span><span class="path6"></span><span class="path7"></span><span class="path8"></span></i>
                    </span>
                    <span class="menu-title">Biometrik</span>
                    <span class="menu-arrow d-lg-none"></span>
                </span>
                <div class="menu-sub menu-sub-lg-down-accordion menu-sub-lg-dropdown py-3 w-lg-250px">
                    @if ($can('view_face_enrollment'))
                        <div class="menu-item">
                            <a class="menu-link {{ $is('admin/biometric*') ? 'active' : '' }}" href="{{ route('biometric.index') }}">
                                <span class="menu-title">Pendaftaran Wajah</span>
                            </a>
                        </div>
                    @endif
                    @if ($can('view_device'))
                        <div class="menu-item">
                            <a class="menu-link {{ $is('admin/devices*') ? 'active' : '' }}" href="{{ route('devices.index') }}">
                                <span class="menu-title">Perangkat Tablet</span>
                            </a>
                        </div>
                    @endif
                </div>
            </div>
        @endif

        {{-- ---------------------------------------------------- Layanan --}}
        @if ($can('manage_app_account') || $can('view_panic_feed') || $can('view_document_submission'))
            <div data-kt-menu-trigger="{default: 'click', lg: 'hover'}" data-kt-menu-placement="bottom-start"
                class="menu-item menu-lg-down-accordion me-0 me-lg-1 {{ $aktif(['admin/app-accounts*', 'admin/panic*', 'admin/documents*']) ? 'here show menu-here-bg' : '' }}">
                <span class="menu-link py-3">
                    <span class="menu-icon">
                        <i class="ki-duotone ki-abstract-26 fs-4"><span class="path1"></span><span class="path2"></span></i>
                    </span>
                    <span class="menu-title">Layanan</span>
                    <span class="menu-arrow d-lg-none"></span>
                </span>
                <div class="menu-sub menu-sub-lg-down-accordion menu-sub-lg-dropdown py-3 w-lg-260px">
                    @if ($can('manage_app_account'))
                        <div class="menu-item">
                            <div class="menu-content jg-navgroup">Akun Aplikasi</div>
                        </div>
                        <div class="menu-item">
                            <a class="menu-link {{ request()->routeIs('app-accounts.index') ? 'active' : '' }}" href="{{ route('app-accounts.index') }}">
                                <span class="menu-title">Daftar Akun</span>
                            </a>
                        </div>
                        <div class="menu-item">
                            <a class="menu-link {{ request()->routeIs('app-accounts.create') ? 'active' : '' }}" href="{{ route('app-accounts.create') }}">
                                <span class="menu-title">Buat Akun</span>
                            </a>
                        </div>
                        <div class="menu-item">
                            <a class="menu-link {{ request()->routeIs('app-accounts.bulk') ? 'active' : '' }}" href="{{ route('app-accounts.bulk') }}">
                                <span class="menu-title">Akun Siswa Massal</span>
                            </a>
                        </div>
                    @endif

                    @if ($can('view_panic_feed'))
                        <div class="separator my-2"></div>
                        <div class="menu-item">
                            <div class="menu-content jg-navgroup">Panic Button</div>
                        </div>
                        <div class="menu-item">
                            <a class="menu-link {{ request()->routeIs('panic.index') ? 'active' : '' }}" href="{{ route('panic.index') }}">
                                <span class="menu-title">Daftar Pengaduan</span>
                            </a>
                        </div>
                        @if ($can('unmask_panic_report'))
                            <div class="menu-item">
                                <a class="menu-link {{ request()->routeIs('panic.unmask-logs') ? 'active' : '' }}" href="{{ route('panic.unmask-logs') }}">
                                    <span class="menu-title">Audit Buka Identitas</span>
                                </a>
                            </div>
                        @endif
                    @endif

                    @if ($can('view_document_submission'))
                        <div class="separator my-2"></div>
                        <div class="menu-item">
                            <div class="menu-content jg-navgroup">Pemberkasan</div>
                        </div>
                        <div class="menu-item">
                            <a class="menu-link {{ request()->routeIs('documents.index') ? 'active' : '' }}" href="{{ route('documents.index') }}">
                                <span class="menu-title">Pengajuan Berkas</span>
                            </a>
                        </div>
                        @if ($can('manage_document_type'))
                            <div class="menu-item">
                                <a class="menu-link {{ request()->routeIs('documents.types') ? 'active' : '' }}" href="{{ route('documents.types') }}">
                                    <span class="menu-title">Jenis Dokumen</span>
                                </a>
                            </div>
                        @endif
                    @endif
                </div>
            </div>
        @endif

        {{-- ------------------------------------------------- Notifikasi --}}
        @if ($can('view_notification'))
            <div data-kt-menu-trigger="{default: 'click', lg: 'hover'}" data-kt-menu-placement="bottom-start"
                class="menu-item menu-lg-down-accordion me-0 me-lg-1 {{ $aktif(['admin/notifications*']) ? 'here show menu-here-bg' : '' }}">
                <span class="menu-link py-3">
                    <span class="menu-icon">
                        <i class="ki-duotone ki-sms fs-4"><span class="path1"></span><span class="path2"></span></i>
                    </span>
                    <span class="menu-title">Notifikasi</span>
                    <span class="menu-arrow d-lg-none"></span>
                </span>
                <div class="menu-sub menu-sub-lg-down-accordion menu-sub-lg-dropdown py-3 w-lg-250px">
                    <div class="menu-item">
                        <a class="menu-link {{ request()->routeIs('notifications.index') ? 'active' : '' }}" href="{{ route('notifications.index') }}">
                            <span class="menu-title">Kirim Pesan</span>
                        </a>
                    </div>
                    <div class="menu-item">
                        <a class="menu-link {{ request()->routeIs('notifications.outbox') ? 'active' : '' }}" href="{{ route('notifications.outbox') }}">
                            <span class="menu-title">Riwayat Pengiriman</span>
                        </a>
                    </div>
                    @if ($can('manage_notification_template'))
                        <div class="menu-item">
                            <a class="menu-link {{ request()->routeIs('notifications.templates') ? 'active' : '' }}" href="{{ route('notifications.templates') }}">
                                <span class="menu-title">Template Pesan</span>
                            </a>
                        </div>
                    @endif
                </div>
            </div>
        @endif

        {{-- ----------------------------------------------------- Sistem --}}
        @if ($can('view_resources') || $can('view_setting') || $can('view_help'))
            <div data-kt-menu-trigger="{default: 'click', lg: 'hover'}" data-kt-menu-placement="bottom-start"
                class="menu-item menu-lg-down-accordion me-0 me-lg-1 {{ $aktif(['admin/users*', 'admin/roles*', 'admin/settings*', 'admin/log-activity*']) ? 'here show menu-here-bg' : '' }}">
                <span class="menu-link py-3">
                    <span class="menu-icon">
                        <i class="ki-duotone ki-setting-2 fs-4"><span class="path1"></span><span class="path2"></span></i>
                    </span>
                    <span class="menu-title">Sistem</span>
                    <span class="menu-arrow d-lg-none"></span>
                </span>
                <div class="menu-sub menu-sub-lg-down-accordion menu-sub-lg-dropdown py-3 w-lg-260px">
                    @if ($can('view_resources'))
                        <div class="menu-item">
                            <div class="menu-content jg-navgroup">Manajemen Pengguna</div>
                        </div>
                        <div class="menu-item">
                            <a class="menu-link {{ $is('admin/users*') ? 'active' : '' }}" href="{{ route('users.index') }}">
                                <span class="menu-title">Pengguna</span>
                            </a>
                        </div>
                        <div class="menu-item">
                            <a class="menu-link {{ $is('admin/roles*') ? 'active' : '' }}" href="{{ route('roles.index') }}">
                                <span class="menu-title">Peran &amp; Izin</span>
                            </a>
                        </div>
                    @endif

                    @if ($can('view_setting'))
                        <div class="separator my-2"></div>
                        <div class="menu-item">
                            <a class="menu-link {{ request()->routeIs('settings.*') ? 'active' : '' }}" href="{{ route('settings.index') }}">
                                <span class="menu-title">Pengaturan Aplikasi</span>
                            </a>
                        </div>
                    @endif

                    @if ($can('view_help'))
                        <div class="separator my-2"></div>
                        <div class="menu-item">
                            <a class="menu-link {{ $is('admin/log-activity*') ? 'active' : '' }}" href="{{ url('admin/log-activity') }}">
                                <span class="menu-title">Log Aktivitas</span>
                            </a>
                        </div>
                        <div class="menu-item">
                            {{-- Dokumentasi API ada di luar dashboard (Swagger milik API Rust)
                                 dan dilindungi Basic Auth, jadi dibuka di tab baru. --}}
                            <a class="menu-link" href="{{ config('services.absensi_api.docs_url') }}" target="_blank" rel="noopener">
                                <span class="menu-title">Dokumentasi API</span>
                                <i class="ki-outline ki-exit-right-corner fs-7 ms-2 text-muted"></i>
                            </a>
                        </div>
                    @endif
                </div>
            </div>
        @endif

    </div>
    <!--end::Menu-->
</div>
