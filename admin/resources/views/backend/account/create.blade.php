@extends('backend.layout.app')
@section('title', 'Buat Akun Jargon GO')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Buat Akun Aplikasi</h2>
            <span class="text-muted fs-7">
                Pengguna wajib mengganti kata sandi saat login pertama.
            </span>
        </div>
        <a href="{{ route('app-accounts.index') }}" class="btn btn-sm btn-light">Kembali</a>
    </div>

    <form method="POST" action="{{ route('app-accounts.store') }}" id="form-akun">
        @csrf
        <div class="row g-5">
            <div class="col-xl-8">
                <div class="card card-flush border border-gray-200 mb-5">
                    <div class="card-header pt-5"><h3 class="card-title fw-bold">Identitas</h3></div>
                    <div class="card-body pt-3">
                        <div class="row g-4">
                            <div class="col-md-6">
                                <label class="form-label required">Peran</label>
                                <select name="role" id="role" class="form-select form-select-sm" required>
                                    @foreach ($roles as $key => $label)
                                        <option value="{{ $key }}" {{ old('role') === $key ? 'selected' : '' }}>
                                            {{ $label }}
                                        </option>
                                    @endforeach
                                </select>
                            </div>
                            <div class="col-md-6">
                                <label class="form-label required" id="identity-label">NIK</label>
                                <input type="text" name="identity_number" id="identity_number"
                                       class="form-control form-control-sm @error('identity_number') is-invalid @enderror"
                                       value="{{ old('identity_number') }}" required
                                       inputmode="numeric" pattern="[0-9]*" maxlength="16">
                                <span class="form-text fs-9" id="identity-hint">
                                    16 digit, sesuai KTP.
                                </span>
                                @error('identity_number')
                                    <div class="invalid-feedback">{{ $message }}</div>
                                @enderror
                            </div>
                            <div class="col-md-6">
                                <label class="form-label required">Nama Lengkap</label>
                                <input type="text" name="name" class="form-control form-control-sm"
                                       value="{{ old('name') }}" required minlength="3" maxlength="150">
                            </div>
                            <div class="col-md-6">
                                <label class="form-label required">Email</label>
                                <input type="email" name="email" class="form-control form-control-sm"
                                       value="{{ old('email') }}" required>
                            </div>
                            <div class="col-md-6">
                                <label class="form-label required">Kata Sandi Awal</label>
                                <input type="text" name="password" class="form-control form-control-sm"
                                       value="{{ old('password') }}" required minlength="8">
                                <span class="form-text fs-9">
                                    Jangan memakai NIK, NISN, atau tanggal lahir &mdash; ketiganya
                                    tercetak di dokumen sekolah dan diketahui orang lain.
                                </span>
                            </div>
                            <div class="col-md-6">
                                <label class="form-label">Nomor HP</label>
                                <input type="text" name="phone" class="form-control form-control-sm"
                                       value="{{ old('phone') }}" maxlength="15">
                            </div>
                        </div>
                    </div>
                </div>

                {{-- Bagian kepegawaian: hanya relevan untuk guru/staf/kepsek. --}}
                <div class="card card-flush border border-gray-200 mb-5" id="blok-kepegawaian">
                    <div class="card-header pt-5"><h3 class="card-title fw-bold">Data Kepegawaian</h3></div>
                    <div class="card-body pt-3">
                        <div class="row g-4">
                            <div class="col-md-6">
                                <label class="form-label">NIP / NUPTK</label>
                                <input type="text" name="employee_no" class="form-control form-control-sm"
                                       value="{{ old('employee_no') }}" maxlength="30">
                            </div>
                            <div class="col-md-6">
                                <label class="form-label">Jabatan</label>
                                <input type="text" name="position" class="form-control form-control-sm"
                                       value="{{ old('position') }}" maxlength="100">
                            </div>
                        </div>
                    </div>
                </div>

                {{-- Tautan siswa: wajib untuk siswa dan orang tua. --}}
                <div class="card card-flush border border-gray-200" id="blok-siswa">
                    <div class="card-header pt-5">
                        <h3 class="card-title fw-bold">Tautan Siswa</h3>
                    </div>
                    <div class="card-body pt-3">
                        <div class="alert alert-light-info py-3 px-4 fs-9 mb-4" id="hint-siswa"></div>

                            <div class="mb-3">
                                <label class="form-label" for="pilih-siswa">Data siswa</label>
                                <select id="pilih-siswa" name="student_ids[]" multiple
                                        class="form-select form-select-sm"
                                        data-url="{{ route('app-accounts.students') }}"
                                        data-placeholder="Ketik nama atau NISN (minimal 3 huruf)"></select>
                                <div class="form-text fs-9">Cari berdasarkan nama atau NISN.</div>
                            </div>



                        <div class="mt-4" id="blok-relasi">
                            <label class="form-label required">Hubungan dengan siswa</label>
                            <select name="guardian_relation" class="form-select form-select-sm w-auto">
                                @foreach ($relations as $key => $label)
                                    <option value="{{ $key }}" {{ old('guardian_relation') === $key ? 'selected' : '' }}>
                                        {{ $label }}
                                    </option>
                                @endforeach
                            </select>
                        </div>
                    </div>
                </div>
            </div>

            <div class="col-xl-4">
                <div class="card card-flush border border-gray-200 mb-5" id="blok-sekolah">
                    <div class="card-header pt-5"><h3 class="card-title fw-bold">Sekolah</h3></div>
                    <div class="card-body pt-3">
                        <select name="school_id" class="form-select form-select-sm">
                            <option value="">&mdash; tanpa sekolah (tingkat provinsi) &mdash;</option>
                            @foreach ($schools as $s)
                                <option value="{{ $s->id }}" {{ old('school_id') === $s->id ? 'selected' : '' }}>
                                    {{ $s->name }}
                                </option>
                            @endforeach
                        </select>
                        <span class="form-text fs-9 mt-2 d-block" id="hint-sekolah"></span>
                    </div>
                </div>

                <div class="card card-flush border border-gray-200">
                    <div class="card-body p-5">
                        <button class="btn btn-primary w-100 mb-3">Buat Akun</button>
                        <span class="text-muted fs-9">
                            Akun dibuat lewat API agar aturan panjang NIK/NISN, pewarisan
                            sekolah dari data siswa, dan pencatatan audit berlaku sama
                            seperti pendaftaran dari sumber lain.
                        </span>
                    </div>
                </div>
            </div>
        </div>
    </form>
@endsection

@push('scripts')
<script>
(function () {
    const role = document.getElementById('role');
    const identity = document.getElementById('identity_number');
    const identityLabel = document.getElementById('identity-label');
    const identityHint = document.getElementById('identity-hint');
    const blokSiswa = document.getElementById('blok-siswa');
    const blokRelasi = document.getElementById('blok-relasi');
    const blokKepegawaian = document.getElementById('blok-kepegawaian');
    const blokSekolah = document.getElementById('blok-sekolah');
    const hintSiswa = document.getElementById('hint-siswa');
    const hintSekolah = document.getElementById('hint-sekolah');

    function render() {
        const value = role.value;
        const isSiswa = value === 'siswa';
        const isOrtu = value === 'orang_tua';
        const butuhSiswa = isSiswa || isOrtu;

        // NISN 10 digit untuk siswa, NIK 16 digit untuk sisanya.
        identityLabel.textContent = isSiswa ? 'NISN' : 'NIK';
        identity.maxLength = isSiswa ? 10 : 16;
        identityHint.textContent = isSiswa
            ? '10 digit, sesuai data Dapodik.'
            : '16 digit, sesuai KTP.';

        blokSiswa.style.display = butuhSiswa ? '' : 'none';
        blokRelasi.style.display = isOrtu ? '' : 'none';
        blokKepegawaian.style.display = butuhSiswa ? 'none' : '';
        // Akun orang tua tidak terikat satu sekolah: anaknya bisa berbeda
        // sekolah, dan mengikatnya justru memberi akses ke seluruh siswa
        // sekolah itu. Akun siswa mewarisi sekolah dari data siswanya.
        blokSekolah.style.display = butuhSiswa ? 'none' : '';

        hintSiswa.textContent = isSiswa
            ? 'Pilih satu data siswa. Sekolah akun akan mengikuti sekolah siswa tersebut.'
            : 'Pilih anak-anak yang boleh dipantau akun ini. Bisa lebih dari satu, termasuk yang berbeda sekolah.';

        hintSekolah.textContent = value === 'petugas_pengaduan'
            ? 'Petugas pengaduan bercakupan provinsi; biarkan kosong.'
            : 'Wajib diisi untuk guru, staf, dan kepala sekolah.';

        // Ganti peran ke 'siswa' saat sudah memilih beberapa: sisakan satu.
        if (isSiswa && window.jQuery) {
            const v = $('#pilih-siswa').val() || [];
            if (v.length > 1) $('#pilih-siswa').val([v[0]]).trigger('change');
        }
    }

    // Dropdown siswa yang bisa dicari.
    //
    // Sebelumnya berupa kotak cari + daftar hasil + daftar terpilih terpisah:
    // tiga blok untuk satu pilihan. Select2 (sudah ada di plugins.bundle
    // Metronic) menyatukannya, dan pencariannya tetap ke endpoint yang sama
    // sehingga penyaringan lingkup sekolah di server tidak berubah.
    const pilihSiswa = $('#pilih-siswa');
    pilihSiswa.select2({
        width: '100%',
        placeholder: pilihSiswa.data('placeholder'),
        minimumInputLength: 3,
        language: {
            inputTooShort: function () { return 'Ketik minimal 3 huruf'; },
            searching: function () { return 'Mencari...'; },
            noResults: function () { return 'Tidak ada siswa yang cocok'; },
        },
        ajax: {
            url: pilihSiswa.data('url'),
            dataType: 'json',
            delay: 250,
            data: function (params) { return { q: params.term }; },
            processResults: function (json) {
                return {
                    results: (json.data || []).map(function (s) {
                        return {
                            id: s.id,
                            text: s.name + ' — ' + (s.nisn || 'tanpa NISN')
                                  + ' · ' + s.classroom + ' · ' + s.school,
                        };
                    }),
                };
            },
            cache: true,
        },
    });
    
    // Akun siswa menaut TEPAT satu siswa; akun orang tua boleh beberapa anak.
    pilihSiswa.on('select2:select', function () {
        if (role.value !== 'siswa') return;
        const v = pilihSiswa.val() || [];
        if (v.length > 1) pilihSiswa.val([v[v.length - 1]]).trigger('change');
    });

    role.addEventListener('change', render);
    render();
})();
</script>
@endpush
