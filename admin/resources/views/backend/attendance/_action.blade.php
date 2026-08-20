@php
    $checkIn = $row->check_in_at
        ? \Illuminate\Support\Carbon::parse($row->check_in_at)->timezone(config('app.timezone'))->format('H:i')
        : '';
    $date = \Illuminate\Support\Carbon::parse($row->attendance_date)->toDateString();
@endphp

<div class="d-flex justify-content-end gap-1">
    @can('view_student')
        <a href="{{ route('students.show', $row->student_id) }}"
           class="btn btn-icon btn-sm btn-light" title="Detail siswa">
            <i class="ki-outline ki-eye fs-5"></i>
        </a>
    @endcan

    @can('override_attendance')
        <button type="button" class="btn btn-icon btn-sm btn-light-warning"
                data-bs-toggle="modal" data-bs-target="#modalManual"
                data-correct-attendance
                data-student-id="{{ $row->student_id }}"
                data-student-name="{{ $row->student_name }}"
                data-date="{{ $date }}"
                data-status="{{ $row->status }}"
                data-check-in="{{ $checkIn }}"
                title="Koreksi absensi">
            <i class="ki-outline ki-pencil fs-5"></i>
        </button>
    @endcan
</div>
