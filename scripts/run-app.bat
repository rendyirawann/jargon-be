@echo off
rem ======================================================================
rem  Tab Web - aplikasi Jargon GO di browser (:5000).
rem
rem  Menunggu API siap lebih dulu supaya layar login tidak langsung
rem  menampilkan "tidak dapat menghubungi server" pada muatan pertama.
rem
rem  Argumen diteruskan apa adanya ke jargon-fe\run-web.bat:
rem    run-app.bat [port] [alamat-api] [server]
rem ======================================================================
title Jargon GO - Web
cd /d "%~dp0..\..\jargon-fe"

where flutter >nul 2>&1
if errorlevel 1 (
    echo.
    echo   [!] flutter tidak ditemukan di PATH.
    echo       Tambahkan ^<folder-flutter^>\bin ke PATH, lalu buka terminal baru.
    echo.
    pause
    exit /b 1
)

call "%~dp0wait-port.bat" 8080 API

call run-web.bat %*
