<?php

namespace App\Models;

use App\Models\Concerns\BelongsToSchool;
use Illuminate\Database\Eloquent\Builder;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

/**
 * Satu baris absensi = satu siswa pada satu hari.
 *
 * PENTING soal performa: tabel ini dipartisi RANGE per bulan pada kolom
 * `attendance_date` (lihat migrasi 0007). Setiap query dari dashboard WAJIB
 * membawa filter tanggal, kalau tidak PostgreSQL akan memindai seluruh
 * riwayat provinsi (ratusan juta baris). Scope `betweenDates()` dan
 * `onDate()` ada untuk memudahkan hal itu — pakai salah satunya, selalu.
 *
 * Tabel ini juga TIDAK memuat data biometrik apa pun. Nama siswa/kelas/
 * sekolah disimpan sebagai snapshot supaya rekap lama tetap benar meski
 * siswa naik kelas atau pindah.
 */
class Attendance extends Model
{
    use BelongsToSchool;

    protected $table = 'attendances';

    public $incrementing = false;

    protected $keyType = 'string';

    protected $fillable = [
        'status', 'notes', 'marked_by', 'marked_at', 'check_in_at', 'check_out_at',
        'late_minutes',
    ];

    protected function casts(): array
    {
        return [
            'attendance_date' => 'date',
            'check_in_at' => 'datetime',
            'check_out_at' => 'datetime',
            'marked_at' => 'datetime',
            'notified_at' => 'datetime',
            'late_minutes' => 'integer',
            'duration_minutes' => 'integer',
            'check_in_similarity' => 'float',
            'check_out_similarity' => 'float',
        ];
    }

    public const STATUSES = ['hadir', 'terlambat', 'izin', 'sakit', 'alfa', 'dispensasi'];

    /** Status yang dihitung sebagai masuk sekolah. */
    public const PRESENT_STATUSES = ['hadir', 'terlambat', 'dispensasi'];

    public function student(): BelongsTo
    {
        return $this->belongsTo(Student::class);
    }

    public function classroom(): BelongsTo
    {
        return $this->belongsTo(Classroom::class);
    }

    public function markedBy(): BelongsTo
    {
        return $this->belongsTo(User::class, 'marked_by');
    }

    public function scopeOnDate(Builder $query, $date): Builder
    {
        return $query->whereDate('attendance_date', $date);
    }

    public function scopeBetweenDates(Builder $query, $from, $to): Builder
    {
        return $query->whereBetween('attendance_date', [$from, $to]);
    }

    public function scopePresent(Builder $query): Builder
    {
        return $query->whereIn('status', self::PRESENT_STATUSES);
    }

    public function scopeMissingCheckOut(Builder $query): Builder
    {
        return $query->whereNotNull('check_in_at')->whereNull('check_out_at');
    }

    public function getStatusLabelAttribute(): string
    {
        return match ($this->status) {
            'hadir' => 'Hadir',
            'terlambat' => 'Terlambat',
            'izin' => 'Izin',
            'sakit' => 'Sakit',
            'alfa' => 'Tanpa Keterangan',
            'dispensasi' => 'Dispensasi',
            default => ucfirst((string) $this->status),
        };
    }

    public function getStatusBadgeAttribute(): string
    {
        return match ($this->status) {
            'hadir' => 'badge-light-success',
            'terlambat' => 'badge-light-warning',
            'izin', 'dispensasi' => 'badge-light-info',
            'sakit' => 'badge-light-primary',
            'alfa' => 'badge-light-danger',
            default => 'badge-light',
        };
    }

    public function getCheckInTimeAttribute(): string
    {
        return $this->check_in_at ? $this->check_in_at->timezone(config('app.timezone'))->format('H:i') : '-';
    }

    public function getCheckOutTimeAttribute(): string
    {
        return $this->check_out_at ? $this->check_out_at->timezone(config('app.timezone'))->format('H:i') : '-';
    }
}
