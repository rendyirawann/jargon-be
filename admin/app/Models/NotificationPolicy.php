<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

/**
 * Kejadian absensi apa saja yang memicu notifikasi ke wali, per sekolah.
 *
 * Primary key-nya `school_id` (satu baris per sekolah), bukan `id`.
 */
class NotificationPolicy extends Model
{
    protected $table = 'notification_policies';

    protected $primaryKey = 'school_id';

    public $incrementing = false;

    protected $keyType = 'string';

    public const CREATED_AT = null;

    protected $fillable = [
        'school_id', 'notify_on_check_in', 'notify_on_check_out', 'notify_on_late',
        'notify_on_absent', 'absent_notify_after', 'quiet_hours_start',
        'quiet_hours_end', 'daily_recap_at',
    ];

    protected function casts(): array
    {
        return [
            'notify_on_check_in' => 'boolean',
            'notify_on_check_out' => 'boolean',
            'notify_on_late' => 'boolean',
            'notify_on_absent' => 'boolean',
        ];
    }

    public function school(): BelongsTo
    {
        return $this->belongsTo(School::class);
    }

    /**
     * Ambil kebijakan sekolah, buat dengan nilai default bila belum ada.
     */
    public static function forSchool(string $schoolId): self
    {
        return static::firstOrCreate(['school_id' => $schoolId]);
    }
}
