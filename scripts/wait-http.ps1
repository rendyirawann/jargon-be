# Tunggu sampai sistem benar-benar melayani, bukan sekadar container
# "running".
#
# `docker compose up -d` kembali segera setelah container dijalankan,
# padahal API masih menjalankan migrasi dan dashboard masih menyusun cache.
# Tanpa penantian ini, pesan "Selesai" muncul sementara membuka
# http://localhost masih menghasilkan 502 — yang terlihat seperti gagal.

param(
    [int]$Port = 0,
    [int]$TimeoutSeconds = 180
)

if (-not $Port) {
    $envPath = Join-Path $PSScriptRoot '..\.env'
    if (Test-Path $envPath) {
        foreach ($l in Get-Content $envPath) {
            if ($l -match '^\s*HTTP_PORT\s*=\s*(\d+)') { $Port = [int]$Matches[1] }
        }
    }
}
if (-not $Port) { $Port = 80 }

$base = "http://127.0.0.1:$Port"

$checks = @(
    @{ Name = 'API';       Url = "$base/health/live" },
    @{ Name = 'Aplikasi';  Url = "$base/" },
    @{ Name = 'Dashboard'; Url = "$base/admin/login" }
)

$deadline = (Get-Date).AddSeconds($TimeoutSeconds)

foreach ($c in $checks) {
    $ok = $false
    while ((Get-Date) -lt $deadline) {
        try {
            $r = Invoke-WebRequest -Uri $c.Url -UseBasicParsing -TimeoutSec 5 -ErrorAction Stop
            if ($r.StatusCode -ge 200 -and $r.StatusCode -lt 400) { $ok = $true; break }
        } catch {
            Start-Sleep -Seconds 2
        }
    }

    if ($ok) {
        Write-Host ("  [ok] {0,-10} {1}" -f $c.Name, $c.Url) -ForegroundColor Green
    } else {
        Write-Host ("  [!]  {0,-10} belum menjawab" -f $c.Name) -ForegroundColor Yellow
        Write-Host  "       Lihat sebabnya:  docker compose logs --tail=50"
    }
}
