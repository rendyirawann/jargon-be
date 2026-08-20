<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

/**
 * Satu langkah pada lini masa penanganan laporan.
 *
 * Disimpan sebagai baris terpisah, bukan sekadar menimpa kolom `status`,
 * supaya riwayat penanganan tetap utuh dan bisa dipertanggungjawabkan.
 */
class PanicReportEvent extends Model
{
    protected $table = 'panic_report_events';

    public $incrementing = false;

    protected $keyType = 'string';

    public $timestamps = false;

    protected function casts(): array
    {
        return ['created_at' => 'datetime', 'is_public' => 'boolean'];
    }

    public function report(): BelongsTo
    {
        return $this->belongsTo(PanicReport::class, 'report_id');
    }
}