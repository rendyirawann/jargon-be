{{--
    Pemilih sekolah aktif.

    Hanya muncul untuk peran tingkat provinsi (Superadmin / Admin Dinas).
    Pengguna tingkat sekolah tidak diberi pilihan sama sekali — bukan
    disembunyikan lewat CSS, tetapi memang tidak dirender, dan andai pun
    di-POST tetap ditolak App\Support\Tenant.

    Variabel: $schools (koleksi), $schoolId (aktif), $allowAll (bool)
--}}
@if (($schools ?? collect())->isNotEmpty())
    <form method="GET" class="d-flex align-items-center gap-2">
        {{-- Parameter lain pada URL dipertahankan agar filter tidak hilang
             saat pengguna berganti sekolah. --}}
        @foreach (request()->except(['school_id', 'page']) as $key => $value)
            @if (! is_array($value))
                <input type="hidden" name="{{ $key }}" value="{{ $value }}">
            @endif
        @endforeach

        <span class="text-muted fs-8 text-nowrap">Sekolah</span>
        <select name="school_id" class="form-select form-select-sm w-250px" onchange="this.form.submit()">
            @if ($allowAll ?? true)
                <option value="all" {{ $schoolId ? '' : 'selected' }}>— Seluruh Provinsi —</option>
            @else
                <option value="">— Pilih sekolah —</option>
            @endif
            @foreach ($schools as $s)
                <option value="{{ $s->id }}" {{ $schoolId === $s->id ? 'selected' : '' }}>
                    {{ $s->name }}@if (! empty($s->npsn)) ({{ $s->npsn }}) @endif
                </option>
            @endforeach
        </select>
    </form>
@endif
