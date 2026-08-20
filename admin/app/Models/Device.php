<?php

namespace App\Models;

use App\Models\Concerns\BelongsToSchool;
use Illuminate\Database\Eloquent\Builder;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\SoftDeletes;

/**
 * Tablet kios di gerbang atau di dalam kelas.
 *
 * Kolom `token_hash` dan `hmac_secret` sengaja TIDAK masuk `$fillable` dan
 * ikut `$hidden`: kredensial perangkat hanya boleh dibuat oleh API Rust pada
 * saat pairing, dan tidak pernah ditampilkan lagi setelah itu.
 */
class Device extends Model
{
    use BelongsToSchool;
    use SoftDeletes;

    protected $table = 'devices';

    public $incrementing = false;

    protected $keyType = 'string';

    protected $fillable = [
        'school_id', 'code', 'name', 'placement', 'classroom_id', 'mode', 'is_active',
    ];

    protected $hidden = ['token_hash', 'hmac_secret'];

    protected function casts(): array
    {
        return [
            'is_active' => 'boolean',
            'last_seen_at' => 'datetime',
            'token_issued_at' => 'datetime',
            'token_revoked_at' => 'datetime',
            'pairing_expires_at' => 'datetime',
            'deleted_at' => 'datetime',
        ];
    }

    public const PLACEMENTS = ['gate', 'classroom', 'office', 'mobile'];

    public const MODES = ['auto', 'check_in', 'check_out', 'enroll'];

    /** Ambang perangkat dianggap offline (menit). Sama dengan nilai di API. */
    public const OFFLINE_AFTER_MINUTES = 10;

    public function classroom(): BelongsTo
    {
        return $this->belongsTo(Classroom::class);
    }

    public function scopeOnline(Builder $query): Builder
    {
        return $query->where('last_seen_at', '>', now()->subMinutes(self::OFFLINE_AFTER_MINUTES));
    }

    public function getIsOnlineAttribute(): bool
    {
        return $this->last_seen_at !== null
            && $this->last_seen_at->gt(now()->subMinutes(self::OFFLINE_AFTER_MINUTES));
    }

    public function getIsPairedAttribute(): bool
    {
        return $this->token_hash !== null && $this->token_revoked_at === null;
    }

    public function getStatusLabelAttribute(): string
    {
        if (! $this->is_active) {
            return 'Nonaktif';
        }
        if (! $this->is_paired) {
            return 'Belum dipasangkan';
        }

        return $this->is_online ? 'Online' : 'Offline';
    }

    public function getStatusBadgeAttribute(): string
    {
        return match ($this->status_label) {
            'Online' => 'badge-light-success',
            'Offline' => 'badge-light-warning',
            'Belum dipasangkan' => 'badge-light-info',
            default => 'badge-light-danger',
        };
    }

    public function getPlacementLabelAttribute(): string
    {
        return match ($this->placement) {
            'gate' => 'Gerbang',
            'classroom' => 'Ruang Kelas',
            'office' => 'Kantor',
            'mobile' => 'Mobile',
            default => ucfirst((string) $this->placement),
        };
    }

    public function getModeLabelAttribute(): string
    {
        return match ($this->mode) {
            'auto' => 'Otomatis (masuk & pulang)',
            'check_in' => 'Hanya absen masuk',
            'check_out' => 'Hanya absen pulang',
            'enroll' => 'Pendaftaran wajah',
            default => ucfirst((string) $this->mode),
        };
    }
}
