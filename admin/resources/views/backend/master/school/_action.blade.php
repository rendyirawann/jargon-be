<div class="d-flex justify-content-end gap-1">
    <a href="{{ route('schools.show', $row->id) }}" class="btn btn-icon btn-sm btn-light" title="Detail">
        <i class="ki-outline ki-eye fs-5"></i>
    </a>
    <a href="{{ route('dashboard', ['school_id' => $row->id]) }}" class="btn btn-icon btn-sm btn-light-info" title="Buka dashboard sekolah">
        <i class="ki-outline ki-chart-simple fs-5"></i>
    </a>
    @can('update_school')
        <a href="{{ route('schools.edit', $row->id) }}" class="btn btn-icon btn-sm btn-light-warning" title="Ubah">
            <i class="ki-outline ki-pencil fs-5"></i>
        </a>
    @endcan
    @can('delete_school')
        <form method="POST" action="{{ route('schools.destroy', $row->id) }}"
              onsubmit="return confirm('Arsipkan {{ addslashes($row->name) }}?\n\nPerangkat akan dinonaktifkan. Riwayat absensi tetap tersimpan dan laporan lama tetap bisa dibuka.');">
            @csrf @method('DELETE')
            <button class="btn btn-icon btn-sm btn-light-danger" title="Arsipkan">
                <i class="ki-outline ki-archive-tick fs-5"></i>
            </button>
        </form>
    @endcan
</div>
