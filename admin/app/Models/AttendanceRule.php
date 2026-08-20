<?php

namespace App\Models;

use App\Models\Concerns\BelongsToSchool;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

/** Jam masuk/pulang per sekolah (boleh dioverride per kelas). */
class AttendanceRule extends Model
{
    use BelongsToSchool;

    protected $table = 'attendance_rules';

    public $incrementing = false;

    protected $keyType = 'string';

    protected $fillable = [
        'school_id', 'classroom_id', 'name',
        'check_in_opens_at', 'check_in_start_at', 'check_in_due_at', 'check_in_closes_at',
        'check_out_opens_at', 'check_out_closes_at',
        'late_grace_minutes', 'active_weekdays', 'require_check_out',
        'effective_from', 'effective_to', 'is_active',
    ];

    protected function casts(): array
    {
        return [
            'late_grace_minutes' => 'integer',
            'active_weekdays' => 'integer',
            'require_check_out' => 'boolean',
            'is_active' => 'boolean',
            'effective_from' => 'date',
            'effective_to' => 'date',
        ];
    }

    /** Nama hari sesuai urutan bitmask: bit0 = Senin. */
    public const WEEKDAY_NAMES = ['Senin', 'Selasa', 'Rabu', 'Kamis', 'Jumat', 'Sabtu', 'Minggu'];

    public function classroom(): BelongsTo
    {
        return $this->belongsTo(Classroom::class);
    }

    /**
     * Daftar hari aktif, dibaca dari bitmask `active_weekdays`.
     *
     * @return array<int, string>
     */
    public function getActiveDayNamesAttribute(): array
    {
        $days = [];
        foreach (self::WEEKDAY_NAMES as $bit => $name) {
            if ($this->active_weekdays & (1 << $bit)) {
                $days[] = $name;
            }
        }

        return $days;
    }

    public function getScopeLabelAttribute(): string
    {
        return $this->classroom_id ? ($this->classroom->name ?? 'Kelas tertentu') : 'Seluruh sekolah';
    }
}
