@echo off
rem ======================================================================
rem  Tab Admin - dashboard Laravel di :8000.
rem
rem  Dashboard MEMBACA PostgreSQL langsung, tetapi MENULIS lewat API.
rem  Karena itu ia menunggu API siap: tanpa API, layar-layar tulis
rem  (moderasi pengaduan, verifikasi berkas, koreksi absensi) akan
rem  menampilkan "Sesi API tidak tersedia" yang membingungkan.
rem ======================================================================
title Jargon GO - Admin
cd /d "%~dp0..\admin"

where php >nul 2>&1
if errorlevel 1 (
    echo.
    echo   [!] php tidak ditemukan di PATH.
    echo       Tambahkan C:\xampp\php ^(atau folder PHP Anda^) ke PATH.
    echo.
    pause
    exit /b 1
)

if not exist "vendor\autoload.php" (
    echo   vendor/ belum ada - menjalankan composer install
    where composer >nul 2>&1
    if errorlevel 1 (
        echo.
        echo   [!] composer tidak ditemukan. Pasang dari getcomposer.org
        echo.
        pause
        exit /b 1
    )
    composer install
)

if not exist ".env" (
    if exist ".env.example" (
        copy /y ".env.example" ".env" >nul
        php artisan key:generate
    )
)

call "%~dp0wait-port.bat" 8080 API

echo.
echo   Dashboard  http://127.0.0.1:8000/admin/login
echo   Login      superadmin / Superadmin#2026
echo.
echo   JANGAN jalankan `php artisan migrate` - skema dimiliki migrasi sqlx
echo   di jargon-be\api\migrations, dan migrasi Laravel sudah ditandai
echo   selesai oleh seed.
echo.

php artisan serve --host=127.0.0.1 --port=8000
