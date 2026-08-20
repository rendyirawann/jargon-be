@echo off
rem ======================================================================
rem  Tab API - Rust / Axum di :8080, sekaligus penyaji Swagger UI.
rem
rem  Menunggu PostgreSQL lebih dulu: migrasi dijalankan otomatis saat
rem  start, jadi API yang menyala sebelum database siap akan berhenti
rem  dengan galat koneksi.
rem ======================================================================
title Jargon GO - API
cd /d "%~dp0..\api"

where cargo >nul 2>&1
if errorlevel 1 (
    echo.
    echo   [!] cargo tidak ditemukan di PATH.
    echo       Pasang Rust dari https://rustup.rs lalu buka terminal baru.
    echo.
    pause
    exit /b 1
)

rem .env dibuat dan dilengkapi otomatis. Nilai yang sudah diisi sendiri
rem tidak pernah ditimpa - lihat komentar di init-env.ps1.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0init-env.ps1"
if errorlevel 1 (
    echo.
    pause
    exit /b 1
)

call "%~dp0wait-port.bat" 5432 PostgreSQL

echo.
echo   API        http://127.0.0.1:8080
echo   Swagger    http://127.0.0.1:8080/docs
echo   Health     http://127.0.0.1:8080/health
echo.
echo   Migrasi dijalankan otomatis saat start.
echo.

cargo run
