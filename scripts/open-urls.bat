@echo off
rem ======================================================================
rem  Buka Swagger dan dashboard di browser setelah servicenya siap.
rem
rem  Tab ini menutup dirinya sendiri setelah selesai (dipanggil dengan
rem  cmd /c dari dev.bat), jadi tidak menyisakan tab kosong.
rem
rem  Aplikasi Jargon GO TIDAK dibuka di sini - Flutter membuka tabnya
rem  sendiri. Membukanya dua kali hanya menghasilkan satu tab mati yang
rem  memuat sebelum bundelnya jadi.
rem ======================================================================
setlocal

set "BRAVE=%ProgramFiles%\BraveSoftware\Brave-Browser\Application\brave.exe"
if not exist "%BRAVE%" set "BRAVE=%ProgramFiles(x86)%\BraveSoftware\Brave-Browser\Application\brave.exe"
if not exist "%BRAVE%" set "BRAVE=%LOCALAPPDATA%\BraveSoftware\Brave-Browser\Application\brave.exe"

call "%~dp0wait-port.bat" 8080 API 120
if errorlevel 1 goto :skip

call "%~dp0wait-port.bat" 8000 Dashboard 60

echo.
echo   Membuka Swagger dan dashboard...

if exist "%BRAVE%" (
    start "" "%BRAVE%" "http://127.0.0.1:8080/docs" "http://127.0.0.1:8000/admin/login"
) else (
    rem Tanda kutip kosong pertama adalah judul jendela - tanpa itu, `start`
    rem menganggap URL sebagai judul dan tidak membuka apa pun.
    start "" "http://127.0.0.1:8080/docs"
    start "" "http://127.0.0.1:8000/admin/login"
)

timeout /t 2 >nul
goto :eof

:skip
echo.
echo   [!] API tidak kunjung siap - browser tidak dibuka.
echo       Periksa tab API dan DB.
echo.
timeout /t 8 >nul
endlocal
