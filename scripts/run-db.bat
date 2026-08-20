@echo off
rem ======================================================================
rem  Tab DB - PostgreSQL (pgvector) + Redis.
rem
rem  PostgreSQL WAJIB punya ekstensi pgvector: migrasi 0001 menjalankan
rem  CREATE EXTENSION vector, dan tabel face_embeddings memakai tipe
rem  VECTOR(512). Tanpa itu API berhenti saat migrasi.
rem
rem  Di Windows, pgvector tidak punya installer resmi - harus dikompilasi
rem  dengan Visual Studio Build Tools. Karena itu Docker dipakai sebagai
rem  jalur bawaan: image pgvector/pgvector:pg17 sudah memuatnya.
rem
rem  Sudah punya PostgreSQL sendiri? Jalankan `dev.bat nodb` - tab ini
rem  tidak akan dibuka, dan dev.bat juga melewatinya otomatis bila port
rem  5432 sudah terpakai.
rem ======================================================================
title Jargon GO - DB
cd /d "%~dp0.."

rem PostgreSQL sudah jalan sendiri? Jangan tumpuk dengan container -
rem hasilnya hanya galat "port is already allocated" yang menyesatkan.
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "$c=New-Object Net.Sockets.TcpClient; try{$c.Connect('127.0.0.1',5432);$c.Close();exit 0}catch{exit 1}" >nul 2>&1
if not errorlevel 1 goto :already

where docker >nul 2>&1
if errorlevel 1 goto :nodocker

docker info >nul 2>&1
if errorlevel 1 goto :daemonoff

echo.
echo   PostgreSQL :5432   Redis :6379
echo   Ctrl+C untuk menghentikan.
echo.

docker compose -f infra/docker-compose.yml up postgres redis
goto :eof

rem ----------------------------------------------------------------------
:already
echo.
echo   PostgreSQL sudah berjalan di :5432 - container tidak dijalankan.
echo.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0check-db.ps1"
echo.
echo   Tab ini boleh ditutup. Lain kali pakai:  dev.bat nodb
echo.
pause
goto :eof

rem ----------------------------------------------------------------------
:daemonoff
echo.
echo   [!] Docker terpasang tetapi daemonnya belum jalan.
echo.
echo   Buka Docker Desktop, tunggu statusnya "Engine running", lalu tekan
echo   panah-atas + Enter di tab ini.
echo.
echo   Tidak ingin memakai Docker? Lihat pilihan di bawah.
echo.
call :nodocker_info
pause
goto :eof

rem ----------------------------------------------------------------------
:nodocker
echo.
echo   [!] Docker tidak ditemukan di PATH.
echo.
call :nodocker_info
pause
goto :eof

rem ----------------------------------------------------------------------
:nodocker_info
echo   MENJALANKAN POSTGRESQL TANPA DOCKER
echo   -----------------------------------
echo   1. Pasang PostgreSQL 17 untuk Windows dari enterprisedb.com
echo.
echo   2. Pasang pgvector. Ini bagian yang merepotkan: tidak ada installer
echo      resminya di Windows, jadi harus dikompilasi.
echo      Buka "x64 Native Tools Command Prompt for VS" lalu:
echo.
echo         set "PGROOT=C:\Program Files\PostgreSQL\17"
echo         git clone --branch v0.8.0 https://github.com/pgvector/pgvector.git
echo         cd pgvector
echo         nmake /F Makefile.win
echo         nmake /F Makefile.win install
echo.
echo   3. Buat role dan database:
echo         psql -U postgres -c "CREATE ROLE absensi LOGIN PASSWORD 'absensi';"
echo         psql -U postgres -c "CREATE DATABASE absensi OWNER absensi;"
echo         psql -U postgres -d absensi -c "CREATE EXTENSION vector;"
echo.
echo   4. Redis boleh dilewati saat pengembangan - yang mati hanyalah rate
echo      limit dan anti-replay nonce. Kosongkan REDIS_URL di
echo      api\.env bila tidak dipakai.
echo.
echo   5. Jalankan:  dev.bat nodb
echo.
echo   Periksa kesiapannya kapan saja dengan:
echo         powershell -File scripts\check-db.ps1
echo.
exit /b 0
