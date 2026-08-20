<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

/**
 * Template pesan ke wali murid.
 *
 * Baris dengan `school_id` NULL adalah template bawaan sistem yang dipakai
 * bila sekolah belum membuat versinya sendiri. Karena itu model ini TIDAK
 * memakai trait tenant: template bawaan harus tetap terlihat oleh semua
 * sekolah, dan penyaringan dilakukan eksplisit lewat `scopeForSchool`.
 */
class NotificationTemplate extends Model
{
    protected $table = 'notification_templates';

    public $incrementing = false;

    protected $keyType = 'string';

    protected $fillable = ['school_id', 'key', 'channel', 'subject', 'body', 'is_active'];

    protected function casts(): array
    {
        return ['is_active' => 'boolean'];
    }

    public const CHANNELS = ['whatsapp', 'telegram', 'email'];

    public const KEYS = [
        'check_in' => 'Absen Masuk',
        'check_out' => 'Absen Pulang',
        'late' => 'Terlambat',
        'absent' => 'Tanpa Keterangan',
        'sick' => 'Sakit',
        'permit' => 'Izin',
        'daily_recap' => 'Rekap Harian',
        'weekly_recap' => 'Rekap Mingguan',
        'custom' => 'Pesan Bebas',
    ];

    /** Placeholder yang boleh dipakai — harus sama dengan daftar di API. */
    public const VARIABLES = [
        'nama_siswa', 'nis', 'kelas', 'sekolah', 'tanggal',
        'jam_masuk', 'jam_pulang', 'status', 'menit_terlambat', 'nama_wali',
    ];

    public function scopeForSchool($query, ?string $schoolId)
    {
        return $query->where(function ($q) use ($schoolId) {
            $q->whereNull('school_id');
            if ($schoolId) {
                $q->orWhere('school_id', $schoolId);
            }
        });
    }

    public function getIsDefaultAttribute(): bool
    {
        return $this->school_id === null;
    }

    public function getKeyLabelAttribute(): string
    {
        return self::KEYS[$this->key] ?? $this->key;
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

    /**
     * Temukan placeholder yang tidak dikenal — dipakai validasi form.
     *
     * @return array<int, string>
     */
    public static function unknownPlaceholders(string $body): array
    {
        preg_match_all('/\{\{\s*([a-z_]+)\s*\}\}/i', $body, $matches);
        $found = array_map('strtolower', $matches[1] ?? []);

        return array_values(array_unique(array_diff($found, self::VARIABLES)));
    }
}
