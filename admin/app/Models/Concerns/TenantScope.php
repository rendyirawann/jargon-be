<?php

namespace App\Models\Concerns;

use App\Support\Tenant;
use Illuminate\Database\Eloquent\Builder;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Scope;

/**
 * Global scope yang menyaring baris ke sekolah yang boleh diakses.
 *
 * @see BelongsToSchool untuk alasan pemilihan pendekatan global scope.
 */
class TenantScope implements Scope
{
    public function apply(Builder $builder, Model $model): void
    {
        $allowed = Tenant::schoolIds();

        // null = cakupan provinsi (superadmin/dinas): tanpa penyaringan.
        if ($allowed === null) {
            return;
        }

        // Akun tanpa sekolah tidak melihat apa pun. Ini disengaja: akun yang
        // salah konfigurasi harus gagal aman, bukan terbuka penuh.
        $builder->whereIn(
            $model->qualifyColumn('school_id'),
            $allowed ?: ['00000000-0000-0000-0000-000000000000']
        );
    }
}
