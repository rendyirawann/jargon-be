<?php

use Illuminate\Support\Facades\Route;

// Import Controller Dashboard
use App\Http\Controllers\Backend\Dashboard\DashboardAdminController; // Sesuaikan jika nama controllernya beda
// Import Controller PROFILE
use App\Http\Controllers\Backend\MyProfile\AccountController;
use App\Http\Controllers\Backend\MyProfile\ProfileController;
use App\Http\Controllers\Backend\MyProfile\SecurityController;
use App\Http\Controllers\Backend\MyProfile\ActivityController;
use App\Http\Controllers\Backend\MyProfile\LoginSessionController;

// Import Controller USER MANAGEMENT
use App\Http\Controllers\Backend\UserManagement\UserController;
use App\Http\Controllers\Backend\UserManagement\RoleController;

// Import Controller HELP/LOG
use App\Http\Controllers\Backend\Help\LogActivityController;
use App\Http\Controllers\Backend\Settings\SettingController;

// Import Controller ABSENSI FACE RECOGNITION
use App\Http\Controllers\Backend\MasterData\SchoolController;
use App\Http\Controllers\Backend\MasterData\ClassroomController;
use App\Http\Controllers\Backend\MasterData\StudentController;
use App\Http\Controllers\Backend\Biometric\FaceEnrollmentController;
use App\Http\Controllers\Backend\Attendance\AttendanceController;
use App\Http\Controllers\Backend\Device\DeviceController;
use App\Http\Controllers\Backend\Notification\NotificationController;

// Import Controller JARGON GO (Super Apps)
use App\Http\Controllers\Backend\Panic\PanicController;
use App\Http\Controllers\Backend\Document\DocumentController;
use App\Http\Controllers\Backend\Account\AppAccountController;
use App\Http\Controllers\Backend\FileProxyController;

/*
|--------------------------------------------------------------------------
| Web Routes
|--------------------------------------------------------------------------
|
| Here is where you can register web routes for your application. These
| routes are loaded by the RouteServiceProvider within a group which
| contains the "web" middleware group. Now create something great!
|
*/

// Halaman Depan (Langsung diarahkan ke Login)
// Halaman Depan (Langsung diarahkan ke Login)
Route::any('/', function () {
    return redirect('/admin/login');
});

Route::any('/dine-sync-pos', function () {
    return redirect('/admin/login');
});



// --- TARUH DEBUG DISINI (DI LUAR MIDDLEWARE AUTH) ---
Route::get('/admin/debug-session', function () {
    $user = auth()->user();

    // Cek manual apakah tabel bans error
    $bannedStatus = 'Tidak dicek';
    $error = null;

    if ($user) {
        try {
            // Kita coba panggil paksa relasi banned-nya
            $bannedStatus = $user->isBanned() ? 'YA TER-BANNED' : 'AMAN';
        } catch (\Exception $e) {
            $bannedStatus = 'ERROR SAAT CEK BANNED: ' . $e->getMessage();
        }
    }

    return [
        'status_login' => $user ? 'SUDAH LOGIN' : 'BELUM LOGIN / SESI HILANG',
        'user_id' => $user?->id,
        'user_name' => $user?->name,
        'session_id' => session()->getId(),
        'driver_session' => config('session.driver'),
        'cek_banned' => $bannedStatus,
    ];
});

// NOTE: Route /login POST dihapus dari sini karena sudah ada di auth.php
// agar tidak bentrok "Route [login] defined twice".

// Group Middleware untuk User yang sudah Login
// Kita tambahkan 'forbid-banned-user' agar user yang di-banned tidak bisa akses
Route::middleware(['auth', 'forbid-banned-user'])->group(function () {

    // --- SHARED ROLE ROUTES (generate-permissions helper, select) ---
    Route::post('/admin/roles/generate-permissions', [RoleController::class, 'generatePermissions'])->name('roles.generate');
    Route::get('/admin/select/role', [RoleController::class, 'select'])->name('role.select');

    // --- DASHBOARD (accessible by ALL authenticated roles) ---
    Route::get('/admin/dashboard', [DashboardAdminController::class, 'index'])->name('dashboard');

    // --- MY ACCOUNT / PROFILE (accessible by ALL authenticated users) ---
    Route::get('/admin/my-account', [AccountController::class, 'index'])->name('account.index');
    Route::get('/admin/my-account/{id}/avatar', [AccountController::class, 'editAvatar'])->name('avatar-edit');
    Route::post('/admin/my-account/{id}/update-avatar', [AccountController::class, 'updateAvatar'])->name('avatar-update');

    Route::resource('/admin/my-profile', ProfileController::class);
    Route::resource('/admin/my-security', SecurityController::class);
    Route::post('/admin/my-security', [SecurityController::class, 'store'])->name('change.password');
    Route::post('/admin/my-security/logout-other-devices', [SecurityController::class, 'logoutOtherDevices'])->name('security.logout-other-devices');

    Route::get('/admin/my-activity', [ActivityController::class, 'index'])->name('my-activity.index');
    Route::get('/admin/mget-my-activity', [ActivityController::class, 'getActivity'])->name('get-my-activity');

    Route::get('/admin/mmy-login-session', [LoginSessionController::class, 'index'])->name('my-login-session.index');
    Route::get('/admin/mget-my-login-session', [LoginSessionController::class, 'getLoginSession'])->name('get-my-login-session');

    // --- SETTINGS (accessible by ALL authenticated users) ---
    Route::get('/admin/settings', [SettingController::class, 'index'])->name('settings.index');
    Route::post('/admin/settings/update', [SettingController::class, 'update'])->name('settings.update');

    // --- DEBUG/CHECK AUTH ---
    Route::get('/admin/check-auth', function () {
        $u = auth()->user();
        return [
            'user' => $u,
            'roles' => $u?->getRoleNames(),
            'permissions' => $u?->getAllPermissions()->pluck('name'),
        ];
    });
    Route::get('/admin/debug-session', function () {
        $user = auth()->user();
        return ['user' => $user?->name, 'roles' => $user?->getRoleNames()];
    });

    // ====================================================
    // RESOURCES (User & Role Mgmt): view_resources — Superadmin only
    // ====================================================
    Route::middleware('can:view_resources')->group(function () {
        Route::resource('/admin/users', UserController::class);
        Route::get('/admin/get-datauser', [UserController::class, 'getDataUsers'])->name('get-users');
        Route::post('/admin/users/mass-delete', [UserController::class, 'massDelete'])->name('users.mass-delete');
        Route::get('/admin/get-user-show-log/{id}', [UserController::class, 'getLoginSession'])->name('get-user-show-log');
        Route::get('/admin/get-user-show-log-activity/{id}', [UserController::class, 'getActivity'])->name('get-user-show-log-activity');
        Route::post('/admin/users/{id}/ban', [UserController::class, 'ban'])->name('users.ban');
        Route::post('/admin/users/{id}/unban', [UserController::class, 'unban'])->name('users.unban');

        Route::resource('/admin/roles', RoleController::class);
        Route::get('/admin/get-datarole', [RoleController::class, 'getDataRoles'])->name('get-datarole');
        Route::post('/admin/roles/mass-delete', [RoleController::class, 'massDelete'])->name('roles.mass-delete');
    });

    // ====================================================
    // ABSENSI FACE RECOGNITION
    //
    // Otorisasi per aksi ditetapkan di dalam masing-masing controller lewat
    // HasMiddleware::middleware() — lebih dekat ke kode yang dijaga sehingga
    // tidak mudah tertinggal saat menambah aksi baru. Penjagaan tenant
    // (sekolah mana yang boleh dilihat) ditangani App\Support\Tenant dan
    // global scope pada model.
    // ====================================================

    // --- Master: Sekolah ---
    Route::get('/admin/schools/data', [SchoolController::class, 'data'])->name('schools.data');
    Route::resource('/admin/schools', SchoolController::class)->names('schools');

    // --- Master: Kelas (rombel) ---
    Route::get('/admin/classrooms', [ClassroomController::class, 'index'])->name('classrooms.index');
    Route::post('/admin/classrooms', [ClassroomController::class, 'store'])->name('classrooms.store');
    Route::put('/admin/classrooms/{classroom}', [ClassroomController::class, 'update'])->name('classrooms.update');
    Route::delete('/admin/classrooms/{classroom}', [ClassroomController::class, 'destroy'])->name('classrooms.destroy');

    // --- Master: Siswa & wali murid ---
    Route::get('/admin/students/data', [StudentController::class, 'data'])->name('students.data');
    Route::get('/admin/students/classrooms', [StudentController::class, 'classroomsBySchool'])->name('students.classrooms');
    Route::resource('/admin/students', StudentController::class)->names('students');
    Route::post('/admin/students/{student}/guardians', [StudentController::class, 'storeGuardian'])->name('students.guardians.store');
    Route::put('/admin/students/{student}/guardians/{guardian}', [StudentController::class, 'updateGuardian'])->name('students.guardians.update');
    Route::delete('/admin/students/{student}/guardians/{guardian}', [StudentController::class, 'destroyGuardian'])->name('students.guardians.destroy');

    // --- Biometrik: pendaftaran wajah ---
    Route::get('/admin/biometric', [FaceEnrollmentController::class, 'index'])->name('biometric.index');
    // Absensi wajah dari browser. Rute LITERAL ini harus berada SEBELUM
    // /admin/biometric/{student}, kalau tidak Laravel akan mencoba
    // mengikat "scan" sebagai model Student dan menghasilkan 404.
    Route::get('/admin/biometric/scan', [FaceEnrollmentController::class, 'scan'])->name('biometric.scan');
    Route::get('/admin/biometric/{student}/capture', [FaceEnrollmentController::class, 'capture'])->name('biometric.capture');
    Route::post('/admin/biometric/{student}', [FaceEnrollmentController::class, 'store'])->name('biometric.store');
    Route::post('/admin/biometric/{student}/batch', [FaceEnrollmentController::class, 'storeBatch'])->name('biometric.store-batch');
    Route::get('/admin/biometric/{student}', [FaceEnrollmentController::class, 'show'])->name('biometric.show');
    Route::delete('/admin/biometric/sample/{enrollment}', [FaceEnrollmentController::class, 'destroy'])->name('biometric.destroy');
    Route::delete('/admin/biometric/{student}/samples', [FaceEnrollmentController::class, 'reset'])->name('biometric.reset');

    // --- Absensi ---
    Route::get('/admin/attendances', [AttendanceController::class, 'index'])->name('attendances.index');
    Route::get('/admin/attendances/data', [AttendanceController::class, 'data'])->name('attendances.data');
    Route::get('/admin/attendances/live', [AttendanceController::class, 'live'])->name('attendances.live');
    Route::get('/admin/attendances/by-classroom', [AttendanceController::class, 'byClassroom'])->name('attendances.by-classroom');
    Route::get('/admin/attendances/recap', [AttendanceController::class, 'recap'])->name('attendances.recap');
    Route::post('/admin/attendances/manual', [AttendanceController::class, 'manual'])->name('attendances.manual');
    Route::post('/admin/attendances/bulk', [AttendanceController::class, 'bulk'])->name('attendances.bulk');
    Route::get('/admin/attendance-rules', [AttendanceController::class, 'rules'])->name('attendance-rules.index');
    Route::post('/admin/attendance-rules', [AttendanceController::class, 'storeRule'])->name('attendance-rules.store');

    // --- Perangkat tablet ---
    Route::get('/admin/devices', [DeviceController::class, 'index'])->name('devices.index');
    Route::post('/admin/devices', [DeviceController::class, 'store'])->name('devices.store');
    Route::get('/admin/devices/{device}', [DeviceController::class, 'show'])->name('devices.show');
    Route::put('/admin/devices/{device}', [DeviceController::class, 'update'])->name('devices.update');
    Route::post('/admin/devices/{device}/pairing-code', [DeviceController::class, 'pairingCode'])->name('devices.pairing-code');
    Route::post('/admin/devices/{device}/revoke', [DeviceController::class, 'revoke'])->name('devices.revoke');
    Route::delete('/admin/devices/{device}', [DeviceController::class, 'destroy'])->name('devices.destroy');

    // --- Notifikasi wali murid ---
    Route::get('/admin/notifications', [NotificationController::class, 'index'])->name('notifications.index');
    Route::get('/admin/notifications/outbox', [NotificationController::class, 'outbox'])->name('notifications.outbox');
    Route::get('/admin/notifications/outbox/data', [NotificationController::class, 'outboxData'])->name('notifications.outbox.data');
    Route::get('/admin/notifications/templates', [NotificationController::class, 'templates'])->name('notifications.templates');
    Route::post('/admin/notifications/templates', [NotificationController::class, 'storeTemplate'])->name('notifications.templates.store');
    Route::post('/admin/notifications/policy', [NotificationController::class, 'updatePolicy'])->name('notifications.policy');
    Route::post('/admin/notifications/send', [NotificationController::class, 'send'])->name('notifications.send');
    Route::post('/admin/notifications/{outbox}/retry', [NotificationController::class, 'retry'])->name('notifications.retry');

    // ====================================================
    // JARGON GO — Panic Button (pengaduan anonim)
    //
    // Identitas pelapor tidak pernah ditampilkan di halaman mana pun.
    // Rute `unmask` adalah satu-satunya jalan membukanya, dan setiap
    // pemakaiannya tercatat permanen di panic_unmask_logs.
    // ====================================================
    Route::get('/admin/panic', [PanicController::class, 'index'])->name('panic.index');
    Route::get('/admin/panic/unmask-logs', [PanicController::class, 'unmaskLogs'])->name('panic.unmask-logs');
    Route::get('/admin/panic/{id}', [PanicController::class, 'show'])->name('panic.show');
    Route::post('/admin/panic/{id}/moderate', [PanicController::class, 'moderate'])->name('panic.moderate');
    Route::post('/admin/panic/{id}/status', [PanicController::class, 'updateStatus'])->name('panic.status');
    Route::post('/admin/panic/{id}/comment', [PanicController::class, 'comment'])->name('panic.comment');
    Route::post('/admin/panic/{id}/unmask', [PanicController::class, 'unmask'])->name('panic.unmask');

    // ====================================================
    // JARGON GO — Pemberkasan kepegawaian
    // ====================================================
    Route::get('/admin/documents', [DocumentController::class, 'index'])->name('documents.index');
    Route::get('/admin/documents/types', [DocumentController::class, 'types'])->name('documents.types');
    Route::post('/admin/documents/types', [DocumentController::class, 'storeType'])->name('documents.types.store');
    Route::get('/admin/documents/{id}', [DocumentController::class, 'show'])->name('documents.show');
    Route::post('/admin/documents/{id}/review', [DocumentController::class, 'review'])->name('documents.review');
    Route::post('/admin/documents/files/{fileId}/review', [DocumentController::class, 'reviewFile'])->name('documents.files.review');

    // ====================================================
    // JARGON GO — Akun aplikasi
    //
    // Pendaftaran akun tidak swalayan: hubungan orang tua-anak hanya bisa
    // diverifikasi sekolah, jadi seluruh pembuatan akun lewat sini.
    // ====================================================
    Route::get('/admin/app-accounts', [AppAccountController::class, 'index'])->name('app-accounts.index');
    Route::get('/admin/app-accounts/create', [AppAccountController::class, 'create'])->name('app-accounts.create');
    Route::post('/admin/app-accounts', [AppAccountController::class, 'store'])->name('app-accounts.store');
    Route::get('/admin/app-accounts/bulk', [AppAccountController::class, 'bulk'])->name('app-accounts.bulk');
    Route::post('/admin/app-accounts/bulk', [AppAccountController::class, 'bulkStore'])->name('app-accounts.bulk.store');
    Route::get('/admin/app-accounts/students', [AppAccountController::class, 'searchStudents'])->name('app-accounts.students');
    Route::get('/admin/app-accounts/{id}', [AppAccountController::class, 'show'])->name('app-accounts.show');
    Route::post('/admin/app-accounts/{id}/children', [AppAccountController::class, 'linkChild'])->name('app-accounts.children.link');
    Route::delete('/admin/app-accounts/{id}/children/{studentId}', [AppAccountController::class, 'unlinkChild'])->name('app-accounts.children.unlink');

    // --- Berkas tersimpan (foto wajah, pemberkasan, lampiran pengaduan) ---
    //
    // Browser tidak memegang access token API, jadi permintaan berkas lewat
    // sini agar token sesi pengguna bisa ditempelkan. Otorisasinya tetap
    // dilakukan API.
    Route::get('/admin/files/{key}', FileProxyController::class)
        ->where('key', '.*')
        ->name('files.show');

    // --- Pemilih sekolah aktif (untuk peran tingkat provinsi) ---
    Route::post('/admin/switch-school', function (\Illuminate\Http\Request $request) {
        $schoolId = $request->input('school_id');
        // Tenant::currentSchoolId() sekaligus memvalidasi hak akses dan
        // menyimpan pilihan ke session.
        \App\Support\Tenant::currentSchoolId($schoolId === null ? 'all' : (string) $schoolId);

        return back();
    })->name('switch-school');

    // ====================================================
    // HELP (Log Activity): view_help — Superadmin, admin
    // ====================================================
    Route::middleware('can:view_help')->group(function () {
        Route::resource('/admin/log-activity', LogActivityController::class);
        Route::get('/admin/get-datalogactivity', [LogActivityController::class, 'getDataLogActivity'])->name('get-datalogactivity');
    });
});

// Load Routes Authentication (Login, Register, Reset Password)
require __DIR__ . '/auth.php';
