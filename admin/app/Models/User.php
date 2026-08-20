<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Foundation\Auth\User as Authenticatable;
use Illuminate\Notifications\Notifiable;
use Spatie\Permission\Traits\HasRoles;
use Cog\Contracts\Ban\Bannable as BannableContract;
use Cog\Laravel\Ban\Traits\Bannable;
use Illuminate\Database\Eloquent\Concerns\HasUuids; // 1. Ini penawar errornya
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\BelongsToMany;
use Illuminate\Database\Eloquent\Relations\HasMany;
use App\Support\Tenant;

class User extends Authenticatable implements BannableContract
{
    /** @use HasFactory<\Database\Factories\UserFactory> */
    // 2. Masukkan HasUuids ke dalam use
    use HasFactory, Notifiable, HasRoles, Bannable, HasUuids;

    /**
     * The attributes that are mass assignable.
     *
     * @var list<string>
     */
    protected $fillable = [
        'name',
        'username',
        'email',
        'no_wa',
        'avatar',
        'last_ip',
        'last_login',
        'banned_at',
        'nik',
        'phone',
        'is_active',
        'password',
        'social_id',
        'social_type',
        // --- Absensi Face Recognition ---
        'school_id',
        'employee_no',
        'position',
        'telegram_chat_id',
        'must_change_password',
        // --- Jargon GO ---
        // Identitas login aplikasi: NIK (16 digit) untuk guru/staff/orang tua,
        // NISN (10 digit) untuk siswa. Satu kolom, karena layar login hanya
        // punya satu kotak isian.
        'identity_number',
        'identity_type',
        'student_id',
    ];

    /**
     * The attributes that should be hidden for serialization.
     *
     * @var list<string>
     */
    protected $hidden = [
        'password',
        'remember_token',
    ];

    /**
     * Get the attributes that should be cast.
     *
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'email_verified_at' => 'datetime',
            'last_login' => 'datetime',
            'banned_at' => 'datetime',
            'password' => 'hashed',
            'is_active' => 'boolean',
            'must_change_password' => 'boolean',
        ];
    }

    // =================================================================
    // Absensi Face Recognition — cakupan sekolah
    // =================================================================

    /**
     * Sekolah utama. NULL = peran tingkat provinsi (superadmin/admin_dinas).
     */
    public function school(): BelongsTo
    {
        return $this->belongsTo(School::class);
    }

    /**
     * Sekolah tambahan (mis. pengawas yang membina beberapa sekolah).
     */
    public function extraSchools(): BelongsToMany
    {
        // Tabel pivot hanya punya created_at (tanpa updated_at), jadi
        // withTimestamps() tidak dipakai agar Eloquent tidak menulis kolom
        // yang tidak ada.
        return $this->belongsToMany(School::class, 'user_school_scopes', 'user_id', 'school_id')
            ->withPivot('created_at');
    }

    /**
     * Kelas yang diampu sebagai wali kelas.
     */
    public function homeroomClassrooms(): HasMany
    {
        return $this->hasMany(Classroom::class, 'homeroom_teacher_id');
    }

    // =================================================================
    // Jargon GO — tautan ke data siswa
    // =================================================================

    /**
     * Data siswa untuk akun berperan `siswa`.
     *
     * Akun siswa hanya dipakai MEMBACA datanya sendiri di aplikasi; absensi
     * tetap berjalan lewat pengenalan wajah di tablet tanpa login.
     */
    public function student(): BelongsTo
    {
        return $this->belongsTo(Student::class, 'student_id');
    }

    /**
     * Anak-anak untuk akun berperan `orang_tua`.
     *
     * Cakupan akun orang tua diturunkan dari sini, BUKAN dari `school_id`:
     * anak-anaknya bisa bersekolah di tempat yang berbeda, dan mengikat akun
     * ke satu sekolah justru memberinya akses ke seluruh siswa sekolah itu.
     */
    public function children(): BelongsToMany
    {
        return $this->belongsToMany(Student::class, 'student_guardians', 'user_id', 'student_id')
            ->withPivot(['relation', 'is_primary', 'phone']);
    }

    /** Label jenis identitas login untuk ditampilkan di UI. */
    public function getIdentityLabelAttribute(): string
    {
        return match ($this->identity_type) {
            'nisn' => 'NISN',
            'nik' => 'NIK',
            default => 'Identitas',
        };
    }

    public function isProvinceScope(): bool
    {
        return $this->hasAnyRole(Tenant::PROVINCE_ROLES);
    }

    public function isSuperadmin(): bool
    {
        return $this->hasRole('superadmin');
    }

    /**
     * Nama peran utama untuk ditampilkan di UI.
     */
    public function getRoleLabelAttribute(): string
    {
        return match ($this->roles->first()?->name) {
            'superadmin' => 'Superadmin',
            'admin_dinas' => 'Admin Dinas',
            'kepala_sekolah' => 'Kepala Sekolah',
            'guru' => 'Guru',
            'staff_tu' => 'Staff TU',
            'siswa' => 'Siswa',
            'orang_tua' => 'Orang Tua',
            'petugas_pengaduan' => 'Petugas Pengaduan',
            default => $this->roles->first()->name ?? 'Pengguna',
        };
    }

    /**
     * Label cakupan: nama sekolah, atau "Provinsi Sumatera Utara".
     */
    public function getScopeLabelAttribute(): string
    {
        if ($this->isProvinceScope()) {
            return 'Provinsi Sumatera Utara';
        }

        return $this->school?->name ?? 'Belum ditautkan ke sekolah';
    }

    protected static function booted(): void
    {
        // Cakupan sekolah di-cache per pengguna; harus dibuang bila
        // penempatannya berubah, kalau tidak pengguna akan tetap melihat
        // (atau kehilangan) data sekolah lama sampai cache kedaluwarsa.
        static::saved(function (self $user) {
            if ($user->wasChanged('school_id')) {
                Tenant::forgetCache($user->id);
            }
        });

        static::deleted(function (self $user) {
            Tenant::forgetCache($user->id);
        });
    }
}
