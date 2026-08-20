@extends('backend.layout.app')
@section('title', 'Template Pesan')

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Template Pesan</h2>
            <span class="text-muted fs-7">
                Template bertanda <em>bawaan</em> berlaku untuk semua sekolah sampai sekolah
                membuat versinya sendiri.
            </span>
        </div>
        @include('backend.partials._school_picker', ['schools' => $schools, 'schoolId' => $schoolId, 'allowAll' => false])
    </div>

    <div class="row g-5">
        <div class="col-xl-7">
            <div class="card card-flush border border-gray-200">
                <div class="card-body p-0">
                    <div class="table-responsive">
                        <table class="table table-row-bordered align-middle mb-0">
                            <thead class="bg-light">
                                <tr class="fw-bold fs-8 text-uppercase text-muted">
                                    <th class="ps-5">Jenis</th>
                                    <th>Kanal</th>
                                    <th>Isi</th>
                                    <th class="pe-5">Asal</th>
                                </tr>
                            </thead>
                            <tbody>
                                @forelse ($templates as $t)
                                    <tr class="{{ $t->is_active ? '' : 'opacity-50' }}">
                                        <td class="ps-5 fs-7 fw-semibold">{{ $t->key_label }}</td>
                                        <td class="fs-8">{{ $t->channel_label }}</td>
                                        <td class="fs-8 text-muted" style="max-width: 320px;">
                                            {{ \Illuminate\Support\Str::limit(strip_tags($t->body), 110) }}
                                        </td>
                                        <td class="pe-5">
                                            <span class="badge badge-light-{{ $t->is_default ? 'secondary' : 'primary' }} fs-9">
                                                {{ $t->is_default ? 'bawaan' : 'sekolah' }}
                                            </span>
                                        </td>
                                    </tr>
                                @empty
                                    <tr><td colspan="4" class="text-center text-muted py-10 fs-7">Belum ada template.</td></tr>
                                @endforelse
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>
        </div>

        <div class="col-xl-5">
            @can('manage_notification_template')
                <div class="card card-flush border border-gray-200">
                    <div class="card-header pt-5"><h3 class="card-title fw-bold">Buat / Ubah Template Sekolah</h3></div>
                    <form method="POST" action="{{ route('notifications.templates.store') }}" class="card-body pt-3">
                        @csrf
                        <input type="hidden" name="school_id" value="{{ $schoolId }}">

                        <div class="row g-3 mb-4">
                            <div class="col-7">
                                <label class="form-label required">Jenis pesan</label>
                                <select name="key" class="form-select form-select-sm" required>
                                    @foreach ($keys as $key => $label)
                                        <option value="{{ $key }}" {{ old('key') === $key ? 'selected' : '' }}>{{ $label }}</option>
                                    @endforeach
                                </select>
                            </div>
                            <div class="col-5">
                                <label class="form-label required">Kanal</label>
                                <select name="channel" class="form-select form-select-sm" required>
                                    @foreach ($channels as $c)
                                        <option value="{{ $c }}" {{ old('channel') === $c ? 'selected' : '' }}>{{ ucfirst($c) }}</option>
                                    @endforeach
                                </select>
                            </div>
                        </div>

                        <div class="mb-4">
                            <label class="form-label">Subjek (email)</label>
                            <input type="text" name="subject" class="form-control form-control-sm" maxlength="200"
                                   value="{{ old('subject') }}">
                        </div>

                        <div class="mb-3">
                            <label class="form-label required">Isi pesan</label>
                            <textarea name="body" class="form-control" rows="8" required minlength="10" maxlength="4000">{{ old('body') }}</textarea>
                        </div>

                        <div class="alert alert-light-primary py-3 px-4 fs-9 mb-4">
                            <span class="fw-semibold d-block mb-2">Placeholder yang tersedia</span>
                            <div class="d-flex flex-wrap gap-1">
                                @foreach ($variables as $v)
                                    {{-- Kurung kurawal dirakit dari chr(): menuliskannya literal di
                                         dalam view, bahkan di dalam string PHP, membuat compiler
                                         Blade menutup echo lebih awal. --}}
                                    @php
                                        $brace = str_repeat(chr(123), 2).$v.str_repeat(chr(125), 2);
                                    @endphp
                                    <code class="cursor-pointer" onclick="insertVar('{{ $v }}')">{{ $brace }}</code>
                                @endforeach
                            </div>
                            <span class="d-block mt-2">
                                Placeholder yang salah tulis akan ditolak saat menyimpan — bukan
                                setelah pesan salah terkirim ke ribuan orang tua.
                            </span>
                        </div>

                        <label class="form-check form-check-custom mb-4">
                            <input type="checkbox" class="form-check-input" name="is_active" value="1" checked>
                            <span class="form-check-label fs-7">Aktif</span>
                        </label>

                        <button class="btn btn-primary w-100" {{ $schoolId ? '' : 'disabled' }}>Simpan template</button>
                    </form>
                </div>
            @endcan
        </div>
    </div>
@endsection

@push('scripts')
    <script>
        // Klik placeholder menyisipkannya di posisi kursor — mengurangi salah
        // tulis, yang merupakan penyebab paling umum pesan gagal render.
        function insertVar(name) {
            const ta = document.querySelector('textarea[name="body"]');
            if (!ta) return;
            // Kurawal disusun dari potongan agar tidak terbaca compiler Blade.
            const token = '{' + '{' + name + '}' + '}';
            const start = ta.selectionStart ?? ta.value.length;
            ta.value = ta.value.slice(0, start) + token + ta.value.slice(ta.selectionEnd ?? start);
            ta.focus();
            ta.selectionStart = ta.selectionEnd = start + token.length;
        }
    </script>
@endpush
