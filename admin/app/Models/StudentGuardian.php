<?php

namespace App\Models;

use App\Models\Concerns\BelongsToSchool;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

/**
 * Wali murid — tujuan notifikasi absensi.
 *
 * Nomor telepon disimpan dalam format 62xxx (E.164 tanpa `+`) karena itulah
 * satu-satunya bentuk yang diterima provider WhatsApp. Normalisasi dilakukan
 * pada mutator agar operator boleh mengetik format apa pun.
 */
class StudentGuardian extends Model
{
    use BelongsToSchool;

    protected $table = 'student_guardians';

    public $incrementing = false;

    protected $keyType = 'string';

    protected $fillable = [
        'student_id', 'school_id', 'relation', 'full_name', 'phone', 'whatsapp',
        'email', 'telegram_chat_id', 'preferred_channel', 'is_primary', 'notify_enabled',
    ];

    protected function casts(): array
    {
        return [
            'is_primary' => 'boolean',
            'notify_enabled' => 'boolean',
        ];
    }

    public const RELATIONS = ['ayah', 'ibu', 'wali'];

    public const CHANNELS = ['whatsapp', 'telegram', 'email', 'none'];

    public function student(): BelongsTo
    {
        return $this->belongsTo(Student::class);
    }

    public function setPhoneAttribute($value): void
    {
        $this->attributes['phone'] = static::normalizePhone($value);
    }

    public function setWhatsappAttribute($value): void
    {
        $this->attributes['whatsapp'] = static::normalizePhone($value);
    }

    /**
     * Normalisasi nomor Indonesia ke 62xxx.
     *
     * Aturannya harus sama dengan `normalize_phone` di API Rust
     * (jargon-be/api/src/domain/student.rs) supaya nomor yang dimasukkan
     * lewat dashboard dan lewat API menghasilkan hasil identik.
     */
    public static function normalizePhone(?string $value): ?string
    {
        if ($value === null) {
            return null;
        }

        $digits = preg_replace('/\D+/', '', $value) ?? '';
        if ($digits === '') {
            return null;
        }

        if (str_starts_with($digits, '62')) {
            $normalized = $digits;
        } elseif (str_starts_with($digits, '0')) {
            $normalized = '62'.substr($digits, 1);
        } elseif (str_starts_with($digits, '8')) {
            $normalized = '62'.$digits;
        } else {
            $normalized = $digits;
        }

        $len = strlen($normalized);

        return ($len >= 11 && $len <= 15) ? $normalized : null;
    }

    /**
     * Alamat tujuan sesuai kanal pilihan; null bila kontaknya belum lengkap.
     */
    public function recipient(): ?string
    {
        return match ($this->preferred_channel) {
            'whatsapp' => $this->whatsapp ?: $this->phone,
            'telegram' => $this->telegram_chat_id ?: null,
            'email' => $this->email ?: null,
            default => null,
        };
    }

    public function getIsReachableAttribute(): bool
    {
        return $this->notify_enabled && $this->recipient() !== null;
    }
}
