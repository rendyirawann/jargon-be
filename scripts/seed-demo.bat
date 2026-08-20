@echo off
rem ======================================================================
rem  Isi database dengan data demo untuk pengujian.
rem
rem  Membuat satu sekolah, dua kelas, lima siswa, riwayat absensi 14 hari,
rem  dan delapan akun (semua peran). Kata sandi seluruh akun demo:
rem  Demo#2026
rem
rem  Aman dijalankan berulang: seluruh INSERT memakai ON CONFLICT dan
rem  pengenal barisnya tetap (NPSN, NISN, NIK), bukan UUID acak.
rem
rem  JANGAN dijalankan di produksi. Sekolah "SMA Negeri 1 Medan (DEMO)"
rem  yang muncul di lingkungan Dinas sebenarnya lebih merepotkan daripada
rem  membantu.
rem ======================================================================
setlocal
cd /d "%~dp0.."

rem Klik-ganda dari Explorer menutup jendela begitu skrip selesai, beserta
rem pesan apa pun yang baru tercetak. Tahan supaya terbaca.
set "HOLD="
echo %cmdcmdline% | find /i "%~nx0" >nul 2>&1
if not errorlevel 1 set "HOLD=1"

rem Diperiksa dengan MENJALANKAN sesuatu di dalam container, bukan dengan
rem mencocokkan teks keluaran `docker compose ps`. Keluaran itu berbunyi
rem "Up 2 minutes", bukan "running", sehingga pencocokan teks memberi
rem negatif palsu dan formatnya bisa berubah antar-versi compose.
docker compose exec -T postgres true >nul 2>&1
if errorlevel 1 (
    echo.
    echo   [x] Container postgres tidak berjalan.
    echo       Jalankan dulu:  docker compose up -d
    echo.
    if defined HOLD pause
    exit /b 1
)

echo.
echo   Mengisi data demo...
echo.

docker compose exec -T postgres psql -U absensi -d absensi -v ON_ERROR_STOP=1 < api\seeds\demo.sql
if errorlevel 1 (
    echo.
    echo   [x] Seed gagal - lihat pesan di atas.
    echo.
    if defined HOLD pause
    exit /b 1
)

echo.
echo   Selesai. Akun demo ^(kata sandi: Demo#2026^):
echo.
echo     Peran               Login
echo     -----------------   ----------------
echo     Superadmin          superadmin          ^(kata sandi Superadmin#2026^)
echo     Kepala Sekolah      1275010000000001
echo     Guru                1275010000000002
echo     Staff TU            1275010000000003
echo     Petugas Pengaduan   1275020000000001
echo     Admin Dinas         1275020000000002
echo     Siswa               0071234501
echo     Orang Tua           1275030000000001
echo.
echo   Siswa login pakai NISN, peran lain pakai NIK.
echo.
if defined HOLD pause
endlocal
