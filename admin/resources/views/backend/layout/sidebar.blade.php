{{-- Sidebar navigasi dashboard absensi --}}
@php
    /**
     * Menu ditampilkan berdasarkan IZIN (bukan peran), sehingga menambah peran
     * baru di /admin/roles langsung memengaruhi navigasi tanpa mengubah view.
     */
    $u = auth()->user();
    $can = fn (string $perm) => $u && $u->can($perm);
    $is = fn (string $pattern) => request()->is($pattern);
@endphp

<div id="kt_app_sidebar" class="d-flex flex-column">

    <!--begin::Sidebar Header-->
    <div class="d-flex flex-column align-items-center px-6 pt-8 pb-5">
        @php $siteLogo = $appSettings['site_logo'] ?? 'base-logo.png'; @endphp
        <a href="{{ route('dashboard') }}" class="mb-4">
            <img alt="Logo" src="{{ asset('assets/media/logos/' . $siteLogo) }}" class="h-40px" />
        </a>
        <h5 class="fw-bold text-gray-800 mb-0 fs-6 text-center">{{ $appSettings['site_name'] ?? 'Absensi Disdik Sumut' }}</h5>
        <span class="text-muted fs-8 text-center">{{ $u?->scope_label }}</span>
    </div>
    <!--end::Sidebar Header-->

    <div class="separator mx-6 mb-3"></div>

    <!--begin::Sidebar Menu-->
    <div class="px-4 flex-column-fluid overflow-auto">
        <div class="menu menu-column menu-rounded menu-sub-indention menu-active-bg fw-semibold fs-6" data-kt-menu="true">

            <!--begin::Dashboard-->
            <div class="menu-item">
                <a class="menu-link {{ request()->routeIs('dashboard') ? 'active' : '' }}" href="{{ route('dashboard') }}">
                    <span class="menu-icon">
                        <i class="ki-duotone ki-element-11 fs-3"><span class="path1"></span><span class="path2"></span><span class="path3"></span><span class="path4"></span></i>
                    </span>
                    <span class="menu-title">Dashboard</span>
                </a>
            </div>

            <!--begin::Section: Absensi-->
            @if ($can('view_attendance'))
                <div class="menu-item pt-5">
                    <div class="menu-content"><span class="menu-heading fw-bold text-uppercase fs-7">Absensi</span></div>
                </div>

                <div data-kt-menu-trigger="click" class="menu-item menu-accordion {{ $is('admin/attendance*') ? 'here show' : '' }}">
                    <span class="menu-link">
                        <span class="menu-icon">
                            <i class="ki-duotone ki-user-tick fs-3"><span class="path1"></span><span class="path2"></span><span class="path3"></span></i>
                        </span>
                        <span class="menu-title">Monitoring Absensi</span>
                        <span class="menu-arrow"></span>
                    </span>
                    <div class="menu-sub menu-sub-accordion {{ $is('admin/attendance*') ? 'show' : '' }}">
                        <div class="menu-item">
                            <a class="menu-link {{ request()->routeIs('attendances.index') ? 'active' : '' }}" href="{{ route('attendances.index') }}">
                                <span class="menu-bullet"><span class="bullet bullet-dot"></span></span>
                                <span class="menu-title">Data Absensi</span>
                            </a>
                        </div>
                        <div class="menu-item">
                            <a class="menu-link {{ request()->routeIs('attendances.by-classroom') ? 'active' : '' }}" href="{{ route('attendances.by-classroom') }}">
                                <span class="menu-bullet"><span class="bullet bullet-dot"></span></span>
                                <span class="menu-title">Rekap per Kelas</span>
                            </a>
                        </div>
                        @if ($can('view_report'))
                            <div class="menu-item">
                                <a class="menu-link {{ request()->routeIs('attendances.recap') ? 'active' : '' }}" href="{{ route('attendances.recap') }}">
                                    <span class="menu-bullet"><span class="bullet bullet-dot"></span></span>
                                    <span class="menu-title">Rekap per Siswa</span>
                                </a>
                            </div>
                        @endif
                        @if ($can('manage_attendance_rule'))
                            <div class="menu-item">
                                <a class="menu-link {{ request()->routeIs('attendance-rules.*') ? 'active' : '' }}" href="{{ route('attendance-rules.index') }}">
                                    <span class="menu-bullet"><span class="bullet bullet-dot"></span></span>
                                    <span class="menu-title">Jam Masuk &amp; Pulang</span>
                                </a>
                            </div>
                        @endif
                    </div>
                </div>
            @endif

            <!--begin::Section: Data Pokok-->
            @if ($can('view_student') || $can('view_classroom') || $can('view_school'))
                <div class="menu-item pt-5">
                    <div class="menu-content"><span class="menu-heading fw-bold text-uppercase fs-7">Data Pokok</span></div>
                </div>

                @if ($can('view_student'))
                    <div class="menu-item">
                        <a class="menu-link {{ $is('admin/students*') ? 'active' : '' }}" href="{{ route('students.index') }}">
                            <span class="menu-icon">
                                <i class="ki-duotone ki-profile-user fs-3"><span class="path1"></span><span class="path2"></span><span class="path3"></span><span class="path4"></span></i>
                            </span>
                            <span class="menu-title">Siswa</span>
                        </a>
                    </div>
                @endif

                @if ($can('view_classroom'))
                    <div class="menu-item">
                        <a class="menu-link {{ $is('admin/classrooms*') ? 'active' : '' }}" href="{{ route('classrooms.index') }}">
                            <span class="menu-icon">
                                <i class="ki-duotone ki-book fs-3"><span class="path1"></span><span class="path2"></span><span class="path3"></span><span class="path4"></span></i>
                            </span>
                            <span class="menu-title">Kelas / Rombel</span>
                        </a>
                    </div>
                @endif

                @if ($can('view_school'))
                    <div class="menu-item">
                        <a class="menu-link {{ $is('admin/schools*') ? 'active' : '' }}" href="{{ route('schools.index') }}">
                            <span class="menu-icon">
                                <i class="ki-duotone ki-bank fs-3"><span class="path1"></span><span class="path2"></span></i>
                            </span>
                            <span class="menu-title">Sekolah</span>
                        </a>
                    </div>
                @endif
            @endif

            <!--begin::Section: Biometrik & Perangkat-->
            @if ($can('view_face_enrollment') || $can('view_device') || $can('operate_face_kiosk'))
                <div class="menu-item pt-5">
                    <div class="menu-content"><span class="menu-heading fw-bold text-uppercase fs-7">Face Recognition</span></div>
                </div>

                {{-- Absensi wajah ditaruh PALING ATAS di bagian ini: itu yang
                     dibuka setiap hari, sementara pendaftaran wajah hanya
                     sekali per siswa. --}}
                @if ($can('operate_face_kiosk'))
                    <div class="menu-item">
                        <a class="menu-link {{ request()->routeIs('biometric.scan') ? 'active' : '' }}" href="{{ route('biometric.scan') }}">
                            <span class="menu-icon">
                                <i class="ki-duotone ki-focus fs-3"><span class="path1"></span><span class="path2"></span><span class="path3"></span></i>
                            </span>
                            <span class="menu-title">Absensi Wajah</span>
                        </a>
                    </div>
                @endif

                @if ($can('view_face_enrollment'))
                    <div class="menu-item">
                        <a class="menu-link {{ $is('admin/biometric*') && ! request()->routeIs('biometric.scan') ? 'active' : '' }}" href="{{ route('biometric.index') }}">
                            <span class="menu-icon">
                                <i class="ki-duotone ki-scan-barcode fs-3"><span class="path1"></span><span class="path2"></span><span class="path3"></span><span class="path4"></span><span class="path5"></span><span class="path6"></span><span class="path7"></span><span class="path8"></span></i>
                            </span>
                            <span class="menu-title">Pendaftaran Wajah</span>
                        </a>
                    </div>
                @endif

                @if ($can('view_device'))
                    <div class="menu-item">
                        <a class="menu-link {{ $is('admin/devices*') ? 'active' : '' }}" href="{{ route('devices.index') }}">
                            <span class="menu-icon">
                                <i class="ki-duotone ki-devices fs-3"><span class="path1"></span><span class="path2"></span><span class="path3"></span><span class="path4"></span><span class="path5"></span></i>
                            </span>
                            <span class="menu-title">Perangkat Tablet</span>
                        </a>
                    </div>
                @endif
            @endif

            <!--begin::Section: Jargon GO (Super Apps)-->
            @if ($can('view_panic_feed') || $can('view_document_submission') || $can('manage_app_account'))
                <div class="menu-item pt-5">
                    <div class="menu-content"><span class="menu-heading fw-bold text-uppercase fs-7">Jargon GO</span></div>
                </div>

                @if ($can('manage_app_account'))
                    <div data-kt-menu-trigger="click" class="menu-item menu-accordion {{ $is('admin/app-accounts*') ? 'here show' : '' }}">
                        <span class="menu-link">
                            <span class="menu-icon">
                                <i class="ki-duotone ki-profile-user fs-3"><span class="path1"></span><span class="path2"></span><span class="path3"></span><span class="path4"></span></i>
                            </span>
                            <span class="menu-title">Akun Aplikasi</span>
                            <span class="menu-arrow"></span>
                        </span>
                        <div class="menu-sub menu-sub-accordion {{ $is('admin/app-accounts*') ? 'show' : '' }}">
                            <div class="menu-item">
                                <a class="menu-link {{ request()->routeIs('app-accounts.index') ? 'active' : '' }}" href="{{ route('app-accounts.index') }}">
                                    <span class="menu-bullet"><span class="bullet bullet-dot"></span></span>
                                    <span class="menu-title">Daftar Akun</span>
                                </a>
                            </div>
                            <div class="menu-item">
                                <a class="menu-link {{ request()->routeIs('app-accounts.create') ? 'active' : '' }}" href="{{ route('app-accounts.create') }}">
                                    <span class="menu-bullet"><span class="bullet bullet-dot"></span></span>
                                    <span class="menu-title">Buat Akun</span>
                                </a>
                            </div>
                            <div class="menu-item">
                                <a class="menu-link {{ request()->routeIs('app-accounts.bulk') ? 'active' : '' }}" href="{{ route('app-accounts.bulk') }}">
                                    <span class="menu-bullet"><span class="bullet bullet-dot"></span></span>
                                    <span class="menu-title">Akun Siswa Massal</span>
                                </a>
                            </div>
                        </div>
                    </div>
                @endif

                @if ($can('view_panic_feed'))
                    <div data-kt-menu-trigger="click" class="menu-item menu-accordion {{ $is('admin/panic*') ? 'here show' : '' }}">
                        <span class="menu-link">
                            <span class="menu-icon">
                                <i class="ki-duotone ki-shield-tick fs-3"><span class="path1"></span><span class="path2"></span></i>
                            </span>
                            <span class="menu-title">Panic Button</span>
                            <span class="menu-arrow"></span>
                        </span>
                        <div class="menu-sub menu-sub-accordion {{ $is('admin/panic*') ? 'show' : '' }}">
                            <div class="menu-item">
                                <a class="menu-link {{ request()->routeIs('panic.index') ? 'active' : '' }}" href="{{ route('panic.index') }}">
                                    <span class="menu-bullet"><span class="bullet bullet-dot"></span></span>
                                    <span class="menu-title">Daftar Pengaduan</span>
                                </a>
                            </div>
                            @if ($can('unmask_panic_report'))
                                <div class="menu-item">
                                    <a class="menu-link {{ request()->routeIs('panic.unmask-logs') ? 'active' : '' }}" href="{{ route('panic.unmask-logs') }}">
                                        <span class="menu-bullet"><span class="bullet bullet-dot"></span></span>
                                        <span class="menu-title">Audit Buka Identitas</span>
                                    </a>
                                </div>
                            @endif
                        </div>
                    </div>
                @endif

                @if ($can('view_document_submission'))
                    <div data-kt-menu-trigger="click" class="menu-item menu-accordion {{ $is('admin/documents*') ? 'here show' : '' }}">
                        <span class="menu-link">
                            <span class="menu-icon">
                                <i class="ki-duotone ki-folder fs-3"><span class="path1"></span><span class="path2"></span></i>
                            </span>
                            <span class="menu-title">Pemberkasan</span>
                            <span class="menu-arrow"></span>
                        </span>
                        <div class="menu-sub menu-sub-accordion {{ $is('admin/documents*') ? 'show' : '' }}">
                            <div class="menu-item">
                                <a class="menu-link {{ request()->routeIs('documents.index') ? 'active' : '' }}" href="{{ route('documents.index') }}">
                                    <span class="menu-bullet"><span class="bullet bullet-dot"></span></span>
                                    <span class="menu-title">Pengajuan Berkas</span>
                                </a>
                            </div>
                            @if ($can('manage_document_type'))
                                <div class="menu-item">
                                    <a class="menu-link {{ request()->routeIs('documents.types') ? 'active' : '' }}" href="{{ route('documents.types') }}">
                                        <span class="menu-bullet"><span class="bullet bullet-dot"></span></span>
                                        <span class="menu-title">Jenis Dokumen</span>
                                    </a>
                                </div>
                            @endif
                        </div>
                    </div>
                @endif
            @endif

            <!--begin::Section: Notifikasi-->
            @if ($can('view_notification'))
                <div class="menu-item pt-5">
                    <div class="menu-content"><span class="menu-heading fw-bold text-uppercase fs-7">Notifikasi</span></div>
                </div>

                <div data-kt-menu-trigger="click" class="menu-item menu-accordion {{ $is('admin/notifications*') ? 'here show' : '' }}">
                    <span class="menu-link">
                        <span class="menu-icon">
                            <i class="ki-duotone ki-notification-status fs-3"><span class="path1"></span><span class="path2"></span><span class="path3"></span><span class="path4"></span></i>
                        </span>
                        <span class="menu-title">Wali Murid</span>
                        <span class="menu-arrow"></span>
                    </span>
                    <div class="menu-sub menu-sub-accordion {{ $is('admin/notifications*') ? 'show' : '' }}">
                        <div class="menu-item">
                            <a class="menu-link {{ request()->routeIs('notifications.index') ? 'active' : '' }}" href="{{ route('notifications.index') }}">
                                <span class="menu-bullet"><span class="bullet bullet-dot"></span></span>
                                <span class="menu-title">Kirim Pesan</span>
                            </a>
                        </div>
                        <div class="menu-item">
                            <a class="menu-link {{ request()->routeIs('notifications.outbox') ? 'active' : '' }}" href="{{ route('notifications.outbox') }}">
                                <span class="menu-bullet"><span class="bullet bullet-dot"></span></span>
                                <span class="menu-title">Riwayat Pengiriman</span>
                            </a>
                        </div>
                        @if ($can('manage_notification_template'))
                            <div class="menu-item">
                                <a class="menu-link {{ request()->routeIs('notifications.templates') ? 'active' : '' }}" href="{{ route('notifications.templates') }}">
                                    <span class="menu-bullet"><span class="bullet bullet-dot"></span></span>
                                    <span class="menu-title">Template Pesan</span>
                                </a>
                            </div>
                        @endif
                    </div>
                </div>
            @endif

            <!--begin::Section: Sistem-->
            @if ($can('view_resources') || $can('view_help') || $can('view_setting'))
                <div class="menu-item pt-5">
                    <div class="menu-content"><span class="menu-heading fw-bold text-uppercase fs-7">Sistem</span></div>
                </div>

                @if ($can('view_resources'))
                    <div data-kt-menu-trigger="click" class="menu-item menu-accordion {{ $is('admin/users*') || $is('admin/roles*') ? 'here show' : '' }}">
                        <span class="menu-link">
                            <span class="menu-icon">
                                <i class="ki-duotone ki-people fs-3"><span class="path1"></span><span class="path2"></span><span class="path3"></span><span class="path4"></span><span class="path5"></span></i>
                            </span>
                            <span class="menu-title">Manajemen Pengguna</span>
                            <span class="menu-arrow"></span>
                        </span>
                        <div class="menu-sub menu-sub-accordion {{ $is('admin/users*') || $is('admin/roles*') ? 'show' : '' }}">
                            <div class="menu-item">
                                <a class="menu-link {{ $is('admin/users*') ? 'active' : '' }}" href="{{ route('users.index') }}">
                                    <span class="menu-bullet"><span class="bullet bullet-dot"></span></span>
                                    <span class="menu-title">Pengguna</span>
                                </a>
                            </div>
                            <div class="menu-item">
                                <a class="menu-link {{ $is('admin/roles*') ? 'active' : '' }}" href="{{ route('roles.index') }}">
                                    <span class="menu-bullet"><span class="bullet bullet-dot"></span></span>
                                    <span class="menu-title">Peran &amp; Izin</span>
                                </a>
                            </div>
                        </div>
                    </div>
                @endif

                @if ($can('view_setting'))
                    <div class="menu-item">
                        <a class="menu-link {{ request()->routeIs('settings.*') ? 'active' : '' }}" href="{{ route('settings.index') }}">
                            <span class="menu-icon">
                                <i class="ki-duotone ki-setting-2 fs-3"><span class="path1"></span><span class="path2"></span></i>
                            </span>
                            <span class="menu-title">Pengaturan</span>
                        </a>
                    </div>
                @endif

                @if ($can('view_help'))
                    <div class="menu-item">
                        <a class="menu-link {{ $is('admin/log-activity*') ? 'active' : '' }}" href="{{ url('admin/log-activity') }}">
                            <span class="menu-icon">
                                <i class="ki-duotone ki-notepad fs-3"><span class="path1"></span><span class="path2"></span><span class="path3"></span><span class="path4"></span><span class="path5"></span></i>
                            </span>
                            <span class="menu-title">Log Aktivitas</span>
                        </a>
                    </div>
                @endif

                <div class="menu-item">
                    <a class="menu-link" href="{{ config('services.absensi_api.docs_url') }}" target="_blank" rel="noopener">
                        <span class="menu-icon">
                            <i class="ki-duotone ki-code fs-3"><span class="path1"></span><span class="path2"></span><span class="path3"></span><span class="path4"></span></i>
                        </span>
                        <span class="menu-title">Dokumentasi API</span>
                        <i class="ki-outline ki-exit-right-corner fs-6 ms-2 text-muted"></i>
                    </a>
                </div>
            @endif

            <!--begin::My Account-->
            <div class="menu-item pt-5">
                <div class="menu-content"><span class="menu-heading fw-bold text-uppercase fs-7">Akun Saya</span></div>
            </div>
            <div data-kt-menu-trigger="click" class="menu-item menu-accordion {{ $is('admin/my-*') || $is('admin/mmy-*') ? 'here show' : '' }}">
                <span class="menu-link">
                    <span class="menu-icon">
                        <i class="ki-duotone ki-profile-circle fs-3"><span class="path1"></span><span class="path2"></span><span class="path3"></span></i>
                    </span>
                    <span class="menu-title">Profil</span>
                    <span class="menu-arrow"></span>
                </span>
                <div class="menu-sub menu-sub-accordion {{ $is('admin/my-*') || $is('admin/mmy-*') ? 'show' : '' }}">
                    <div class="menu-item">
                        <a class="menu-link {{ request()->routeIs('account.index') ? 'active' : '' }}" href="{{ route('account.index') }}">
                            <span class="menu-bullet"><span class="bullet bullet-dot"></span></span>
                            <span class="menu-title">Ringkasan</span>
                        </a>
                    </div>
                    <div class="menu-item">
                        <a class="menu-link {{ $is('admin/my-security*') ? 'active' : '' }}" href="{{ route('my-security.index') }}">
                            <span class="menu-bullet"><span class="bullet bullet-dot"></span></span>
                            <span class="menu-title">Keamanan</span>
                        </a>
                    </div>
                    <div class="menu-item">
                        <a class="menu-link {{ $is('admin/my-activity*') ? 'active' : '' }}" href="{{ route('my-activity.index') }}">
                            <span class="menu-bullet"><span class="bullet bullet-dot"></span></span>
                            <span class="menu-title">Aktivitas</span>
                        </a>
                    </div>
                    <div class="menu-item">
                        <a class="menu-link {{ $is('admin/mmy-login-session*') ? 'active' : '' }}" href="{{ route('my-login-session.index') }}">
                            <span class="menu-bullet"><span class="bullet bullet-dot"></span></span>
                            <span class="menu-title">Sesi Login</span>
                        </a>
                    </div>
                </div>
            </div>

        </div>
    </div>
    <!--end::Sidebar Menu-->

    <!--begin::Sidebar Footer-->
    <div class="px-6 py-5 mt-auto">
        <div class="separator mb-4"></div>
        <div class="d-flex align-items-center">
            @auth
                <div class="symbol symbol-35px me-3">
                    <img alt="Avatar" src="{{ asset('assets/media/avatars/' . (auth()->user()->avatar ?? 'default.png')) }}" />
                </div>
                <div class="d-flex flex-column flex-grow-1">
                    <span class="fw-bold fs-7 text-gray-800">{{ auth()->user()->name }}</span>
                    <span class="text-muted fs-8">{{ auth()->user()->role_label }}</span>
                </div>
            @endauth
        </div>
    </div>
    <!--end::Sidebar Footer-->
</div>
