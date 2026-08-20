<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\HasMany;

/** Kabupaten/kota di Provinsi Sumatera Utara. */
class Region extends Model
{
    protected $table = 'regions';

    public $incrementing = false;

    protected $keyType = 'string';

    protected $fillable = ['code', 'name', 'kind', 'parent_id'];

    public function schools(): HasMany
    {
        return $this->hasMany(School::class);
    }
}
