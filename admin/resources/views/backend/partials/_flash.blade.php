{{-- Pesan sukses / error yang seragam di seluruh halaman dashboard. --}}

@if (session('success'))
    <div class="alert alert-success d-flex align-items-center mt-5 mb-0">
        <i class="ki-duotone ki-check-circle fs-2 me-3"><span class="path1"></span><span class="path2"></span></i>
        <span>{{ session('success') }}</span>
    </div>
@endif

@if (session('error'))
    <div class="alert alert-danger d-flex align-items-center mt-5 mb-0">
        <i class="ki-duotone ki-cross-circle fs-2 me-3"><span class="path1"></span><span class="path2"></span></i>
        <span>{{ session('error') }}</span>
    </div>
@endif

@if ($errors->any())
    <div class="alert alert-danger mt-5 mb-0">
        <div class="d-flex align-items-center mb-2">
            <i class="ki-duotone ki-information-5 fs-2 me-3"><span class="path1"></span><span class="path2"></span><span class="path3"></span></i>
            <span class="fw-semibold">Periksa kembali data yang dikirim:</span>
        </div>
        <ul class="mb-0 ps-9">
            @foreach ($errors->all() as $error)
                <li class="fs-7">{{ $error }}</li>
            @endforeach
        </ul>
    </div>
@endif

{{-- Kode pairing perangkat hanya ditampilkan SEKALI setelah dibuat: ia tidak
     disimpan di dashboard dan tidak bisa dilihat lagi. --}}
@if (session('pairing'))
    @php $p = session('pairing'); @endphp
    <div class="alert alert-primary mt-5 mb-0">
        <div class="d-flex flex-wrap align-items-center gap-4">
            <i class="ki-duotone ki-devices fs-3x"><span class="path1"></span><span class="path2"></span><span class="path3"></span><span class="path4"></span><span class="path5"></span></i>
            <div class="flex-grow-1">
                <span class="fw-bold d-block">Kode pairing untuk {{ $p['device_code'] }}</span>
                <span class="fs-8 text-gray-700">
                    Masukkan kode ini di aplikasi tablet. Kode hanya berlaku sekali dan
                    kedaluwarsa
                    @if (! empty($p['expires_at']))
                        {{ \Illuminate\Support\Carbon::parse($p['expires_at'])->timezone(config('app.timezone'))->format('H:i') }}.
                    @else
                        dalam 30 menit.
                    @endif
                    Setelah halaman ini ditutup, kode tidak dapat dilihat lagi.
                </span>
            </div>
            <div class="fs-2hx fw-bold text-primary font-monospace ls-3">
                {{ $p['pairing_code'] ?? '-' }}
            </div>
        </div>
    </div>
@endif
