<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

/** Jenis dokumen yang diminta untuk sebuah keperluan kepegawaian. */
class DocumentType extends Model
{
    protected $table = 'document_types';

    public $incrementing = false;

    protected $keyType = 'string';

    public const UPDATED_AT = null;

    protected $fillable = [
        'code', 'name', 'description', 'purpose', 'is_required',
        'max_bytes', 'allowed_mime', 'sort_order', 'is_active',
    ];

    protected function casts(): array
    {
        return [
            'is_required' => 'boolean',
            'is_active' => 'boolean',
            'max_bytes' => 'integer',
            'sort_order' => 'integer',
            'created_at' => 'datetime',
        ];
    }

    public const PURPOSES = [
        'kenaikan_pangkat' => 'Kenaikan Pangkat',
        'sertifikasi' => 'Sertifikasi',
        'tunjangan' => 'Tunjangan',
        'mutasi' => 'Mutasi',
        'pensiun' => 'Pensiun',
        'umum' => 'Umum',
    ];

    public function getPurposeLabelAttribute(): string
    {
        return self::PURPOSES[$this->purpose] ?? $this->purpose;
    }
}