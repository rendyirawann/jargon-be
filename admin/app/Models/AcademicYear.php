<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\HasMany;

class AcademicYear extends Model
{
    protected $table = 'academic_years';

    public $incrementing = false;

    protected $keyType = 'string';

    protected $fillable = ['name', 'start_date', 'end_date', 'is_active'];

    protected function casts(): array
    {
        return [
            'start_date' => 'date',
            'end_date' => 'date',
            'is_active' => 'boolean',
        ];
    }

    public function classrooms(): HasMany
    {
        return $this->hasMany(Classroom::class);
    }

    /**
     * Tahun ajaran aktif. Hanya boleh ada satu (dijaga unique index di DB).
     */
    public static function active(): ?self
    {
        return static::where('is_active', true)->first();
    }
}
