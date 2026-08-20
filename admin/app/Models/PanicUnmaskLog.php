<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

/**
 * Catatan pembukaan identitas pelapor.
 *
 * Tabel ini adalah pengaman terakhir anonimitas Panic Button. Tanpa catatan
 * yang tidak bisa dihapus, izin `unmask_panic_report` hanyalah janji.
 * Karena itu model ini sengaja READ-ONLY dari dashboard: `$fillable` kosong
 * dan tidak ada satu pun jalur di aplikasi yang menghapus barisnya.
 */
class PanicUnmaskLog extends Model
{
    protected $table = 'panic_unmask_logs';

    public $incrementing = false;

    protected $keyType = 'string';

    public $timestamps = false;

    protected $fillable = [];

    protected function casts(): array
    {
        return ['created_at' => 'datetime'];
    }
}