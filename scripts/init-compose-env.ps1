# Siapkan .env di akar repositori untuk docker compose.
#
# Sama semangatnya dengan init-env.ps1 (yang menyiapkan .env API untuk
# pengembangan lokal): rahasia DIBANGKITKAN, bukan diminta diketik.
#
# Yang paling sering terjadi bukan lupa mengisi JWT_SECRET, melainkan
# mengisinya dengan sesuatu yang mudah diketik. JWT_SECRET yang bisa
# ditebak berarti siapa pun dapat menempa token akses untuk peran apa pun,
# termasuk superadmin — pada pemasangan siapa pun yang memakai nilai itu.
#
# Karena itu pula compose sengaja BERHENTI bila rahasianya kosong,
# alih-alih memakai nilai bawaan: nilai bawaan yang ikut tersebar bersama
# repositori sama saja dengan tidak ada kunci sama sekali.

param(
    [string]$EnvPath = (Join-Path $PSScriptRoot '..\.env'),
    [string]$ExamplePath = (Join-Path $PSScriptRoot '..\.env.example')
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path $ExamplePath)) {
    Write-Host "  [x] .env.example tidak ditemukan di $ExamplePath" -ForegroundColor Red
    exit 1
}

if (-not (Test-Path $EnvPath)) {
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

# JWT_SECRET — minimal 32 karakter; API menolak yang lebih pendek.
$jwt = Get-EnvValue 'JWT_SECRET'
if ([string]::IsNullOrWhiteSpace($jwt) -or $jwt.Length -lt 32) {
    Set-EnvValue 'JWT_SECRET' ([Convert]::ToBase64String((New-RandomBytes 48)))
}

# SECRETS_KEY_HEX — tepat 32 byte heksadesimal.
$key = Get-EnvValue 'SECRETS_KEY_HEX'
if ([string]::IsNullOrWhiteSpace($key) -or $key -notmatch '^[0-9a-fA-F]{64}$') {
    Set-EnvValue 'SECRETS_KEY_HEX' (([BitConverter]::ToString((New-RandomBytes 32))).Replace('-', '').ToLower())
}

# APP_KEY — kunci enkripsi cookie sesi Laravel. Diisi di sini supaya sesi
# login bertahan antar-restart; bila dibiarkan kosong, container membuat
# kunci baru setiap start dan semua orang ter-logout.
$appKey = Get-EnvValue 'APP_KEY'
if ([string]::IsNullOrWhiteSpace($appKey)) {
    Set-EnvValue 'APP_KEY' ('base64:' + [Convert]::ToBase64String((New-RandomBytes 32)))
}

if ($changed.Count -gt 0) {
    Set-Content -Path $EnvPath -Value $lines -Encoding UTF8
    Write-Host ('  Rahasia dibangkitkan: ' + ($changed -join ', ')) -ForegroundColor Green
} else {
    Write-Host '  .env sudah lengkap - tidak ada yang diubah.'
}

# Peringatkan bila port yang dipilih sudah dipakai. Tanpa ini, `docker
# compose up` gagal dengan "port is already allocated" yang tidak
# menyebutkan siapa pemakainya.
function Test-Port([int]$p) {
    $c = New-Object Net.Sockets.TcpClient
    try { $c.Connect('127.0.0.1', $p); $c.Close(); return $true } catch { return $false }
}

$httpPort = [int](Get-EnvValue 'HTTP_PORT'); if (-not $httpPort) { $httpPort = 80 }
$pgPort   = [int](Get-EnvValue 'POSTGRES_PORT'); if (-not $pgPort) { $pgPort = 5432 }

if (Test-Port $httpPort) {
    Write-Host "  [!] Port $httpPort sudah dipakai proses lain." -ForegroundColor Yellow
    Write-Host "      Setel HTTP_PORT di .env ke port lain, mis. 8090."
}
if (Test-Port $pgPort) {
    Write-Host "  [!] Port $pgPort sudah dipakai - kemungkinan PostgreSQL Anda sendiri." -ForegroundColor Yellow
    Write-Host "      Setel POSTGRES_PORT di .env ke port lain, mis. 55432."
    Write-Host "      Itu hanya mengubah port di HOST; di dalam jaringan container"
    Write-Host "      API tetap memanggil postgres:5432."
}

Write-Host ''
