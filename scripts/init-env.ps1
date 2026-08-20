# Siapkan api\.env agar API bisa langsung dijalankan.
#
# MENGAPA OTOMATIS
#
# Tiga nilai wajib (DATABASE_URL, JWT_SECRET, SECRETS_KEY_HEX) sebelumnya
# harus disalin sendiri dari terminal ke berkas. Itu langkah manual yang
# mudah salah, dan yang paling sering terjadi bukan lupa — melainkan
# mengisi JWT_SECRET dengan sesuatu yang mudah diketik. JWT_SECRET yang
# bisa ditebak berarti siapa pun dapat menempa token akses untuk peran apa
# pun, termasuk superadmin.
#
# Skrip ini hanya mengisi yang MASIH KOSONG atau masih berupa placeholder.
# Nilai yang sudah Anda isi tidak pernah ditimpa.

param(
    [string]$EnvPath = (Join-Path $PSScriptRoot '..\api\.env'),
    [string]$ExamplePath = (Join-Path $PSScriptRoot '..\api\.env.example')
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path $EnvPath)) {
    if (-not (Test-Path $ExamplePath)) {
        Write-Host "  [x] .env.example tidak ditemukan di $ExamplePath" -ForegroundColor Red
        exit 1
    }
    Copy-Item $ExamplePath $EnvPath
    Write-Host '  .env dibuat dari .env.example'
}

$lines = Get-Content $EnvPath
$changed = @()

function New-RandomBytes([int]$count) {
    $rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
    $b = New-Object byte[] $count
    $rng.GetBytes($b)
    $rng.Dispose()
    return $b
}

function Get-EnvValue([string]$key) {
    foreach ($l in $lines) {
        if ($l -match "^\s*$([regex]::Escape($key))\s*=(.*)$") { return $Matches[1].Trim() }
    }
    return $null
}

function Set-EnvValue([string]$key, [string]$value) {
    $found = $false
    $script:lines = $lines | ForEach-Object {
        if ($_ -match "^\s*$([regex]::Escape($key))\s*=") {
            $found = $true
            "$key=$value"
        } else { $_ }
    }
    if (-not $found) { $script:lines += "$key=$value" }
    $script:changed += $key
}

# --- JWT_SECRET -------------------------------------------------------
# Placeholder bawaan .env.example diawali "GANTI-INI". Panjang minimal 32
# karakter ditegakkan API saat start, jadi nilai pendek ditolak juga.
$jwt = Get-EnvValue 'JWT_SECRET'
if ([string]::IsNullOrWhiteSpace($jwt) -or $jwt -like 'GANTI*' -or $jwt.Length -lt 32) {
    Set-EnvValue 'JWT_SECRET' ([Convert]::ToBase64String((New-RandomBytes 48)))
}

# --- SECRETS_KEY_HEX --------------------------------------------------
# Tepat 32 byte dalam heksadesimal (64 karakter) — panjangnya ditentukan
# algoritma enkripsinya, bukan pilihan bebas. Kunci ini mengenkripsi
# kredensial provider notifikasi di database.
$key = Get-EnvValue 'SECRETS_KEY_HEX'
if ([string]::IsNullOrWhiteSpace($key) -or $key -notmatch '^[0-9a-fA-F]{64}$') {
    $hex = ([BitConverter]::ToString((New-RandomBytes 32))).Replace('-', '').ToLower()
    Set-EnvValue 'SECRETS_KEY_HEX' $hex
}

# --- DATABASE_URL -----------------------------------------------------
# Bawaan .env.example sudah cocok dengan kredensial PostgreSQL di
# docker-compose (absensi/absensi@localhost:5432/absensi), jadi tidak
# diubah — hanya diisi bila benar-benar kosong.
$db = Get-EnvValue 'DATABASE_URL'
if ([string]::IsNullOrWhiteSpace($db)) {
    Set-EnvValue 'DATABASE_URL' 'postgres://absensi:absensi@127.0.0.1:5432/absensi'
}

if ($changed.Count -gt 0) {
    Set-Content -Path $EnvPath -Value $lines -Encoding UTF8
    Write-Host ('  .env dilengkapi otomatis: ' + ($changed -join ', ')) -ForegroundColor Green
} else {
    Write-Host '  .env sudah lengkap.'
}

# Tampilkan alamat database yang akan dipakai — ini nilai yang paling
# sering perlu diubah saat PostgreSQL tidak dijalankan lewat Docker.
Write-Host ('  DATABASE_URL = ' + (Get-EnvValue 'DATABASE_URL')) -ForegroundColor DarkGray
