<?php

namespace App\Models;

use App\Models\Concerns\BelongsToSchool;
use Illuminate\Database\Eloquent\Builder;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;
use Illuminate\Database\Eloquent\SoftDeletes;

/** Rombongan belajar (kelas). */
class Classroom extends Model
{
    use BelongsToSchool;
    use SoftDeletes;

    protected $table = 'classrooms';

    public $incrementing = false;

    protected $keyType = 'string';

    protected $fillable = [
        'school_id', 'academic_year_id', 'name', 'grade_level', 'major',
        'homeroom_teacher_id', 'capacity', 'is_active',
    ];

    protected function casts(): array
    {
        return [
            'grade_level' => 'integer',
            'capacity' => 'integer',
            'is_active' => 'boolean',
            'deleted_at' => 'datetime',
        ];
    }

    public function academicYear(): BelongsTo
    {
        return $this->belongsTo(AcademicYear::class);
    }

    public function homeroomTeacher(): BelongsTo
    {
        return $this->belongsTo(User::class, 'homeroom_teacher_id');
    }

    public function students(): HasMany
    {
        return $this->hasMany(Student::class, 'current_classroom_id');
    }

    public function scopeCurrentYear(Builder $query): Builder
    {
        return $query->whereHas('academicYear', fn ($q) => $q->where('is_active', true));
    }

    /**
     * Hanya kelas yang diampu pengguna sebagai wali kelas.
     */
    public function scopeHomeroomOf(Builder $query, string $userId): Builder
    {
        return $query->where('homeroom_teacher_id', $userId);
    }

    public function getFullNameAttribute(): string
    {
        return $this->major ? "{$this->name} - {$this->major}" : $this->name;
    }
}
