<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

/** Satu langkah pada lini masa pengajuan berkas. */
class DocumentSubmissionEvent extends Model
{
    protected $table = 'document_submission_events';

    public $incrementing = false;

    protected $keyType = 'string';

    public $timestamps = false;

    protected function casts(): array
    {
        return ['created_at' => 'datetime'];
    }

    public function submission(): BelongsTo
    {
        return $this->belongsTo(DocumentSubmission::class, 'submission_id');
    }
}