@extends('backend.layout.app')
@section('title', 'Jenis Dokumen')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Jenis Dokumen</h2>
            <span class="text-muted fs-7">
                Daftar ini yang menjadi daftar periksa di aplikasi &mdash; guru hanya melihat
                jenis dokumen yang terdaftar di sini untuk keperluan yang dipilihnya.
            </span>
        </div>
        <a href="{{ route('documents.index') }}" class="btn btn-sm btn-light">Kembali</a>
    </div>

    <div class="row g-5">
        <div class="col-xl-8">
            @foreach ($purposes as $key => $label)
                <div class="card card-flush border border-gray-200 mb-5">
                    <div class="card-header pt-5">
                        <h3 class="card-title fw-bold">{{ $label }}</h3>
                        <div class="card-toolbar">
                            <span class="badge badge-light">{{ ($types[$key] ?? collect())->count() }} dokumen</span>
                        </div>
                    </div>
                    <div class="card-body pt-3 p-0">
                        <div class="table-responsive">
                            <table class="table table-row-bordered align-middle mb-0">
                                <thead class="bg-light">
                                    <tr class="fw-bold fs-8 text-uppercase text-muted">
                                        <th class="ps-5">Nama</th>
                                        <th>Kode</th>
                                        <th class="text-center">Wajib</th>
                                        <th class="text-center">Maks</th>
                                        <th class="pe-5 text-center">Aktif</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    @forelse (($types[$key] ?? collect()) as $t)
                                        <tr>
                                            <td class="ps-5" style="max-width: 320px;">
                                                <span class="fw-semibold fs-7 d-block">{{ $t->name }}</span>
                                                @if ($t->description)
                                                    <span class="text-muted fs-9">{{ $t->description }}</span>
                                                @endif
                                            </td>
                                            <td><code class="fs-8">{{ $t->code }}</code></td>
                                            <td class="text-center">
                                                @if ($t->is_required)
                                                    <span class="badge badge-light-danger fs-9">wajib</span>
                                                @else
                                                    <span class="text-muted fs-9">opsional</span>
                                                @endif
                                            </td>
                                            <td class="text-center fs-8 text-muted">
                                                {{ round($t->max_bytes / 1048576, 1) }} MB
                                            </td>
                                            <td class="pe-5 text-center">
                                                @if ($t->is_active)
                                                    <span class="badge badge-light-success fs-9">aktif</span>
                                                @else
                                                    <span class="badge badge-light fs-9">nonaktif</span>
                                                @endif
                                            </td>
                                        </tr>
                                    @empty
                                        <tr>
                                            <td colspan="5" class="text-center text-muted py-8 fs-8">
                                                Belum ada jenis dokumen untuk keperluan ini.
                                            </td>
                                        </tr>
                                    @endforelse
                                </tbody>
                            </table>
                        </div>
                    </div>
                </div>
            @endforeach
        </div>

        <div class="col-xl-4">
            <div class="card card-flush border border-gray-200">
                <div class="card-header pt-5"><h3 class="card-title fw-bold">Tambah Jenis Dokumen</h3></div>
                <form method="POST" action="{{ route('documents.types.store') }}" class="card-body pt-3">
                    @csrf
                    <div class="mb-3">
                        <label class="form-label required">Keperluan</label>
                        <select name="purpose" class="form-select form-select-sm" required>
                            @foreach ($purposes as $key => $label)
                                <option value="{{ $key }}" {{ old('purpose') === $key ? 'selected' : '' }}>
                                    {{ $label }}
                                </option>
                            @endforeach
                        </select>
                    </div>
                    <div class="mb-3">
                        <label class="form-label required">Kode</label>
                        <input type="text" name="code" class="form-control form-control-sm"
                               value="{{ old('code') }}" required maxlength="40"
                               placeholder="mis. sk_pangkat_terakhir">
                        <span class="form-text fs-9">
                            Kode dipakai aplikasi untuk mencocokkan berkas; jangan diubah setelah dipakai.
                        </span>
                    </div>
                    <div class="mb-3">
                        <label class="form-label required">Nama</label>
                        <input type="text" name="name" class="form-control form-control-sm"
                               value="{{ old('name') }}" required maxlength="120">
                    </div>
                    <div class="mb-3">
                        <label class="form-label">Keterangan</label>
                        <input type="text" name="description" class="form-control form-control-sm"
                               value="{{ old('description') }}" maxlength="300"
                               placeholder="Petunjuk singkat bagi pengusul">
                    </div>
                    <div class="row g-3 mb-3">
                        <div class="col-6">
                            <label class="form-label">Ukuran maks (MB)</label>
                            <input type="number" name="max_mb" class="form-control form-control-sm"
                                   value="{{ old('max_mb', 5) }}" min="1" max="25">
                        </div>
                        <div class="col-6">
                            <label class="form-label">Urutan</label>
                            <input type="number" name="sort_order" class="form-control form-control-sm"
                                   value="{{ old('sort_order', 0) }}" min="0" max="999">
                        </div>
                    </div>
                    <label class="form-check form-check-sm form-check-custom mb-4">
                        <input type="checkbox" class="form-check-input" name="is_required" value="1"
                               {{ old('is_required') ? 'checked' : '' }}>
                        <span class="form-check-label fs-8">Wajib diunggah</span>
                    </label>
                    <button class="btn btn-sm btn-primary w-100">Tambah</button>
                    <span class="form-text fs-9 mt-3 d-block">
                        Format yang diterima: PDF, JPG, dan PNG.
                    </span>
                </form>
            </div>
        </div>
    </div>
@endsection
