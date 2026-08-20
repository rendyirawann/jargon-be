# Periksa apakah PostgreSQL siap dipakai Jargon GO — dengan atau tanpa Docker.
#
# Yang diperiksa, berurutan:
#   1. Ada yang mendengarkan di port 5432?
#   2. Bisa login dengan kredensial di DATABASE_URL?
#   3. Ekstensi `vector` (pgvector) tersedia?
#
# Nomor 3 yang paling sering menggagalkan pemasangan tanpa Docker.
# Migrasi 0001 menjalankan CREATE EXTENSION vector, dan tabel
# face_embeddings memakai tipe VECTOR(512) — tanpa pgvector, API berhenti
# saat migrasi dan tidak ada jalan memutar di sisi aplikasi.

param(
    [string]$DbUrl = ''
)

$ErrorActionPreference = 'Continue'

function Test-Port([int]$port) {
    $c = New-Object Net.Sockets.TcpClient
    try { $c.Connect('127.0.0.1', $port); $c.Close(); return $true }
    catch { return $false }
}

Write-Host ''
Write-Host '  Pemeriksaan PostgreSQL' -ForegroundColor Cyan
Write-Host '  ----------------------'

# --- 1. Port ----------------------------------------------------------
if (-not (Test-Port 5432)) {
    Write-Host '  [x] Tidak ada yang mendengarkan di 127.0.0.1:5432' -ForegroundColor Red
    Write-Host ''
    Write-Host '      Pilih SATU cara menjalankan PostgreSQL:'
    Write-Host ''
    Write-Host '      A. Docker (paling mudah, pgvector sudah termasuk)'
    Write-Host '         Buka Docker Desktop, tunggu "Engine running", lalu:'
    Write-Host '         dev.bat'
    Write-Host ''
    Write-Host '      B. PostgreSQL yang dipasang sendiri di Windows'
    Write-Host '         Pasang PostgreSQL 17, lalu pgvector (lihat catatan di'
    Write-Host '         bawah), buat database, lalu:  dev.bat nodb'
    Write-Host ''
    exit 1
}
Write-Host '  [ok] Port 5432 terbuka'

# --- 2. Koneksi -------------------------------------------------------
if (-not $DbUrl) {
    $envPath = Join-Path $PSScriptRoot '..\api\.env'
    if (Test-Path $envPath) {
        foreach ($l in Get-Content $envPath) {
            if ($l -match '^\s*DATABASE_URL\s*=(.*)$') { $DbUrl = $Matches[1].Trim() }
        }
    }
}
if (-not $DbUrl) { $DbUrl = 'postgres://absensi:absensi@127.0.0.1:5432/absensi' }

$psql = (Get-Command psql -ErrorAction SilentlyContinue)
if (-not $psql) {
    # Tanpa psql, port terbuka sudah cukup untuk melanjutkan: API akan
    # melaporkan galatnya sendiri bila kredensial atau pgvector bermasalah.
    Write-Host '  [--] psql tidak ada di PATH - pemeriksaan lanjutan dilewati.'
    Write-Host '       Bila API gagal saat migrasi, penyebab paling sering'
    Write-Host '       adalah ekstensi pgvector belum terpasang.'
    Write-Host ''
    exit 0
}

$env:PGCONNECT_TIMEOUT = '5'
$ping = & psql $DbUrl -tAc 'SELECT 1' 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host '  [x] Tidak bisa login ke database' -ForegroundColor Red
    Write-Host "      DATABASE_URL = $DbUrl"
    Write-Host "      $ping"
    Write-Host ''
    Write-Host '      Bila memakai PostgreSQL sendiri, buat role dan database'
    Write-Host '      yang cocok dengan DATABASE_URL:'
    Write-Host ''
    Write-Host "        psql -U postgres -c ""CREATE ROLE absensi LOGIN PASSWORD 'absensi';"""
    Write-Host '        psql -U postgres -c "CREATE DATABASE absensi OWNER absensi;"'
    Write-Host ''
    exit 1
}
Write-Host '  [ok] Koneksi database berhasil'

# --- 3. pgvector ------------------------------------------------------
$hasVector = & psql $DbUrl -tAc "SELECT 1 FROM pg_extension WHERE extname='vector'" 2>&1
if ($hasVector -match '1') {
    Write-Host '  [ok] Ekstensi pgvector aktif'
    Write-Host ''
    exit 0
}

$available = & psql $DbUrl -tAc "SELECT 1 FROM pg_available_extensions WHERE name='vector'" 2>&1
if ($available -match '1') {
    Write-Host '  [--] pgvector tersedia tetapi belum diaktifkan - mengaktifkan...'
    & psql $DbUrl -c 'CREATE EXTENSION IF NOT EXISTS vector' | Out-Null
    if ($LASTEXITCODE -eq 0) {
        Write-Host '  [ok] Ekstensi pgvector aktif'
        Write-Host ''
        exit 0
    }
}

Write-Host '  [x] Ekstensi pgvector TIDAK terpasang' -ForegroundColor Red
Write-Host ''
Write-Host '      Migrasi 0001 menjalankan CREATE EXTENSION vector dan tabel'
Write-Host '      face_embeddings memakai tipe VECTOR(512). Tanpa pgvector,'
Write-Host '      API berhenti saat migrasi - tidak ada jalan memutar di'
Write-Host '      sisi aplikasi.'
Write-Host ''
Write-Host '      Cara termudah: pakai Docker.' -ForegroundColor Yellow
Write-Host '        Image pgvector/pgvector:pg17 sudah memuatnya, tidak perlu'
Write-Host '        kompilasi apa pun. Buka Docker Desktop lalu jalankan dev.bat'
Write-Host ''
Write-Host '      Bila tetap ingin PostgreSQL sendiri di Windows, pgvector'
Write-Host '      harus dikompilasi - tidak ada installer resminya:'
Write-Host ''
Write-Host '        1. Pasang Visual Studio Build Tools (workload C++)'
Write-Host '        2. Buka "x64 Native Tools Command Prompt for VS"'
Write-Host '        3. set "PGROOT=C:\Program Files\PostgreSQL\17"'
Write-Host '           git clone --branch v0.8.0 https://github.com/pgvector/pgvector.git'
Write-Host '           cd pgvector'
Write-Host '           nmake /F Makefile.win'
Write-Host '           nmake /F Makefile.win install'
Write-Host '        4. psql -U postgres -d absensi -c "CREATE EXTENSION vector;"'
Write-Host ''
exit 1
