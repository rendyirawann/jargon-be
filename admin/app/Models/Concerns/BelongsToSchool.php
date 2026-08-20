<?php

namespace App\Models\Concerns;

use App\Models\School;
use Illuminate\Database\Eloquent\Builder;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

/**
 * Membatasi model ke sekolah yang boleh diakses pengguna aktif.
 *
 * Diterapkan sebagai GLOBAL SCOPE, bukan sebagai kondisi yang harus ditulis
 * di setiap query. Alasannya: kebocoran data lintas sekolah adalah kegagalan
 * paling serius pada sistem ini, dan mengandalkan setiap pengembang untuk
 * ingat menulis `->where('school_id', ...)` pasti gagal cepat atau lambat.
 * Dengan global scope, yang perlu eksplisit justru sebaliknya —
 * `withoutTenantScope()` untuk kasus yang memang lintas sekolah.
 */
trait BelongsToSchool
{
    public static function bootBelongsToSchool(): void
    {
        static::addGlobalScope(new TenantScope());
    }

    public function school(): BelongsTo
    {
        return $this->belongsTo(School::class);
    }

    /**
     * Lepas penyaringan tenant — hanya untuk laporan tingkat provinsi.
     */
    public static function withoutTenantScope(): Builder
    {
        return static::query()->withoutGlobalScope(TenantScope::class);
    }
}
