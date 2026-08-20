<?php

namespace App\Models;

use App\Models\Concerns\BelongsToSchool;
use Illuminate\Database\Eloquent\Builder;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

/**
 * Antrean/riwayat pesan ke wali murid.
 *
 * Pengiriman dilakukan worker di API Rust, bukan queue Laravel — supaya
 * pencatatan absensi dan pembuatan pesan terjadi dalam satu transaksi
 * database. Dashboard hanya membaca tabel ini dan boleh meminta kirim ulang.
 *
 * Dipartisi per bulan pada `created_at`, jadi query dashboard selalu dibatasi
 * rentang waktu (lihat `scopeRecent`).
 */
class NotificationOutbox extends Model
{
    use BelongsToSchool;

    protected $table = 'notification_outbox';

    public $incrementing = false;

    protected $keyType = 'string';

    /** Isi pesan dibuat oleh API; dashboard hanya mengubah status kirim ulang. */
    protected $fillable = ['status', 'scheduled_at', 'attempts', 'last_error'];

    protected function casts(): array
    {
        return [
            'variables' => 'array',
            'attempts' => 'integer',
            'max_attempts' => 'integer',
            'scheduled_at' => 'datetime',
            'locked_at' => 'datetime',
            'sent_at' => 'datetime',
        ];
    }

    public const STATUSES = ['queued', 'sending', 'sent', 'failed', 'cancelled'];

    public function student(): BelongsTo
    {
        return $this->belongsTo(Student::class);
    }

    public function guardian(): BelongsTo
    {
        return $this->belongsTo(StudentGuardian::class, 'guardian_id');
    }

    /**
     * Batasi ke partisi terbaru. WAJIB dipakai sebelum query lain.
     */
    public function scopeRecent(Builder $query, int $days = 90): Builder
    {
        return $query->where('created_at', '>', now()->subDays($days));
    }

    public function scopeFailed(Builder $query): Builder
    {
        return $query->where('status', 'failed');
    }

    /**
     * Samarkan tujuan agar daftar log tidak menjadi sumber kebocoran nomor
     * telepon orang tua. Aturannya sama dengan `mask_recipient` di API.
     */
    public function getMaskedRecipientAttribute(): string
    {
        $value = (string) $this->recipient;

        if (str_contains($value, '@')) {
            [$local, $domain] = explode('@', $value, 2);

            return mb_substr($local, 0, 1).'***@'.$domain;
        }

        $len = mb_strlen($value);
        if ($len <= 6) {
            return str_repeat('*', $len);
        }

        return mb_substr($value, 0, 4).str_repeat('*', $len - 7).mb_substr($value, -3);
    }

    public function getStatusBadgeAttribute(): string
    {
        return match ($this->status) {
            'sent' => 'badge-light-success',
            'queued' => 'badge-light-info',
            'sending' => 'badge-light-primary',
            'failed' => 'badge-light-danger',
            'cancelled' => 'badge-light-warning',
            default => 'badge-light',
        };
    }

    public function getChannelLabelAttribute(): string
    {
        return match ($this->channel) {
            'whatsapp' => 'WhatsApp',
            'telegram' => 'Telegram',
            'email' => 'Email',
            default => ucfirst((string) $this->channel),
        };
    }
}
