<?php

namespace App\Support;

use App\Models\School;
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\Cache;

/**
 * Penentu cakupan data (tenant) untuk dashboard.
 *
 * Tenant sistem ini adalah SEKOLAH. Aturannya:
 *
 *   - superadmin / admin_dinas  -> cakupan PROVINSI (users.school_id NULL)
 *   - kepala_sekolah/guru/staff -> hanya sekolahnya sendiri
 *   - pengawas                  -> beberapa sekolah (user_school_scopes)
 *
 * Semua penyaringan tenant di dashboard melewati kelas ini. Menaruh aturan
 * ini di satu tempat membuat mustahil ada satu halaman yang lupa memfilter
 * dan membocorkan data sekolah lain.
 */
class Tenant
{
    public const PROVINCE_ROLES = ['superadmin', 'admin_dinas'];

    /**
     * True bila pengguna aktif boleh melihat lintas sekolah.
     */
    public static function isProvinceScope(): bool
    {
        $user = Auth::user();
        if (! $user) {
            return false;
        }

        return $user->hasAnyRole(self::PROVINCE_ROLES);
    }

    /**
     * Daftar id sekolah yang boleh diakses, atau null bila seluruh provinsi.
     *
     * @return array<int, string>|null
     */
    public static function schoolIds(): ?array
    {
        if (self::isProvinceScope()) {
            return null;
        }

        $user = Auth::user();
        if (! $user) {
            return [];
        }

        return Cache::remember(
            "tenant:schools:{$user->id}",
            now()->addMinutes(10),
            function () use ($user) {
                $ids = [];
                if ($user->school_id) {
                    $ids[] = $user->school_id;
                }

                return array_values(array_unique(
                    array_merge($ids, $user->extraSchools()->pluck('schools.id')->all())
                ));
            }
        );
    }

    /**
     * Sekolah "aktif" yang sedang dilihat.
     *
     * Pengguna sekolah selalu terikat pada sekolahnya. Pengguna provinsi bisa
     * memilih lewat query `?school_id=` atau session.
     */
    public static function currentSchoolId(?string $requested = null): ?string
    {
        $allowed = self::schoolIds();

        // Cakupan provinsi: bebas memilih, termasuk tidak memilih (semua).
        if ($allowed === null) {
            if ($requested === null) {
                return session('active_school_id');
            }
            if ($requested === '' || $requested === 'all') {
                session()->forget('active_school_id');

                return null;
            }
            session(['active_school_id' => $requested]);

            return $requested;
        }

        if ($allowed === []) {
            return null;
        }

        // Permintaan yang tidak berhak diabaikan (bukan error) supaya URL
        // yang di-bookmark tidak membuat halaman rusak — data yang tampil
        // tetap hanya sekolah miliknya.
        if ($requested !== null && in_array($requested, $allowed, true)) {
            return $requested;
        }

        return $allowed[0];
    }

    public static function currentSchool(?string $requested = null): ?School
    {
        $id = self::currentSchoolId($requested);

        return $id ? School::find($id) : null;
    }

    /**
     * Gagalkan permintaan bila pengguna tidak berhak atas sekolah tersebut.
     */
    public static function authorizeSchool(?string $schoolId): void
    {
        if ($schoolId === null) {
            return;
        }

        $allowed = self::schoolIds();
        if ($allowed === null) {
            return;
        }

        if (! in_array($schoolId, $allowed, true)) {
            abort(403, 'Anda hanya dapat mengakses data sekolah Anda sendiri.');
        }
    }

    /**
     * Daftar sekolah untuk dropdown pemilih.
     *
     * @return \Illuminate\Support\Collection<int, School>
     */
    public static function selectableSchools()
    {
        $query = School::query()
            ->whereNull('deleted_at')
            ->orderBy('name');

        $allowed = self::schoolIds();
        if ($allowed !== null) {
            $query->whereIn('id', $allowed ?: ['00000000-0000-0000-0000-000000000000']);
        }

        return $query->get(['id', 'name', 'npsn', 'jenjang']);
    }

    public static function forgetCache(?string $userId = null): void
    {
        $userId ??= Auth::id();
        if ($userId) {
            Cache::forget("tenant:schools:{$userId}");
        }
    }
}
