<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

/**
 * Komentar pada laporan Panic Button.
 *
 * Sama seperti laporannya, `author_user_id` disembunyikan dan tidak punya
 * relasi ke User. Komentar RESMI adalah pengecualian yang disengaja: nama dan
 * jabatan petugas justru ditampilkan agar pelapor tahu laporannya ditangani
 * pihak berwenang, bukan sesama warga.
 */
class PanicComment extends Model
{
    protected $table = 'panic_comments';

    public $incrementing = false;

    protected $keyType = 'string';

    public const UPDATED_AT = null;

    protected $fillable = ['moderation_status', 'deleted_at'];

    protected $hidden = ['author_user_id'];

    protected function casts(): array
    {
        return [
            'is_official' => 'boolean',
            'created_at' => 'datetime',
            'deleted_at' => 'datetime',
        ];
    }

    public function report(): BelongsTo
    {
        return $this->belongsTo(PanicReport::class, 'report_id');
    }

    public function getDisplayNameAttribute(): string
    {
        return $this->is_official
            ? ($this->official_name ?? 'Petugas')
            : ($this->anonymous_handle ?? 'Anonim');
    }
}