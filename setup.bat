@echo off
setlocal

rem ======================================================================
rem  Jargon GO - penyiapan pertama kali (Windows).
rem
rem  Membuat .env berisi rahasia acak, lalu membangun dan menjalankan
rem  seluruh sistem. Cukup dijalankan SEKALI; berikutnya pakai:
rem
rem      docker compose up -d
rem
rem  Pemakaian:
rem    setup.bat            siapkan .env, bangun, jalankan
rem    setup.bat env        siapkan .env saja, jangan jalankan
rem ======================================================================

set "ROOT=%~dp0"
if "%ROOT:~-1%"=="\" set "ROOT=%ROOT:~0,-1%"

rem Klik-ganda dari Explorer menutup jendela begitu skrip selesai, beserta
rem pesan apa pun yang baru tercetak. Tahan supaya terbaca.
set "HOLD="
echo %cmdcmdline% | find /i "%~nx0" >nul 2>&1
if not errorlevel 1 set "HOLD=1"

echo.
echo   Jargon GO - penyiapan
echo   =====================
echo.

rem ----------------------------------------------------------------------
rem  jargon-fe adalah REPOSITORI TERPISAH dan harus di-clone sebagai folder
rem  sebelah, karena service `web` dibangun dari sana (context: ../jargon-fe).
rem
rem  Diperiksa di sini, bukan dibiarkan gagal saat build: galat bawaan
rem  Docker untuk context yang hilang tidak menyebutkan sama sekali bahwa
rem  yang kurang adalah sebuah repositori.
rem ----------------------------------------------------------------------
if not exist "%ROOT%\..\jargon-fe\pubspec.yaml" (
    echo   [x] Repositori jargon-fe tidak ditemukan di sebelah repositori ini.
    echo.
    echo       Diharapkan: %ROOT%\..\jargon-fe
    echo.
    echo       Clone keduanya berdampingan:
    echo         git clone https://github.com/rendyirawann/jargon-be.git
    echo         git clone https://github.com/rendyirawann/jargon-fe.git
    echo.
    echo       Tidak ingin membangun dari sumber? Pakai image siap pakai:
    echo         docker compose -f docker-compose.prod.yml up -d
    echo.
    goto :fail
)

powershell -NoProfile -ExecutionPolicy Bypass -File "%ROOT%\scripts\init-compose-env.ps1"
if errorlevel 1 goto :fail

if /i "%~1"=="env" goto :done

where docker >nul 2>&1
if errorlevel 1 (
    echo.
    echo   [x] Docker tidak ditemukan. Pasang Docker Desktop lebih dulu:
    echo       https://www.docker.com/products/docker-desktop
    echo.
    goto :fail
)

docker info >nul 2>&1
if errorlevel 1 (
    echo.
    echo   [x] Docker terpasang tetapi daemonnya belum jalan.
    echo       Buka Docker Desktop, tunggu statusnya "Engine running",
    echo       lalu jalankan setup.bat lagi.
    echo.
    goto :fail
)

echo.
echo   Membangun image. Yang pertama memakan 10-20 menit: Rust dan Flutter
echo   dikompilasi dari nol. Build berikutnya jauh lebih cepat karena
echo   layer dependensinya di-cache.
echo.

cd /d "%ROOT%"
docker compose up -d --build
if errorlevel 1 goto :fail

echo.
echo   Menunggu sistem siap...
powershell -NoProfile -ExecutionPolicy Bypass -File "%ROOT%\scripts\wait-http.ps1"

:done
echo.
echo   Selesai.
echo.
echo     Jargon GO   http://localhost/
echo     Dashboard   http://localhost/admin/login
echo     Swagger     http://localhost/docs
echo.
echo     Login pertama: superadmin / Superadmin#2026  ^(ganti segera^)
echo.
echo   Perintah harian:
echo     docker compose up -d          jalankan
echo     docker compose logs -f api    lihat log API
echo     docker compose down           hentikan
echo     docker compose down -v        hentikan + HAPUS seluruh data
echo.
goto :end

:fail
echo.
echo   Penyiapan dihentikan.
echo.

:end
if defined HOLD pause
endlocal
