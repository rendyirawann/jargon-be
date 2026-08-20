@echo off
setlocal enabledelayedexpansion

rem ======================================================================
rem  Jargon GO - jalankan SEMUANYA dengan satu perintah.
rem
rem  Membuka satu jendela Windows Terminal berisi tab-tab:
rem
rem    DB     PostgreSQL + Redis (docker compose)      :5432 :6379
rem    API    Rust / Axum + Swagger UI                 :8080
rem    Admin  Dashboard Laravel                        :8000
rem    Web    Aplikasi Jargon GO di browser            :5000
rem    Buka   membuka Swagger + dashboard, lalu tutup sendiri
rem
rem  Keempatnya TIDAK bisa disatukan dalam satu tab: masing-masing adalah
rem  proses yang terus berjalan dan menampilkan lognya sendiri. Menjejalkan
rem  semuanya ke satu jendela berarti log yang saling menimpa dan tidak ada
rem  cara menghentikan satu service tanpa mematikan yang lain.
rem
rem  Tiap tab menunggu apa yang dibutuhkannya (API menunggu PostgreSQL,
rem  Admin dan Web menunggu API), jadi urutan start tidak perlu diatur.
rem
rem  Pemakaian (boleh digabung):
rem    dev.bat                  semua
rem    dev.bat noweb            tanpa tab Web (sedang pakai emulator Android)
rem    dev.bat nodb             tanpa tab DB (PostgreSQL dijalankan sendiri)
rem    dev.bat nodb noweb       keduanya
rem    dev.bat web              HANYA tab Web
rem    dev.bat stop             hentikan semua yang sedang berjalan
rem    dev.bat dryrun           cetak baris perintah wt, jangan jalankan
rem ======================================================================

set "ROOT=%~dp0"
if "%ROOT:~-1%"=="\" set "ROOT=%ROOT:~0,-1%"

rem ----------------------------------------------------------------------
rem  Deteksi klik-ganda dari Explorer.
rem
rem  Saat diklik-ganda, cmd dijalankan dengan /c sehingga jendelanya tertutup
rem  begitu skrip selesai - beserta seluruh pesan yang baru saja tercetak.
rem  Itulah sebabnya kegagalan apa pun terlihat seperti "jendela langsung
rem  tertutup, tidak terjadi apa-apa". Bila terdeteksi, jendela ditahan di
rem  akhir supaya pesannya sempat dibaca.
rem ----------------------------------------------------------------------
set "HOLD="
echo %cmdcmdline% | find /i "%~nx0" >nul 2>&1
if not errorlevel 1 set "HOLD=1"

rem ----------------------------------------------------------------------
rem  Argumen dibaca sebagai daftar, bukan hanya %1, supaya `nodb noweb`
rem  bekerja. Menerima satu argumen saja akan membuat kombinasi yang wajar
rem  gagal tanpa pesan apa pun.
rem ----------------------------------------------------------------------
set "OPT_NODB="
set "OPT_NOWEB="
set "OPT_WEBONLY="
set "OPT_DRYRUN="
set "OPT_BAD="

set "OPT_STOP="

:parse
if "%~1"=="" goto :parsed
if /i "%~1"=="nodb"   set "OPT_NODB=1"   & shift & goto :parse
if /i "%~1"=="noweb"  set "OPT_NOWEB=1"  & shift & goto :parse
if /i "%~1"=="web"    set "OPT_WEBONLY=1" & shift & goto :parse
if /i "%~1"=="stop"   set "OPT_STOP=1"   & shift & goto :parse
if /i "%~1"=="dryrun" set "OPT_DRYRUN=1" & shift & goto :parse
set "OPT_BAD=%~1"
goto :parsed

:parsed
if defined OPT_BAD (
    echo   [x] Argumen tidak dikenal: %OPT_BAD%
    echo       Pilihan: nodb, noweb, web, stop, dryrun
    echo.
    goto :end
)

if defined OPT_STOP (
    powershell -NoProfile -ExecutionPolicy Bypass -File "%ROOT%\scripts\stop.ps1"
    goto :end
)

rem Port dibuat TETAP, bukan acak: alamat ini muncul di CORS_ALLOWED_ORIGINS
rem milik API dan di setelan Alamat Server aplikasi. Port yang berganti tiap
rem run berarti CORS gagal setiap kali.
set "WEB_PORT=5000"

echo.
echo   Jargon GO - lingkungan pengembangan
echo   ===================================
echo.

rem ----------------------------------------------------------------------
rem  Pemeriksaan awal. Yang hilang dilaporkan SEKARANG, bukan sebagai tab
rem  yang mati beberapa detik kemudian di balik tab lain.
rem ----------------------------------------------------------------------
set "MISSING="
call :need cargo   "Rust (https://rustup.rs)"
call :need php     "PHP (tambahkan C:\xampp\php ke PATH)"
call :need flutter "Flutter (tambahkan <flutter>\bin ke PATH)"

if defined MISSING (
    echo.
    echo   Perkakas di atas belum ada di PATH. Tab yang membutuhkannya akan
    echo   berhenti dengan pesan yang menjelaskan, jadi sisanya tetap bisa
    echo   dipakai.
    echo.
)

where wt.exe >nul 2>&1
if errorlevel 1 goto :manual

if defined OPT_WEBONLY goto :webonly

rem ----------------------------------------------------------------------
rem  Lewati tab DB bila sudah ada PostgreSQL yang mendengarkan di 5432.
rem  Menjalankan docker compose di atasnya hanya menghasilkan galat "port
rem  is already allocated" yang menyesatkan.
rem ----------------------------------------------------------------------
set "DB_TAB=1"
if defined OPT_NODB set "DB_TAB="

if defined DB_TAB (
    powershell -NoProfile -ExecutionPolicy Bypass -Command ^
      "$c=New-Object Net.Sockets.TcpClient; try{$c.Connect('127.0.0.1',5432);$c.Close();exit 0}catch{exit 1}" >nul 2>&1
    if not errorlevel 1 (
        set "DB_TAB="
        echo   PostgreSQL sudah berjalan di :5432 - tab DB dilewati.
    )
)

set "WEB_TAB=1"
if defined OPT_NOWEB set "WEB_TAB="

rem ----------------------------------------------------------------------
rem  Susun baris perintah wt.
rem
rem  Direktori kerja diatur lewat -d sehingga path skripnya relatif dan
rem  tidak butuh tanda kutip bersarang - sumber kerusakan paling sering
rem  pada baris perintah wt.
rem ----------------------------------------------------------------------
set "CMD="
if defined DB_TAB set "CMD=new-tab --title DB -d "%ROOT%" cmd /k scripts\run-db.bat ;"

set "CMD=!CMD! new-tab --title API -d "%ROOT%" cmd /k scripts\run-api.bat ;"
set "CMD=!CMD! new-tab --title Admin -d "%ROOT%" cmd /k scripts\run-admin.bat ;"

if defined WEB_TAB set "CMD=!CMD! new-tab --title Web -d "%ROOT%" cmd /k scripts\run-app.bat %WEB_PORT% ;"

set "CMD=!CMD! new-tab --title Buka -d "%ROOT%" cmd /c scripts\open-urls.bat"

if defined OPT_DRYRUN (
    echo.
    echo   wt.exe !CMD!
    echo.
    goto :end
)

echo   Membuka Windows Terminal...
start "" wt.exe !CMD!

rem `start` tidak melaporkan kegagalan wt. Tanpa pemeriksaan ini, baris
rem perintah yang salah akan terlihat persis seperti "tidak terjadi
rem apa-apa" - jendela ini tertutup dan tidak ada tab yang muncul.
call :verify run-api.bat
if errorlevel 1 goto :wtfail
goto :info

:webonly
set "CMD=new-tab --title Web -d "%ROOT%" cmd /k scripts\run-app.bat %WEB_PORT%"
if defined OPT_DRYRUN (
    echo.
    echo   wt.exe !CMD!
    echo.
    goto :end
)
start "" wt.exe !CMD!
call :verify run-app.bat
if errorlevel 1 goto :wtfail
goto :info

:info
echo.
echo   Jargon GO   http://127.0.0.1:%WEB_PORT%
echo   Swagger     http://127.0.0.1:8080/docs
echo   Dashboard   http://127.0.0.1:8000/admin/login
echo.
echo   Login pertama: superadmin / Superadmin#2026  ^(ganti segera^)
echo.
echo   Tab Buka akan membuka Swagger dan dashboard sendiri setelah siap.
echo   Aplikasi Jargon GO dibuka Flutter di Brave.
echo.
echo   Menghentikan semuanya:  dev.bat stop
echo.
goto :end

:wtfail
echo.
echo   [x] Windows Terminal terbuka, tetapi tabnya tidak jalan.
echo.
echo       Coba jalankan satu per satu untuk melihat pesan galatnya:
echo         scripts\run-db.bat
echo         scripts\run-api.bat
echo         scripts\run-admin.bat
echo         scripts\run-app.bat
echo.
goto :end

:manual
echo   [x] Windows Terminal ^(wt.exe^) tidak ditemukan.
echo       Pasang dari Microsoft Store agar semuanya muat dalam satu jendela.
echo.
echo       Sementara itu, jalankan empat perintah ini di empat jendela:
echo.
echo         1^)  scripts\run-db.bat
echo         2^)  scripts\run-api.bat
echo         3^)  scripts\run-admin.bat
echo         4^)  scripts\run-app.bat
echo.
echo       Semuanya relatif terhadap "%ROOT%".
echo       Penjelasan tiap perintah ada di dev-manual.txt
echo.
goto :end

rem ----------------------------------------------------------------------
rem  Sengaja memakai goto, bukan `if ... ( ... ) else ( ... )`.
rem
rem  Isi %~2 memuat tanda kurung ("Rust (https://rustup.rs)"). Di dalam blok
rem  berkurung, cmd mengganti %~2 pada saat PARSE - kurung tutup di dalamnya
rem  menutup blok lebih awal, sehingga baris sesudahnya ikut dijalankan tanpa
rem  syarat. Akibatnya MISSING selalu terisi dan peringatan muncul walau
rem  semua perkakas sebenarnya ada.
rem ----------------------------------------------------------------------
:need
where %~1 >nul 2>&1
if not errorlevel 1 goto :need_ok
echo   [x] %~1 tidak ada di PATH - %~2
set "MISSING=1"
exit /b 0

:need_ok
echo   [ok] %~1
exit /b 0

rem ----------------------------------------------------------------------
rem  Tunggu sampai tab yang diminta benar-benar punya proses berjalan.
rem ----------------------------------------------------------------------
:verify
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "$d=(Get-Date).AddSeconds(12);" ^
  "while((Get-Date) -lt $d){" ^
  "  $p = Get-CimInstance Win32_Process -Filter \"Name='cmd.exe'\" -EA SilentlyContinue |" ^
  "       Where-Object { $_.CommandLine -like '*%~1*' };" ^
  "  if ($p) { exit 0 }; Start-Sleep -Milliseconds 500" ^
  "}; exit 1" >nul 2>&1
exit /b %errorlevel%

:end
rem Bila skrip diklik-ganda dari Explorer, jendela ini tertutup begitu
rem skrip selesai - beserta seluruh pesan di atas. Tahan supaya terbaca.
if defined HOLD (
    echo.
    pause
)
endlocal
