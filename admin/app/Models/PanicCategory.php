<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\HasMany;

/** Kategori pengaduan Panic Button. */
class PanicCategory extends Model
{
    protected $table = 'panic_categories';

    public $incrementing = false;

    protected $keyType = 'string';

    public $timestamps = false;

    protected $fillable = [
        'code', 'name', 'description', 'icon', 'default_severity',
        'sort_order', 'is_active',
    ];

    protected function casts(): array
    {
        return ['is_active' => 'boolean', 'sort_order' => 'integer'];
    }

    public function reports(): HasMany
    {
        return $this->hasMany(PanicReport::class, 'category_id');
    }

    /**
     * Kategori yang menyangkut pihak sekolah sendiri, sehingga tidak
     * ditampilkan kepada peran tingkat sekolah.
     */
    public function getIsSensitiveAttribute(): bool
    {
        return in_array($this->code, PanicReport::SCHOOL_HIDDEN_CATEGORIES, true);
    }
}