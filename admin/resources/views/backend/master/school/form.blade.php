@extends('backend.layout.app')
@section('title', $school->exists ? 'Ubah Sekolah' : 'Tambah Sekolah')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex align-items-center justify-content-between mt-5 mb-6">
        <h2 class="fw-bold text-gray-900 mb-0">{{ $school->exists ? 'Ubah Data Sekolah' : 'Tambah Sekolah' }}</h2>
        <a href="{{ route('schools.index') }}" class="btn btn-sm btn-light">Kembali</a>
    </div>

    <form method="POST" action="{{ $school->exists ? route('schools.update', $school) : route('schools.store') }}">
        @csrf
        @if ($school->exists) @method('PUT') @endif

        <div class="row g-5">
            <div class="col-xl-8">
                <div class="card card-flush border border-gray-200">
                    <div class="card-header pt-5"><h3 class="card-title fw-bold">Identitas Sekolah</h3></div>
                    <div class="card-body pt-3">
                        <div class="row g-4">
                            <div class="col-md-4">
                                <label class="form-label required">NPSN</label>
                                <input type="text" name="npsn" class="form-control" required maxlength="12"
                                       value="{{ old('npsn', $school->npsn) }}" placeholder="8 digit">
                            </div>
                            <div class="col-md-8">
                                <label class="form-label required">Nama sekolah</label>
                                <input type="text" name="name" class="form-control" required maxlength="200"
                                       value="{{ old('name', $school->name) }}">
                            </div>
                            <div class="col-md-4">
                                <label class="form-label required">Jenjang</label>
                                <select name="jenjang" class="form-select" required>
                                    @foreach (\App\Models\School::JENJANG as $j)
                                        <option value="{{ $j }}" {{ old('jenjang', $school->jenjang) === $j ? 'selected' : '' }}>{{ $j }}</option>
                                    @endforeach
                                </select>
                            </div>
                            <div class="col-md-4">
                                <label class="form-label required">Status</label>
                                <select name="status" class="form-select" required>
                                    @foreach (\App\Models\School::STATUS as $s)
                                        <option value="{{ $s }}" {{ old('status', $school->status) === $s ? 'selected' : '' }}>{{ ucfirst($s) }}</option>
                                    @endforeach
                                </select>
                            </div>
                            <div class="col-md-4">
                                <label class="form-label">Kabupaten / Kota</label>
                                <select name="region_id" class="form-select">
                                    <option value="">-</option>
                                    @foreach ($regions as $r)
                                        <option value="{{ $r->id }}" {{ old('region_id', $school->region_id) === $r->id ? 'selected' : '' }}>
                                            {{ $r->name }}
                                        </option>
                                    @endforeach
                                </select>
                            </div>
                            <div class="col-12">
                                <label class="form-label">Alamat</label>
                                <textarea name="address" class="form-control" rows="2" maxlength="500">{{ old('address', $school->address) }}</textarea>
                            </div>
                            <div class="col-md-4">
                                <label class="form-label">Kelurahan / Desa</label>
                                <input type="text" name="village" class="form-control" maxlength="120" value="{{ old('village', $school->village) }}">
                            </div>
                            <div class="col-md-4">
                                <label class="form-label">Kecamatan</label>
                                <input type="text" name="district" class="form-control" maxlength="120" value="{{ old('district', $school->district) }}">
                            </div>
                            <div class="col-md-4">
                                <label class="form-label">Kode pos</label>
                                <input type="text" name="postal_code" class="form-control" maxlength="10" value="{{ old('postal_code', $school->postal_code) }}">
                            </div>
                            <div class="col-md-4">
                                <label class="form-label">Telepon</label>
                                <input type="text" name="phone" class="form-control" maxlength="30" value="{{ old('phone', $school->phone) }}">
                            </div>
                            <div class="col-md-4">
                                <label class="form-label">Email</label>
                                <input type="email" name="email" class="form-control" maxlength="150" value="{{ old('email', $school->email) }}">
                            </div>
                            <div class="col-md-4">
                                <label class="form-label">Kepala sekolah</label>
                                <input type="text" name="principal_name" class="form-control" maxlength="150" value="{{ old('principal_name', $school->principal_name) }}">
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            <div class="col-xl-4">
                <div class="card card-flush border border-gray-200 mb-5">
                    <div class="card-header pt-5"><h3 class="card-title fw-bold">Lokasi &amp; Geofence</h3></div>
                    <div class="card-body pt-3">
                        <div class="row g-3">
                            <div class="col-6">
                                <label class="form-label">Latitude</label>
                                <input type="number" step="any" name="latitude" class="form-control form-control-sm"
                                       value="{{ old('latitude', $school->latitude) }}" placeholder="3.5952">
                            </div>
                            <div class="col-6">
                                <label class="form-label">Longitude</label>
                                <input type="number" step="any" name="longitude" class="form-control form-control-sm"
                                       value="{{ old('longitude', $school->longitude) }}" placeholder="98.6722">
                            </div>
                            <div class="col-12">
                                <label class="form-label">Radius geofence (meter)</label>
                                <input type="number" name="geofence_radius_m" class="form-control form-control-sm"
                                       min="20" max="5000" value="{{ old('geofence_radius_m', $school->geofence_radius_m ?? 250) }}">
                                <span class="form-text fs-9">
                                    Dipakai memverifikasi tablet mobile berada di area sekolah.
                                </span>
                            </div>
                        </div>
                    </div>
                </div>

                <div class="card card-flush border border-gray-200 mb-5">
                    <div class="card-header pt-5"><h3 class="card-title fw-bold">Pengenalan Wajah</h3></div>
                    <div class="card-body pt-3">
                        <label class="form-label">Ambang kemiripan</label>
                        <input type="number" step="0.01" min="0.3" max="0.99" name="face_match_threshold"
                               class="form-control form-control-sm"
                               value="{{ old('face_match_threshold', $school->face_match_threshold) }}"
                               placeholder="kosong = pakai default global (0.62)">
                        <div class="alert alert-light-warning mt-3 mb-0 py-3 px-4 fs-9">
                            Nilai terlalu <strong>rendah</strong> membuat orang lain bisa dikenali sebagai
                            siswa. Terlalu <strong>tinggi</strong> membuat siswa yang sah gagal absen.
                            Ubah hanya bila ada masalah nyata di lapangan, dan uji setelahnya.
                        </div>
                    </div>
                </div>

                @if ($school->exists)
                    <div class="card card-flush border border-gray-200 mb-5">
                        <div class="card-body p-5">
                            <label class="form-check form-check-custom">
                                <input type="checkbox" class="form-check-input" name="is_active" value="1"
                                       {{ old('is_active', $school->is_active) ? 'checked' : '' }}>
                                <span class="form-check-label fs-7">Sekolah aktif</span>
                            </label>
                            <span class="form-text fs-9 mt-2 d-block">
                                Menonaktifkan sekolah menghentikan absensi dari tablet-tabletnya.
                            </span>
                        </div>
                    </div>
                @endif

                <div class="d-flex gap-2">
                    <button class="btn btn-primary flex-grow-1">Simpan</button>
                    <a href="{{ route('schools.index') }}" class="btn btn-light">Batal</a>
                </div>
            </div>
        </div>
    </form>
@endsection
