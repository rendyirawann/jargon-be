# Buat nilai acak untuk JWT_SECRET dan SECRETS_KEY_HEX.
#
# Keduanya WAJIB diisi sebelum API bisa dijalankan, dan keduanya harus acak
# kriptografis — bukan diketik sendiri. JWT_SECRET yang bisa ditebak berarti
# siapa pun dapat menempa token akses untuk peran apa pun, termasuk
# superadmin; SECRETS_KEY_HEX adalah kunci yang mengenkripsi kredensial
# provider notifikasi di database.
#
# Pemakaian:
#   powershell -File scripts\new-secrets.ps1
#
# Salin hasilnya ke api\.env

$rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()

# JWT_SECRET: 48 byte acak dalam base64 (~64 karakter).
$jwt = New-Object byte[] 48
$rng.GetBytes($jwt)

# SECRETS_KEY_HEX: tepat 32 byte dalam heksadesimal — ukurannya ditentukan
# algoritma enkripsinya, jadi jangan diubah panjangnya.
$key = New-Object byte[] 32
$rng.GetBytes($key)

$rng.Dispose()

Write-Host ''
Write-Host 'Salin dua baris ini ke api\.env :' -ForegroundColor Cyan
Write-Host ''
Write-Host ("JWT_SECRET=" + [Convert]::ToBase64String($jwt))
Write-Host ("SECRETS_KEY_HEX=" + [BitConverter]::ToString($key).Replace('-', '').ToLower())
Write-Host ''
Write-Host 'Jangan pakai nilai yang sama antara staging dan produksi.' -ForegroundColor Yellow
Write-Host ''
