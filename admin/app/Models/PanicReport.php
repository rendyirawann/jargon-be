<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Builder;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;

/**
 * Laporan Panic Button.
 *
 * PERINGATAN PRIVASI — BACA SEBELUM MENGUBAH BERKAS INI
 *
 * Kolom `author_user_id` memuat identitas pelapor. Pelapor perundungan dan
 * pungli hampir selalu berada pada posisi lemah terhadap pihak yang
 * dilaporkan — kadang gurunya sendiri. Karena itu:
 *
 *   * `author_user_id` masuk `$hidden` sehingga tidak pernah ikut ter-serialisasi
 *     ke JSON, tampilan Blade, maupun log yang men-dump model.
 *   * Relasi `author()` sengaja TIDAK didefinisikan. Membukanya berarti satu
 *     `->with('author')` yang tidak sengaja sudah cukup untuk membocorkan
 *     identitas ke seluruh halaman.
 *   * Pembukaan identitas hanya lewat API (`POST /v1/panic/reports/{id}/unmask`)
 *     yang mewajibkan alasan tertulis dan mencatatnya permanen.
 *
 * Dashboard hanya perlu MEMBACA laporan untuk moderasi dan penanganan; untuk
 * itu identitas pelapor tidak dibutuhkan sama sekali.
 */
class PanicReport extends Model
{
    protected $table = 'panic_reports';

    public $incrementing = false;

    protected $keyType = 'string';

    /** Perubahan status dilakukan lewat API agar lini masa ikut tercatat. */
    protected $fillable = ['moderation_status', 'moderation_note', 'moderated_by', 'moderated_at'];

    protected $hidden = ['author_user_id'];

    protected function casts(): array
    {
        return [
            'created_at' => 'datetime',
            'updated_at' => 'datetime',
            'moderated_at' => 'datetime',
            'handled_at' => 'datetime',
            'resolved_at' => 'datetime',
            'support_count' => 'integer',
            'comment_count' => 'integer',
        ];
    }

    public const STATUSES = ['baru', 'diverifikasi', 'ditindaklanjuti', 'selesai', 'ditolak'];

    public const SEVERITIES = ['rendah', 'sedang', 'tinggi', 'darurat'];

    /**
     * Kategori yang tidak pernah ditampilkan kepada peran tingkat sekolah.
     *
     * Ketiganya lazim melibatkan pihak yang berwenang di sekolah itu sendiri;
     * menampilkannya di dashboard kepala sekolah sama dengan memberi tahu
     * terlapor bahwa ada yang melapor.
     */
    public const SCHOOL_HIDDEN_CATEGORIES = ['kekerasan', 'pelecehan', 'pungli'];

    public function school(): BelongsTo
    {
        return $this->belongsTo(School::class);
    }

    public function category(): BelongsTo
    {
        return $this->belongsTo(PanicCategory::class, 'category_id');
    }

    public function comments(): HasMany
    {
        return $this->hasMany(PanicComment::class, 'report_id')
            ->whereNull('deleted_at')
            ->orderBy('created_at');
    }

    public function events(): HasMany
    {
        return $this->hasMany(PanicReportEvent::class, 'report_id')
            ->orderBy('created_at');
    }

    /**
     * Batasi ke partisi terbaru. WAJIB dipakai sebelum query lain —
     * tabel dipartisi per bulan.
     */
    public function scopeRecent(Builder $query, int $days = 180): Builder
    {
        return $query->where('panic_reports.created_at', '>', now()->subDays($days));
    }

    /**
     * Terapkan pembatasan penglihatan sesuai peran pengguna aktif.
     *
     * Ini pengganti global scope tenant: laporan tidak difilter semata-mata
     * oleh school_id, melainkan oleh kombinasi peran + kategori.
     */
    public function scopeVisibleTo(Builder $query, ?\App\Models\User $user): Builder
    {
        if (! $user) {
            return $query->whereRaw('1 = 0');
        }

        // Peran tingkat provinsi melihat semuanya, termasuk laporan `terbatas`.
        if ($user->hasAnyRole(['superadmin', 'admin_dinas', 'petugas_pengaduan'])) {
            return $query;
        }

        // Peran tingkat sekolah: hanya sekolahnya, tanpa kategori sensitif.
        $allowed = \App\Support\Tenant::schoolIds() ?? [];

        return $query
            ->whereIn('panic_reports.school_id', $allowed ?: ['00000000-0000-0000-0000-000000000000'])
            ->where('panic_reports.visibility', 'publik')
            ->whereHas('category', fn ($q) => $q->whereNotIn('code', self::SCHOOL_HIDDEN_CATEGORIES));
    }

    public function scopePendingModeration(Builder $query): Builder
    {
        return $query->where('moderation_status', 'pending');
    }

    public function scopeUrgent(Builder $query): Builder
    {
        return $query->whereIn('severity', ['tinggi', 'darurat'])->whereNull('handled_at');
    }

    public function getStatusLabelAttribute(): string
    {
        return match ($this->status) {
            'baru' => 'Baru',
            'diverifikasi' => 'Diverifikasi',
            'ditindaklanjuti' => 'Ditindaklanjuti',
            'selesai' => 'Selesai',
            'ditolak' => 'Ditolak',
            default => (string) $this->status,
        };
    }

    public function getStatusBadgeAttribute(): string
    {
        return match ($this->status) {
            'baru' => 'badge-light-warning',
            'diverifikasi' => 'badge-light-info',
            'ditindaklanjuti' => 'badge-light-primary',
            'selesai' => 'badge-light-success',
            'ditolak' => 'badge-light-danger',
            default => 'badge-light',
        };
    }

    public function getSeverityBadgeAttribute(): string
    {
        return match ($this->severity) {
            'darurat' => 'badge-danger',
            'tinggi' => 'badge-light-danger',
            'sedang' => 'badge-light-warning',
            default => 'badge-light',
        };
    }
}
