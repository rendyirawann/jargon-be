<div class="d-flex justify-content-end gap-1">
    <a href="{{ route('students.show', $row->id) }}" class="btn btn-icon btn-sm btn-light" title="Detail">
        <i class="ki-outline ki-eye fs-5"></i>
    </a>

    @can('create_face_enrollment')
        <a href="{{ route('biometric.capture', $row->id) }}"
           class="btn btn-icon btn-sm btn-light-{{ $row->face_enrolled ? 'success' : 'danger' }}"
           title="{{ $row->face_enrolled ? 'Tambah sampel wajah' : 'Daftarkan wajah' }}">
            <i class="ki-outline ki-scan-barcode fs-5"></i>
        </a>
    @endcan

    @can('update_student')
        <a href="{{ route('students.edit', $row->id) }}" class="btn btn-icon btn-sm btn-light-warning" title="Ubah">
            <i class="ki-outline ki-pencil fs-5"></i>
        </a>
    @endcan

    @can('delete_student')
        <form method="POST" action="{{ route('students.destroy', $row->id) }}"
              onsubmit="return confirm('Hapus siswa {{ addslashes($row->full_name) }}?\n\nSeluruh data wajahnya akan dimusnahkan permanen. Riwayat absensi tetap tersimpan.');">
            @csrf
            @method('DELETE')
            <button class="btn btn-icon btn-sm btn-light-danger" title="Hapus">
                <i class="ki-outline ki-trash fs-5"></i>
            </button>
        </form>
    @endcan
</div>
