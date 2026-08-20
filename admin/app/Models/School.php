<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Builder;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;
use Illuminate\Database\Eloquent\SoftDeletes;

/**
 * Sekolah = tenant sistem.
 *
 * Skema tabel dimiliki oleh migrasi sqlx pada `jargon-be/api/migrations`.
 * Model ini sengaja tidak memakai `$guarded = []`: kolom `slug` dan `npsn`
 * ikut menentukan identitas, jadi perubahannya harus lewat jalur eksplisit.
 */
class School extends Model
{
    use SoftDeletes;

    protected $table = 'schools';

    public $incrementing = false;

    protected $keyType = 'string';

    protected $fillable = [
        'npsn', 'name', 'slug', 'jenjang', 'status', 'region_id', 'address',
        'village', 'district', 'postal_code', 'latitude', 'longitude',
        'geofence_radius_m', 'phone', 'email', 'principal_name', 'logo_path',
        'timezone', 'face_match_threshold', 'settings', 'is_active',
    ];

    protected function casts(): array
    {
        return [
            'latitude' => 'float',
            'longitude' => 'float',
            'geofence_radius_m' => 'integer',
            'face_match_threshold' => 'float',
            'settings' => 'array',
            'is_active' => 'boolean',
            'deleted_at' => 'datetime',
        ];
    }

    public const JENJANG = ['PAUD', 'TK', 'SD', 'SMP', 'SMA', 'SMK', 'SLB'];

    public const STATUS = ['negeri', 'swasta'];

    public function region(): BelongsTo
    {
        return $this->belongsTo(Region::class);
    }

    public function classrooms(): HasMany
    {
        return $this->hasMany(Classroom::class);
    }

    public function students(): HasMany
    {
        return $this->hasMany(Student::class);
    }

    public function devices(): HasMany
    {
        return $this->hasMany(Device::class);
    }

    public function users(): HasMany
    {
        return $this->hasMany(User::class);
    }

    /**
     * Hanya sekolah yang boleh diakses pengguna aktif.
     */
    public function scopeAccessible(Builder $query): Builder
    {
        $allowed = \App\Support\Tenant::schoolIds();

        if ($allowed === null) {
            return $query;
        }

        return $query->whereIn('id', $allowed ?: ['00000000-0000-0000-0000-000000000000']);
    }

    public function getLabelAttribute(): string
    {
        return "{$this->name} ({$this->npsn})";
    }
}
