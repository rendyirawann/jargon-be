<?php

use App\Http\Controllers\Auth\AuthenticatedSessionController;
use App\Http\Controllers\Auth\ConfirmablePasswordController;
use App\Http\Controllers\Auth\EmailVerificationNotificationController;
use App\Http\Controllers\Auth\EmailVerificationPromptController;
use App\Http\Controllers\Auth\NewPasswordController;
use App\Http\Controllers\Auth\PasswordController;
use App\Http\Controllers\Auth\PasswordResetLinkController;
use App\Http\Controllers\Auth\RegisteredUserController;
use App\Http\Controllers\Auth\VerifyEmailController;
use App\Http\Controllers\Auth\SocialLoginController;
use Illuminate\Support\Facades\Route;

Route::middleware('guest')->group(function () {
    // --- REGISTER: DIMATIKAN 21 Agustus 2026 (pemasangan beoulve-dev) ---
    //
    // Alasan: RegisteredUserController::store membuat pengguna TANPA peran lalu
    // langsung Auth::login. Di sistem absensi dinas, akun hanya boleh dibuat
    // admin dari dashboard (/admin/users).
    //
    // Tidak ada fitur berfungsi yang hilang: viewnya sendiri rusak — memakai
    // komponen Breeze (<x-input-label> dsb.) yang tidak ada di repo ini dan
    // berisi sisa view lain (@endpush/@endsection tanpa pembuka), sehingga
    // GET /admin/register selalu 500. Viewnya dipindahkan ke
    // /var/www/html/face-recognition/disabled-views/ .
    //
    // Catatan: URI POST di bawah ditulis 'register' (tanpa /admin/), jadi dulu
    // ia berada di /register — bukan di /admin/register seperti GET-nya.
    //
    // Untuk mengaktifkan kembali: lepas komentar dua rute di bawah, kembalikan
    // + perbaiki viewnya, DAN hapus baris
    // `location = /jargon-be/admin/register { return 404; }`
    // di /etc/nginx/sites-available/beoulve-dev.conf.
    //
    // Route::get('/admin/register', [RegisteredUserController::class, 'create'])
    //     ->name('register');
    //
    // Route::post('register', [RegisteredUserController::class, 'store']);

    // --- LOGIN ---
    Route::get('/admin/login', [AuthenticatedSessionController::class, 'create'])
        ->name('login');

    // PENTING: Saya tambahkan middleware throttle (Limit 5x percobaan per menit)
    // Ini menggantikan fungsi yang tadi dihapus di web.php
    Route::post('/admin/login', [AuthenticatedSessionController::class, 'store'])
        ->middleware('throttle:3,1');

    // --- FORGOT PASSWORD ---
    Route::get('/admin/forgot-password', [PasswordResetLinkController::class, 'create'])
        ->name('password.request');

    Route::post('/admin/forgot-password', [PasswordResetLinkController::class, 'store'])
        ->name('password.email');

    // --- RESET PASSWORD ---
    Route::get('/admin/reset-password/{token}', [NewPasswordController::class, 'create'])
        ->name('password.reset');

    Route::post('/admin/reset-password', [NewPasswordController::class, 'store'])
        ->name('password.store');

    // --- SOCIAL LOGIN ---
    Route::get('/admin/auth/{provider}/redirect', [SocialLoginController::class, 'redirect'])
        ->name('social.redirect');
    Route::get('/admin/auth/{provider}/callback', [SocialLoginController::class, 'callback'])
        ->name('social.callback');
});

Route::middleware('auth')->group(function () {
    // --- VERIFY EMAIL ---
    Route::get('/admin/verify-email', EmailVerificationPromptController::class)
        ->name('verification.notice');

    Route::get('/admin/verify-email/{id}/{hash}', VerifyEmailController::class)
        ->middleware(['signed', 'throttle:6,1'])
        ->name('verification.verify');

    Route::post('/admin/email/verification-notification', [EmailVerificationNotificationController::class, 'store'])
        ->middleware('throttle:6,1')
        ->name('verification.send');

    // --- CONFIRM PASSWORD ---
    Route::get('/admin/confirm-password', [ConfirmablePasswordController::class, 'show'])
        ->name('password.confirm');

    Route::post('/admin/confirm-password', [ConfirmablePasswordController::class, 'store']);

    // --- UPDATE PASSWORD ---
    Route::put('/admin/password', [PasswordController::class, 'update'])->name('password.update');

    // --- LOGOUT ---
    Route::post('/admin/logout', [AuthenticatedSessionController::class, 'destroy'])
        ->name('logout');
});
