@extends('backend.layout.app')
@section('title', 'Kelas / Rombel')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Kelas / Rombongan Belajar</h2>
            <span class="text-muted fs-7">
                Tahun ajaran aktif: {{ $activeYear->name ?? 'belum ditetapkan' }}
            </span>
        </div>
        @include('backend.partials._school_picker', ['schools' => $schools, 'schoolId' => $schoolId, 'allowAll' => false])
    </div>

    @unless ($activeYear)
        <div class="alert alert-warning">
            Belum ada tahun ajaran aktif. Kelas tidak dapat dibuat sampai tahun ajaran ditetapkan
            (lihat tabel <code>academic_years</code>).
        </div>
    @endunless

    <div class="row g-5">
        <div class="col-xl-8">
            <div class="card card-flush border border-gray-200">
                <div class="card-body p-0">
                    <div class="table-responsive">
                        <table class="table table-row-bordered align-middle mb-0">
                            <thead class="bg-light">
                                <tr class="fw-bold fs-8 text-uppercase text-muted">
                                    <th class="ps-5">Kelas</th>
                                    <th>Tingkat</th>
                                    <th>Wali Kelas</th>
                                    <th class="text-center">Siswa</th>
                                    <th class="text-center">Kapasitas</th>
                                    <th class="text-end pe-5">Aksi</th>
                                </tr>
                            </thead>
                            <tbody>
                                @forelse ($classrooms as $c)
                                    <tr class="{{ $c->is_active ? '' : 'opacity-50' }}">
                                        <td class="ps-5">
                                            <span class="fw-semibold fs-7">{{ $c->name }}</span>
                                            @if ($c->major)
                                                <span class="text-muted fs-9 d-block">{{ $c->major }}</span>
                                            @endif
                                        </td>
                                        <td class="fs-7">{{ $c->grade_level }}</td>
                                        <td class="fs-7">
                                            {{ $c->homeroomTeacher->name ?? '—' }}
                                            @unless ($c->homeroom_teacher_id)
                                                <span class="badge badge-light-warning fs-9 d-block mt-1">belum ditetapkan</span>
                                            @endunless
                                        </td>
                                        <td class="text-center">
                                            <a href="{{ route('students.index', ['school_id' => $schoolId, 'classroom_id' => $c->id]) }}"
                                               class="fw-bold">{{ $c->students_count }}</a>
                                        </td>
                                        <td class="text-center fs-7">
                                            {{ $c->capacity }}
                                            @if ($c->students_count > $c->capacity)
                                                <span class="badge badge-light-danger fs-9 d-block mt-1">melebihi</span>
                                            @endif
                                        </td>
                                        <td class="text-end pe-5">
                                            <div class="d-flex justify-content-end gap-1">
                                                @can('update_classroom')
                                                    <button class="btn btn-icon btn-sm btn-light-warning"
                                                            data-bs-toggle="modal" data-bs-target="#editKelas{{ $loop->index }}">
                                                        <i class="ki-outline ki-pencil fs-5"></i>
                                                    </button>
                                                @endcan
                                                @can('delete_classroom')
                                                    <form method="POST" action="{{ route('classrooms.destroy', $c) }}"
                                                          onsubmit="return confirm('Hapus kelas {{ addslashes($c->name) }}?');">
                                                        @csrf @method('DELETE')
                                                        <button class="btn btn-icon btn-sm btn-light-danger">
                                                            <i class="ki-outline ki-trash fs-5"></i>
                                                        </button>
                                                    </form>
                                                @endcan
                                            </div>
                                        </td>
                                    </tr>

                                    @can('update_classroom')
                                        <div class="modal fade" id="editKelas{{ $loop->index }}" tabindex="-1" aria-hidden="true">
                                            <div class="modal-dialog modal-dialog-centered">
                                                <form method="POST" action="{{ route('classrooms.update', $c) }}" class="modal-content">
                                                    @csrf @method('PUT')
                                                    <div class="modal-header">
                                                        <h4 class="modal-title">Ubah {{ $c->name }}</h4>
                                                        <button type="button" class="btn btn-icon btn-sm" data-bs-dismiss="modal">
                                                            <i class="ki-outline ki-cross fs-2"></i>
                                                        </button>
                                                    </div>
                                                    <div class="modal-body">
                                                        <div class="row g-3">
                                                            <div class="col-8">
                                                                <label class="form-label required">Nama kelas</label>
                                                                <input type="text" name="name" class="form-control" required
                                                                       value="{{ $c->name }}" maxlength="60">
                                                            </div>
                                                            <div class="col-4">
                                                                <label class="form-label required">Tingkat</label>
                                                                <input type="number" name="grade_level" class="form-control" required
                                                                       min="1" max="13" value="{{ $c->grade_level }}">
                                                            </div>
                                                            <div class="col-8">
                                                                <label class="form-label">Jurusan</label>
                                                                <input type="text" name="major" class="form-control"
                                                                       value="{{ $c->major }}" maxlength="60">
                                                            </div>
                                                            <div class="col-4">
                                                                <label class="form-label">Kapasitas</label>
                                                                <input type="number" name="capacity" class="form-control"
                                                                       min="1" max="100" value="{{ $c->capacity }}">
                                                            </div>
                                                            <div class="col-12">
                                                                <label class="form-label">Wali kelas</label>
                                                                <select name="homeroom_teacher_id" class="form-select">
                                                                    <option value="">Belum ditetapkan</option>
                                                                    @foreach ($teachers as $t)
                                                                        <option value="{{ $t->id }}" {{ $c->homeroom_teacher_id === $t->id ? 'selected' : '' }}>
                                                                            {{ $t->name }}
                                                                        </option>
                                                                    @endforeach
                                                                </select>
                                                            </div>
                                                            <div class="col-12">
                                                                <label class="form-check form-check-custom">
                                                                    <input type="checkbox" class="form-check-input" name="is_active"
                                                                           value="1" {{ $c->is_active ? 'checked' : '' }}>
                                                                    <span class="form-check-label fs-7">Kelas aktif</span>
                                                                </label>
                                                            </div>
                                                        </div>
                                                    </div>
                                                    <div class="modal-footer">
                                                        <button type="button" class="btn btn-light" data-bs-dismiss="modal">Batal</button>
                                                        <button class="btn btn-primary">Simpan</button>
                                                    </div>
                                                </form>
                                            </div>
                                        </div>
                                    @endcan
                                @empty
                                    <tr>
                                        <td colspan="6" class="text-center text-muted py-10 fs-7">
                                            @if (! $schoolId)
                                                Pilih sekolah terlebih dahulu.
                                            @else
                                                Belum ada kelas pada tahun ajaran aktif.
                                            @endif
                                        </td>
                                    </tr>
                                @endforelse
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>
        </div>

        <div class="col-xl-4">
            @can('create_classroom')
                <div class="card card-flush border border-gray-200">
                    <div class="card-header pt-5"><h3 class="card-title fw-bold">Tambah Kelas</h3></div>
                    <form method="POST" action="{{ route('classrooms.store') }}" class="card-body pt-3">
                        @csrf
                        <input type="hidden" name="school_id" value="{{ $schoolId }}">

                        <div class="row g-3">
                            <div class="col-8">
                                <label class="form-label required">Nama kelas</label>
                                <input type="text" name="name" class="form-control form-control-sm" required
                                       placeholder="X IPA 1" maxlength="60" value="{{ old('name') }}">
                            </div>
                            <div class="col-4">
                                <label class="form-label required">Tingkat</label>
                                <input type="number" name="grade_level" class="form-control form-control-sm" required
                                       min="1" max="13" value="{{ old('grade_level') }}">
                            </div>
                            <div class="col-8">
                                <label class="form-label">Jurusan</label>
                                <input type="text" name="major" class="form-control form-control-sm"
                                       placeholder="IPA / IPS / TKJ" maxlength="60" value="{{ old('major') }}">
                            </div>
                            <div class="col-4">
                                <label class="form-label">Kapasitas</label>
                                <input type="number" name="capacity" class="form-control form-control-sm"
                                       min="1" max="100" value="{{ old('capacity', 36) }}">
                            </div>
                            <div class="col-12">
                                <label class="form-label">Wali kelas</label>
                                <select name="homeroom_teacher_id" class="form-select form-select-sm">
                                    <option value="">Belum ditetapkan</option>
                                    @foreach ($teachers as $t)
                                        <option value="{{ $t->id }}">{{ $t->name }}</option>
                                    @endforeach
                                </select>
                                <span class="form-text fs-9">
                                    Hanya pegawai sekolah ini yang dapat dipilih.
                                </span>
                            </div>
                        </div>

                        <button class="btn btn-primary w-100 mt-4" {{ $schoolId && $activeYear ? '' : 'disabled' }}>
                            Tambah kelas
                        </button>
                    </form>
                </div>
            @endcan
        </div>
    </div>
@endsection
