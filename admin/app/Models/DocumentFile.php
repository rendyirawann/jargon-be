<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

/**
 * Satu berkas pada pengajuan kepegawaian.
 *
 * Isi berkas TIDAK disimpan di database, hanya `file_key` menuju storage.
 * Ijazah hasil pindai bisa beberapa MB; menaruhnya di kolom akan membuat
 * backup database membengkak dan replikasi melambat.
 *
 * Berkas juga tidak pernah dilayani langsung oleh web server: URL-nya
 * menunjuk ke endpoint API yang memverifikasi hak akses lebih dulu.
 */
class DocumentFile extends Model
{
    protected $table = 'document_files';

    public $incrementing = false;

    protected $keyType = 'string';

    public $timestamps = false;

    protected $fillable = ['status', 'reject_reason', 'reviewed_by', 'reviewed_at'];

    protected function casts(): array
    {
        return [
            'bytes' => 'integer',
            'uploaded_at' => 'datetime',
            'reviewed_at' => 'datetime',
        ];
    }

    public function submission(): BelongsTo
    {
        return $this->belongsTo(DocumentSubmission::class, 'submission_id');
    }

    public function documentType(): BelongsTo
    {
        return $this->belongsTo(DocumentType::class, 'document_type_id');
    }

    public function reviewer(): BelongsTo
    {
        return $this->belongsTo(User::class, 'reviewed_by');
    }

    /**
     * URL berkas melalui penerus dashboard.
     *
     * Browser memegang cookie sesi dashboard, bukan access token API, jadi
     * tautan diarahkan ke `/admin/files/*` yang menempelkan token itu.
     * Otorisasinya tetap dilakukan API: berkas ini memuat NIK, nomor rekening,
     * dan ijazah.
     */
    public function getFileUrlAttribute(): string
    {
        return route('files.show', ['key' => ltrim($this->file_key, '/')]);
    }

    public function getSizeLabelAttribute(): string
    {
        return $this->bytes < 1048576
            ? round($this->bytes / 1024).' KB'
            : round($this->bytes / 1048576, 1).' MB';
    }

    public function getStatusBadgeAttribute(): string
    {
        return match ($this->status) {
            'disetujui' => 'badge-light-success',
            'ditolak' => 'badge-light-danger',
            default => 'badge-light-warning',
        };
    }
}