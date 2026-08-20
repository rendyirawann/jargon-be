<?php

namespace App\Models;

use App\Models\Concerns\BelongsToSchool;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

/**
 * Satu sampel foto wajah hasil pendaftaran.
 *
 * Ini SATU-SATUNYA tempat gambar wajah tersimpan. Gambar tidak disimpan di
 * kolom (hanya `image_key` menuju storage), dan tidak pernah dilayani
 * langsung oleh web server — akses selalu melewati endpoint `/files/*` pada
 * API Rust yang memverifikasi hak akses terhadap sekolah siswa.
 *
 * Vektor embedding tinggal di tabel `face_embeddings` dan tidak dipetakan ke
 * Eloquent: dashboard tidak punya alasan membacanya.
 */
class FaceEnrollment extends Model
{
    use BelongsToSchool;

    protected $table = 'face_enrollments';

    public $incrementing = false;

    protected $keyType = 'string';

    protected $fillable = ['status', 'reject_reason', 'reviewed_by', 'reviewed_at'];

    protected function casts(): array
    {
        return [
            'quality_score' => 'float',
            'quality_detail' => 'array',
            'image_bytes' => 'integer',
            'reviewed_at' => 'datetime',
        ];
    }

    public const POSES = ['frontal', 'left', 'right', 'up', 'down'];

    public const STATUSES = ['pending', 'approved', 'rejected', 'replaced'];

    public function student(): BelongsTo
    {
        return $this->belongsTo(Student::class);
    }

    public function capturedBy(): BelongsTo
    {
        return $this->belongsTo(User::class, 'captured_by');
    }

    public function reviewedBy(): BelongsTo
    {
        return $this->belongsTo(User::class, 'reviewed_by');
    }

    public function device(): BelongsTo
    {
        return $this->belongsTo(Device::class);
    }

    /**
     * URL gambar melalui penerus dashboard, bukan path filesystem.
     *
     * Diarahkan ke `/admin/files/*` alih-alih langsung ke API karena browser
     * memegang cookie sesi dashboard, bukan access token API. Penerus itulah
     * yang menempelkan token; otorisasinya tetap dilakukan API.
     */
    public function getImageUrlAttribute(): string
    {
        return route('files.show', ['key' => ltrim($this->image_key, '/')]);
    }

    public function getPoseLabelAttribute(): string
    {
        return match ($this->pose) {
            'frontal' => 'Depan',
            'left' => 'Miring Kiri',
            'right' => 'Miring Kanan',
            'up' => 'Menengadah',
            'down' => 'Menunduk',
            default => ucfirst((string) $this->pose),
        };
    }

    public function getQualityBadgeAttribute(): string
    {
        return match (true) {
            $this->quality_score === null => 'badge-light',
            $this->quality_score >= 0.7 => 'badge-light-success',
            $this->quality_score >= 0.45 => 'badge-light-warning',
            default => 'badge-light-danger',
        };
    }
}
