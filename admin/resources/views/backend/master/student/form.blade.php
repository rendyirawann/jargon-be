@extends('backend.layout.app')
@section('title', $student->exists ? 'Ubah Siswa' : 'Tambah Siswa')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex align-items-center justify-content-between mt-5 mb-6">
        <h2 class="fw-bold text-gray-900 mb-0">
            {{ $student->exists ? 'Ubah Data Siswa' : 'Tambah Siswa Baru' }}
        </h2>
        <a href="{{ route('students.index') }}" class="btn btn-sm btn-light">
            <i class="ki-outline ki-arrow-left fs-5 me-1"></i>Kembali
        </a>
    </div>

    <form method="POST"
          action="{{ $student->exists ? route('students.update', $student) : route('students.store') }}">
        @csrf
        @if ($student->exists) @method('PUT') @endif

        <div class="row g-5">
            <div class="col-xl-8">
                <div class="card card-flush border border-gray-200">
                    <div class="card-header pt-5"><h3 class="card-title fw-bold">Data Pokok</h3></div>
                    <div class="card-body pt-3">
                        <div class="row g-4">
                            @unless ($student->exists)
                                <div class="col-md-6">
                                    <label class="form-label required">Sekolah</label>
                                    <select name="school_id" class="form-select" required id="schoolSelect">
                                        <option value="">Pilih sekolah</option>
                                        @foreach ($schools as $s)
                                            <option value="{{ $s->id }}"
                                                {{ old('school_id', $student->school_id) === $s->id ? 'selected' : '' }}>
                                                {{ $s->name }}
                                            </option>
                                        @endforeach
                                    </select>
                                </div>
                            @else
                                <div class="col-md-6">
                                    <label class="form-label">Sekolah</label>
                                    <input type="text" class="form-control" value="{{ $student->school->name ?? '-' }}" disabled>
                                    <span class="form-text fs-9">
                                        Perpindahan sekolah dilakukan lewat proses mutasi, bukan dari form ini.
                                    </span>
                                </div>
                            @endunless

                            <div class="col-md-6">
                                <label class="form-label">Kelas</label>
                                <select name="current_classroom_id" class="form-select" id="classroomSelect">
                                    <option value="">Belum ditempatkan</option>
                                    @foreach ($classrooms as $c)
                                        <option value="{{ $c->id }}"
                                            {{ old('current_classroom_id', $student->current_classroom_id) === $c->id ? 'selected' : '' }}>
                                            {{ $c->name }}
                                        </option>
                                    @endforeach
                                </select>
                            </div>

                            <div class="col-md-8">
                                <label class="form-label required">Nama lengkap</label>
                                <input type="text" name="full_name" class="form-control" required minlength="2" maxlength="150"
                                       value="{{ old('full_name', $student->full_name) }}">
                            </div>
                            <div class="col-md-4">
                                <label class="form-label">Jenis kelamin</label>
                                <select name="gender" class="form-select">
                                    <option value="">-</option>
                                    <option value="L" {{ old('gender', $student->gender) === 'L' ? 'selected' : '' }}>Laki-laki</option>
                                    <option value="P" {{ old('gender', $student->gender) === 'P' ? 'selected' : '' }}>Perempuan</option>
                                </select>
                            </div>

                            <div class="col-md-4">
                                <label class="form-label">NISN</label>
                                <input type="text" name="nisn" class="form-control" inputmode="numeric" maxlength="10"
                                       value="{{ old('nisn', $student->nisn) }}" placeholder="10 digit">
                                <span class="form-text fs-9">Unik secara nasional.</span>
                            </div>
                            <div class="col-md-4">
                                <label class="form-label">NIS</label>
                                <input type="text" name="nis" class="form-control" maxlength="20"
                                       value="{{ old('nis', $student->nis) }}">
                                <span class="form-text fs-9">Unik dalam sekolah ini.</span>
                            </div>
                            <div class="col-md-4">
                                <label class="form-label">Tahun masuk</label>
                                <input type="number" name="entry_year" class="form-control" min="1990" max="2100"
                                       value="{{ old('entry_year', $student->entry_year) }}">
                            </div>

                            <div class="col-md-4">
                                <label class="form-label">Tempat lahir</label>
                                <input type="text" name="birth_place" class="form-control" maxlength="100"
                                       value="{{ old('birth_place', $student->birth_place) }}">
                            </div>
                            <div class="col-md-4">
                                <label class="form-label">Tanggal lahir</label>
                                <input type="date" name="birth_date" class="form-control" max="{{ now()->toDateString() }}"
                                       value="{{ old('birth_date', $student->birth_date?->toDateString()) }}">
                            </div>
                            <div class="col-md-4">
                                <label class="form-label">Agama</label>
                                <select name="religion" class="form-select">
                                    <option value="">-</option>
                                    @foreach (['Islam', 'Kristen', 'Katolik', 'Hindu', 'Buddha', 'Konghucu', 'Lainnya'] as $r)
                                        <option value="{{ $r }}" {{ old('religion', $student->religion) === $r ? 'selected' : '' }}>{{ $r }}</option>
                                    @endforeach
                                </select>
                            </div>

                            <div class="col-12">
                                <label class="form-label">Alamat</label>
                                <textarea name="address" class="form-control" rows="2" maxlength="500">{{ old('address', $student->address) }}</textarea>
                            </div>

                            <div class="col-md-4">
                                <label class="form-label">No. HP siswa</label>
                                <input type="text" name="phone" class="form-control" maxlength="20"
                                       value="{{ old('phone', $student->phone) }}">
                            </div>
                            <div class="col-md-4">
                                <label class="form-label">Nama ayah</label>
                                <input type="text" name="father_name" class="form-control" maxlength="150"
                                       value="{{ old('father_name', $student->father_name) }}">
                            </div>
                            <div class="col-md-4">
                                <label class="form-label">Nama ibu</label>
                                <input type="text" name="mother_name" class="form-control" maxlength="150"
                                       value="{{ old('mother_name', $student->mother_name) }}">
                            </div>

                            @if ($student->exists)
                                <div class="col-md-4">
                                    <label class="form-label">Status siswa</label>
                                    <select name="status" class="form-select">
                                        @foreach (\App\Models\Student::STATUS as $s)
                                            <option value="{{ $s }}" {{ old('status', $student->status) === $s ? 'selected' : '' }}>
                                                {{ ucfirst($s) }}
                                            </option>
                                        @endforeach
                                    </select>
                                    <span class="form-text fs-9">
                                        Status selain <em>aktif</em> membuat siswa berhenti dikenali tablet.
                                    </span>
                                </div>
                            @endif
                        </div>
                    </div>
                </div>
            </div>

            <div class="col-xl-4">
                @unless ($student->exists)
                    <div class="card card-flush border border-gray-200">
                        <div class="card-header pt-5">
                            <h3 class="card-title fw-bold">Wali Murid</h3>
                        </div>
                        <div class="card-body pt-3">
                            <div class="alert alert-light-primary py-3 px-4 fs-8 mb-4">
                                Tanpa kontak wali, notifikasi absensi tidak punya tujuan. Wali pertama
                                otomatis menjadi kontak utama.
                            </div>

                            <div class="mb-3">
                                <label class="form-label">Hubungan</label>
                                <select name="guardian_relation" class="form-select form-select-sm">
                                    @foreach (\App\Models\StudentGuardian::RELATIONS as $r)
                                        <option value="{{ $r }}" {{ old('guardian_relation') === $r ? 'selected' : '' }}>
                                            {{ ucfirst($r) }}
                                        </option>
                                    @endforeach
                                </select>
                            </div>
                            <div class="mb-3">
                                <label class="form-label">Nama wali</label>
                                <input type="text" name="guardian_full_name" class="form-control form-control-sm"
                                       value="{{ old('guardian_full_name') }}" maxlength="150">
                            </div>
                            <div class="mb-3">
                                <label class="form-label">Kanal notifikasi</label>
                                <select name="guardian_preferred_channel" class="form-select form-select-sm">
                                    @foreach (\App\Models\StudentGuardian::CHANNELS as $c)
                                        <option value="{{ $c }}" {{ old('guardian_preferred_channel', 'whatsapp') === $c ? 'selected' : '' }}>
                                            {{ $c === 'none' ? 'Tidak dikirimi' : ucfirst($c) }}
                                        </option>
                                    @endforeach
                                </select>
                            </div>
                            <div class="mb-3">
                                <label class="form-label">Nomor WhatsApp</label>
                                <input type="text" name="guardian_whatsapp" class="form-control form-control-sm"
                                       value="{{ old('guardian_whatsapp') }}" placeholder="08xxxxxxxxxx">
                                <span class="form-text fs-9">Format apa pun; otomatis dinormalisasi ke 62xxx.</span>
                            </div>
                            <div class="mb-3">
                                <label class="form-label">No. HP lain</label>
                                <input type="text" name="guardian_phone" class="form-control form-control-sm"
                                       value="{{ old('guardian_phone') }}">
                            </div>
                            <div class="mb-3">
                                <label class="form-label">Email</label>
                                <input type="email" name="guardian_email" class="form-control form-control-sm"
                                       value="{{ old('guardian_email') }}">
                            </div>
                            <div class="mb-0">
                                <label class="form-label">Telegram chat ID</label>
                                <input type="text" name="guardian_telegram_chat_id" class="form-control form-control-sm"
                                       value="{{ old('guardian_telegram_chat_id') }}">
                                <span class="form-text fs-9">Diperoleh setelah wali menekan /start pada bot sekolah.</span>
                            </div>
                        </div>
                    </div>
                @else
                    <div class="card card-flush border border-gray-200">
                        <div class="card-body p-5">
                            <span class="text-muted fs-8">
                                Pengelolaan wali murid dilakukan di halaman detail siswa.
                            </span>
                            <a href="{{ route('students.show', $student) }}" class="btn btn-sm btn-light w-100 mt-3">
                                Buka detail siswa
                            </a>
                        </div>
                    </div>
                @endunless

                <div class="d-flex gap-2 mt-5">
                    <button class="btn btn-primary flex-grow-1">
                        {{ $student->exists ? 'Simpan Perubahan' : 'Simpan Siswa' }}
                    </button>
                    <a href="{{ route('students.index') }}" class="btn btn-light">Batal</a>
                </div>
            </div>
        </div>
    </form>
@endsection

@push('scripts')
    <script>
        // Dropdown kelas mengikuti sekolah yang dipilih — daftar kelas satu
        // sekolah tidak boleh muncul untuk sekolah lain.
        (function () {
            const school = document.getElementById('schoolSelect');
            const classroom = document.getElementById('classroomSelect');
            if (!school || !classroom) return;

            school.addEventListener('change', async function () {
                classroom.innerHTML = '<option value="">Memuat...</option>';
                if (!school.value) {
                    classroom.innerHTML = '<option value="">Belum ditempatkan</option>';
                    return;
                }
                try {
                    const url = new URL(@json(route('students.classrooms')), window.location.origin);
                    url.searchParams.set('school_id', school.value);
                    const res = await fetch(url, { headers: { Accept: 'application/json' } });
                    const items = res.ok ? await res.json() : [];

                    classroom.replaceChildren();
                    const blank = document.createElement('option');
                    blank.value = '';
                    blank.textContent = 'Belum ditempatkan';
                    classroom.append(blank);

                    items.forEach(function (item) {
                        const opt = document.createElement('option');
                        opt.value = item.id;
                        opt.textContent = item.text;
                        classroom.append(opt);
                    });
                } catch (e) {
                    classroom.innerHTML = '<option value="">Gagal memuat kelas</option>';
                }
            });
        })();
    </script>
@endpush
