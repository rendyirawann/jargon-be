@extends('backend.layout.app')
@section('title', 'Data Wajah - '.$student->full_name)

@section('content')
    @include('backend.partials._flash')

    <div class="d-flex flex-wrap align-items-center justify-content-between gap-3 mt-5 mb-6">
        <div>
            <h2 class="fw-bold text-gray-900 mb-1">Data Wajah {{ $student->full_name }}</h2>
            <span class="text-muted fs-7">
                {{ $student->classroom?->name ?? 'Belum ditempatkan' }} &middot; {{ $student->school->name }}
            </span>
        </div>
        <a href="{{ route('students.show', $student) }}" class="btn btn-sm btn-light">Kembali</a>
    </div>

    <div class="card card-flush border border-gray-200">
        <div class="card-body p-5">
            @if ($samples->isEmpty())
                <div class="text-center text-muted py-15">
                    <i class="ki-duotone ki-scan-barcode fs-3x mb-3"><span class="path1"></span><span class="path2"></span><span class="path3"></span><span class="path4"></span><span class="path5"></span><span class="path6"></span><span class="path7"></span><span class="path8"></span></i>
                    <p class="fs-7 mb-0">Belum ada sampel wajah untuk siswa ini.</p>
                </div>
            @else
                <div class="row g-4">
                    @foreach ($samples as $s)
                        <div class="col-md-4 col-xl-3">
                            <div class="border border-gray-200 rounded p-3 h-100">
                                <img src="{{ $s->image_url }}" class="rounded w-100 mb-3" alt="Sampel wajah"
                                     style="aspect-ratio: 1; object-fit: cover;" loading="lazy">
                                <div class="d-flex align-items-center justify-content-between mb-2">
                                    <span class="fw-semibold fs-7">{{ $s->pose_label }}</span>
                                    <span class="badge {{ $s->quality_badge }} fs-9">
                                        Kualitas {{ $s->quality_score !== null ? number_format($s->quality_score, 2) : '?' }}
                                    </span>
                                </div>
                                <div class="fs-9 text-muted">
                                    {{ $s->created_at->timezone(config('app.timezone'))->format('d/m/Y H:i') }}<br>
                                    oleh {{ $s->capturedBy->name ?? ($s->device_id ? 'tablet' : 'sistem') }}
                                </div>

                                @if (! empty($s->quality_detail['issues']))
                                    <div class="mt-2 fs-9 text-warning">
                                        {{ implode('; ', $s->quality_detail['issues']) }}
                                    </div>
                                @endif

                                @can('delete_face_enrollment')
                                    <form method="POST" action="{{ route('biometric.destroy', $s) }}" class="mt-3"
                                          onsubmit="return confirm('Hapus sampel ini secara permanen?');">
                                        @csrf @method('DELETE')
                                        <button class="btn btn-sm btn-light-danger w-100 py-1 fs-9">Hapus sampel</button>
                                    </form>
                                @endcan
                            </div>
                        </div>
                    @endforeach
                </div>
            @endif
        </div>
    </div>
@endsection
