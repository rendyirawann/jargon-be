# Hentikan seluruh service Jargon GO yang dijalankan dev.bat.
#
# Sasarannya DIBATASI pada proses yang perintahnya menyebut skrip di folder
# ini, beserta anak-anaknya. Pembatasan itu penting: `taskkill /im cargo.exe`
# atau `/im php.exe` akan ikut mematikan pekerjaan lain yang kebetulan
# sedang berjalan — termasuk XAMPP.
#
# Container Docker dihentikan lewat docker compose, bukan dibunuh, supaya
# PostgreSQL sempat menutup datanya dengan rapi.

$ErrorActionPreference = 'Continue'
$root = Split-Path $PSScriptRoot -Parent

Write-Host ''
Write-Host '  Menghentikan Jargon GO' -ForegroundColor Cyan
Write-Host '  ----------------------'

# --- Proses tab (cmd + seluruh keturunannya) --------------------------
$all = Get-CimInstance Win32_Process -ErrorAction SilentlyContinue

$roots = $all | Where-Object {
    $_.Name -eq 'cmd.exe' -and $_.CommandLine -match 'scripts\\run-(db|api|admin|app)\.bat'
}

if (-not $roots) {
    Write-Host '  Tidak ada tab yang sedang berjalan.'
} else {
    # Kumpulkan pohon proses lebih dulu, baru dimatikan dari daun ke akar.
    # Mematikan induk lebih dulu membuat anaknya kehilangan orang tua dan
    # lolos dari penelusuran — cargo/flutter akan tetap hidup memegang port.
    $targets = New-Object System.Collections.Generic.List[object]

    function Add-Tree($pid_) {
        foreach ($c in $all | Where-Object { $_.ParentProcessId -eq $pid_ }) {
            Add-Tree $c.ProcessId
            $targets.Add($c) | Out-Null
        }
    }

    foreach ($r in $roots) {
        Add-Tree $r.ProcessId
        $targets.Add($r) | Out-Null
    }

    foreach ($t in $targets) {
        try {
            Stop-Process -Id $t.ProcessId -Force -ErrorAction Stop
            Write-Host ("  dihentikan: {0} (pid {1})" -f $t.Name, $t.ProcessId)
        } catch {
            # Sudah mati bersama induknya - bukan masalah.
        }
    }
}

# --- Container ---------------------------------------------------------
$compose = Join-Path $root 'docker-compose.yml'
if ((Get-Command docker -ErrorAction SilentlyContinue) -and (Test-Path $compose)) {
    & docker info 2>&1 | Out-Null
    if ($LASTEXITCODE -eq 0) {
        Write-Host '  Menghentikan container...'
        & docker compose -f $compose stop postgres redis 2>&1 | Out-Null
        Write-Host '  container berhenti.'
    }
}

Write-Host ''
Write-Host '  Selesai. Tab yang tersisa boleh ditutup.' -ForegroundColor Green
Write-Host ''
