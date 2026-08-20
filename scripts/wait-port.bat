@echo off
rem ======================================================================
rem  wait-port.bat <port> [label] [detik]
rem
rem  Tunggu sampai ada yang mendengarkan di 127.0.0.1:<port>.
rem
rem  Dipakai supaya tab API tidak langsung mati ketika database belum
rem  selesai menyala. Tanpa ini, urutan start antar-tab menjadi taruhan:
rem  cargo run yang menang cepat akan berhenti dengan galat koneksi, dan
rem  operator harus tahu sendiri bahwa tinggal menjalankannya ulang.
rem
rem  Penantian dibatasi waktu, lalu tetap lanjut. Menunggu selamanya akan
rem  menyembunyikan penyebab sebenarnya (mis. Docker tidak berjalan);
rem  lebih baik perintahnya jalan dan mencetak galatnya sendiri.
rem ======================================================================
setlocal

set "PORT=%~1"
set "LABEL=%~2"
if "%LABEL%"=="" set "LABEL=port %PORT%"
set "LIMIT=%~3"
if "%LIMIT%"=="" set "LIMIT=90"

rem Cek cepat sekali dulu: kalau sudah siap, tidak perlu mencetak apa pun.
call :probe
if not errorlevel 1 goto :ready

echo   Menunggu %LABEL% di 127.0.0.1:%PORT% ^(maksimal %LIMIT% detik^)...

powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "$deadline = (Get-Date).AddSeconds(%LIMIT%);" ^
  "while ((Get-Date) -lt $deadline) {" ^
  "  $c = New-Object Net.Sockets.TcpClient;" ^
  "  try { $c.Connect('127.0.0.1', %PORT%); $c.Close(); exit 0 }" ^
  "  catch { Start-Sleep -Milliseconds 800 }" ^
  "}; exit 1"

if errorlevel 1 (
    echo   [!] %LABEL% belum siap setelah %LIMIT% detik - dilanjutkan saja.
    echo       Bila perintah berikut gagal, periksa tab DB lebih dulu.
    echo.
    endlocal & exit /b 1
)

:ready
echo   %LABEL% siap.
endlocal & exit /b 0

:probe
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "$c = New-Object Net.Sockets.TcpClient;" ^
  "try { $c.Connect('127.0.0.1', %PORT%); $c.Close(); exit 0 } catch { exit 1 }"
exit /b %errorlevel%
