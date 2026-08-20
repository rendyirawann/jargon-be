<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Builder;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;

/**
 * Pengajuan berkas kepegawaian.
 *
 * Berkasnya memuat data pribadi (NIK, rekening, ijazah), jadi aturan
 * penglihatannya lebih ketat daripada tenant biasa: seseorang tanpa izin
 * verifikasi hanya melihat pengajuannya sendiri, bahkan untuk rekan
 * sesekolah. Itu ditegakkan oleh scope `visibleTo`.
 */
class DocumentSubmission extends Model
{
    protected $table = 'document_submissions';

    public $incrementing = false;

    protected $keyType = 'string';

    /** Perubahan status dilakukan lewat API agar lini masa ikut tercatat. */
    protected $fillable = ['status', 'review_note', 'reviewed_by', 'reviewed_at'];

    protected function casts(): array
    {
        return [
            'submitted_at' => 'datetime',
            'reviewed_at' => 'datetime',
            'created_at' => 'datetime',
            'updated_at' => 'datetime',
            'file_count' => 'integer',
            'approved_file_count' => 'integer',
            'rejected_file_count' => 'integer',
        ];
    }

    public const STATUSES = [
        'draft' => 'Draft',
        'diajukan' => 'Menunggu Diperiksa',
        'diperiksa' => 'Sedang Diperiksa',
        'revisi' => 'Perlu Perbaikan',
        'disetujui' => 'Disetujui',
        'ditolak' => 'Ditolak',
    ];

    public function owner(): BelongsTo
    {
        return $this->belongsTo(User::class, 'user_id');
    }

    public function school(): BelongsTo
    {
        return $this->belongsTo(School::class);
    }

    public function reviewer(): BelongsTo
    {
        return $this->belongsTo(User::class, 'reviewed_by');
    }

    public function files(): HasMany
    {
        return $this->hasMany(DocumentFile::class, 'submission_id');
    }

    public function events(): HasMany
    {
        return $this->hasMany(DocumentSubmissionEvent::class, 'submission_id')
            ->orderBy('created_at');
    }

    public function scopeVisibleTo(Builder $query, ?User $user): Builder
    {
        if (! $user) {
            return $query->whereRaw('1 = 0');
        }

        // Tanpa izin verifikasi: hanya miliknya sendiri.
        if (! $user->can('verify_document_submission')) {
            return $query->where('document_submissions.user_id', $user->id);
        }

        // Draft milik orang lain tidak pernah terlihat verifikator — itu
        // masih berupa coretan, bukan pengajuan.
        $query->where(function ($q) use ($user) {
            $q->where('document_submissions.user_id', $user->id)
                ->orWhere('document_submissions.status', '<>', 'draft');
        });

        $allowed = \App\Support\Tenant::schoolIds();
        if ($allowed !== null) {
            $query->where(function ($q) use ($allowed, $user) {
                $q->whereIn('document_submissions.school_id', $allowed ?: ['00000000-0000-0000-0000-000000000000'])
                    ->orWhere('document_submissions.user_id', $user->id);
            });
        }

        return $query;
    }

    public function scopeAwaitingReview(Builder $query): Builder
    {
        return $query->whereIn('status', ['diajukan', 'diperiksa']);
    }

    public function getStatusLabelAttribute(): string
    {
        return self::STATUSES[$this->status] ?? $this->status;
    }

    public function getStatusBadgeAttribute(): string
    {
        return match ($this->status) {
            'draft' => 'badge-light',
            'diajukan' => 'badge-light-warning',
            'diperiksa' => 'badge-light-info',
            'revisi' => 'badge-light-warning',
            'disetujui' => 'badge-light-success',
            'ditolak' => 'badge-light-danger',
            default => 'badge-light',
        };
    }

    public function getPurposeLabelAttribute(): string
    {
        return DocumentType::PURPOSES[$this->purpose] ?? $this->purpose;
    }
}
