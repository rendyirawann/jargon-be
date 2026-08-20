<?php

namespace App\Models;

use App\Models\Concerns\BelongsToSchool;
use Illuminate\Database\Eloquent\Builder;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;
use Illuminate\Database\Eloquent\Relations\HasOne;
use Illuminate\Database\Eloquent\SoftDeletes;

/**
 * Siswa.
 *
 * Tabel ini tidak memuat kredensial apa pun. Identitas operasional siswa
 * adalah wajah yang didaftarkan sekali di awal; identitas administratifnya
 * NISN/NIS.
 *
 * Sejak Jargon GO, siswa BOLEH punya akun aplikasi (lihat `appAccount`),
 * tetapi akun itu hidup di tabel `users` dan hanya dipakai MEMBACA datanya
 * sendiri. Absensi tetap berjalan lewat pengenalan wajah di tablet tanpa
 * login — akun siswa tidak pernah menjadi jalan untuk mengabsenkan diri.
 */
class Student extends Model
{
    use BelongsToSchool;
    use SoftDeletes;

    protected $table = 'students';

    public $incrementing = false;

    protected $keyType = 'string';

    protected $fillable = [
        'school_id', 'current_classroom_id', 'nisn', 'nis', 'full_name', 'gender',
        'birth_place', 'birth_date', 'religion', 'address', 'phone', 'photo_path',
        'father_name', 'mother_name', 'status', 'entry_year', 'metadata',
    ];

    protected function casts(): array
    {
        return [
            'birth_date' => 'date',
            'entry_year' => 'integer',
            'face_enrolled' => 'boolean',
            'face_enrolled_at' => 'datetime',
            'face_sample_count' => 'integer',
            'metadata' => 'array',
            'deleted_at' => 'datetime',
        ];
    }

    public const STATUS = ['aktif', 'lulus', 'pindah', 'keluar', 'cuti'];

    /** Jumlah sampel wajah yang dianjurkan agar pengenalan andal. */
    public const RECOMMENDED_SAMPLES = 3;

    public function classroom(): BelongsTo
    {
        return $this->belongsTo(Classroom::class, 'current_classroom_id');
    }

    public function guardians(): HasMany
    {
        return $this->hasMany(StudentGuardian::class);
    }

    /**
     * Akun aplikasi Jargon GO milik siswa ini, bila sudah dibuatkan.
     *
     * Dipakai untuk menyaring "siswa yang belum punya akun" pada pembuatan
     * akun massal; tanpa itu, setiap kali tombol ditekan akan mencoba membuat
     * ulang akun seluruh kelas.
     */
    public function appAccount(): HasOne
    {
        return $this->hasOne(User::class, 'student_id')->whereNull('deleted_at');
    }

    public function primaryGuardian(): HasMany
    {
        return $this->hasMany(StudentGuardian::class)->where('is_primary', true);
    }

    public function faceEnrollments(): HasMany
    {
        return $this->hasMany(FaceEnrollment::class);
    }

    public function attendances(): HasMany
    {
        return $this->hasMany(Attendance::class);
    }

    public function scopeActive(Builder $query): Builder
    {
        return $query->where('status', 'aktif');
    }

    public function scopeNeedsFaceEnrollment(Builder $query): Builder
    {
        return $query->where('face_enrolled', false)->where('status', 'aktif');
    }

    /**
     * Sudah punya wajah tapi sampelnya kurang — masih bisa absen, tapi
     * akurasinya rentan terhadap perubahan pencahayaan/sudut.
     */
    public function scopeUnderSampled(Builder $query): Builder
    {
        return $query->where('face_enrolled', true)
            ->where('face_sample_count', '<', self::RECOMMENDED_SAMPLES);
    }

    public function getBiometricStatusAttribute(): string
    {
        if (! $this->face_enrolled) {
            return 'belum';
        }

        return $this->face_sample_count >= self::RECOMMENDED_SAMPLES ? 'lengkap' : 'kurang';
    }

    public function getBiometricBadgeAttribute(): string
    {
        return match ($this->biometric_status) {
            'lengkap' => 'badge-light-success',
            'kurang' => 'badge-light-warning',
            default => 'badge-light-danger',
        };
    }
}
