//! Agregasi untuk dashboard & laporan.
//!
//! Semua query di sini SELALU membawa `attendance_date` (atau rentangnya)
//! sehingga PostgreSQL bisa melakukan partition pruning. Tanpa itu, satu
//! permintaan dashboard akan memindai seluruh riwayat provinsi.

use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::attendance::{
    AttendanceSummary, ClassroomSummary, ProvinceOverview, SchoolRate, StudentAttendanceRecap,
};
use crate::domain::face::FaceCoverage;
use crate::error::ApiResult;

/// Ringkasan satu hari. `school_id = None` berarti seluruh provinsi.
pub async fn daily_summary(
    pool: &PgPool,
    school_id: Option<Uuid>,
    date: NaiveDate,
) -> ApiResult<AttendanceSummary> {
    // Total siswa aktif dihitung terpisah dari absensi: siswa yang belum
    // discan sama sekali tidak punya baris di `attendances`, dan justru
    // itulah angka "belum absen" yang paling ingin dilihat kepala sekolah.
    let (total_students,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)::bigint FROM students s
        WHERE s.deleted_at IS NULL AND s.status = 'aktif'
          AND ($1::uuid IS NULL OR s.school_id = $1)
        "#,
    )
    .bind(school_id)
    .fetch_one(pool)
    .await?;

    let row: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE status = 'hadir')::bigint,
            COUNT(*) FILTER (WHERE status = 'terlambat')::bigint,
            COUNT(*) FILTER (WHERE status = 'izin')::bigint,
            COUNT(*) FILTER (WHERE status = 'sakit')::bigint,
            COUNT(*) FILTER (WHERE status = 'alfa')::bigint,
            COUNT(*) FILTER (WHERE status = 'dispensasi')::bigint
        FROM attendances
        WHERE attendance_date = $2
          AND ($1::uuid IS NULL OR school_id = $1)
        "#,
    )
    .bind(school_id)
    .bind(date)
    .fetch_one(pool)
    .await?;

    let (hadir, terlambat, izin, sakit, alfa, dispensasi) = row;
    let recorded = hadir + terlambat + izin + sakit + alfa + dispensasi;

    Ok(AttendanceSummary {
        summary_date: date,
        total_students,
        hadir,
        terlambat,
        izin,
        sakit,
        alfa,
        dispensasi,
        belum_absen: (total_students - recorded).max(0),
    })
}

/// Ringkasan per kelas untuk satu sekolah pada satu tanggal.
pub async fn classroom_summaries(
    pool: &PgPool,
    school_id: Uuid,
    date: NaiveDate,
) -> ApiResult<Vec<ClassroomSummary>> {
    let rows: Vec<ClassroomSummary> = sqlx::query_as(
        r#"
        WITH aktif AS (
            SELECT current_classroom_id AS classroom_id, COUNT(*)::bigint AS total
            FROM students
            WHERE school_id = $1 AND deleted_at IS NULL AND status = 'aktif'
            GROUP BY current_classroom_id
        ),
        absen AS (
            SELECT classroom_id,
                   COUNT(*) FILTER (WHERE status = 'hadir')::bigint      AS hadir,
                   COUNT(*) FILTER (WHERE status = 'terlambat')::bigint  AS terlambat,
                   COUNT(*) FILTER (WHERE status = 'izin')::bigint       AS izin,
                   COUNT(*) FILTER (WHERE status = 'sakit')::bigint      AS sakit,
                   COUNT(*) FILTER (WHERE status = 'alfa')::bigint       AS alfa,
                   COUNT(*)::bigint                                      AS tercatat
            FROM attendances
            WHERE school_id = $1 AND attendance_date = $2
            GROUP BY classroom_id
        )
        SELECT c.id            AS classroom_id,
               c.name          AS classroom_name,
               c.grade_level,
               u.name          AS homeroom_teacher_name,
               COALESCE(a.total, 0)      AS total_students,
               COALESCE(b.hadir, 0)      AS hadir,
               COALESCE(b.terlambat, 0)  AS terlambat,
               COALESCE(b.izin, 0)       AS izin,
               COALESCE(b.sakit, 0)      AS sakit,
               COALESCE(b.alfa, 0)       AS alfa,
               GREATEST(COALESCE(a.total, 0) - COALESCE(b.tercatat, 0), 0) AS belum_absen
        FROM classrooms c
        LEFT JOIN aktif a ON a.classroom_id = c.id
        LEFT JOIN absen b ON b.classroom_id = c.id
        LEFT JOIN users u ON u.id = c.homeroom_teacher_id
        WHERE c.school_id = $1 AND c.deleted_at IS NULL AND c.is_active
        ORDER BY c.grade_level, c.name
        "#,
    )
    .bind(school_id)
    .bind(date)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Rekap kehadiran per siswa untuk satu rentang tanggal.
pub async fn student_recap(
    pool: &PgPool,
    school_id: Uuid,
    classroom_id: Option<Uuid>,
    from: NaiveDate,
    to: NaiveDate,
) -> ApiResult<Vec<StudentAttendanceRecap>> {
    // Hari efektif = jumlah hari berbeda yang memang ada kegiatan absensi di
    // sekolah ini. Memakai jumlah hari kalender akan membuat persentase
    // kehadiran salah pada pekan yang ada libur.
    let rows: Vec<StudentAttendanceRecap> = sqlx::query_as(
        r#"
        WITH hari_efektif AS (
            SELECT COUNT(DISTINCT attendance_date)::bigint AS n
            FROM attendances
            WHERE school_id = $1 AND attendance_date BETWEEN $3 AND $4
        )
        SELECT s.id                  AS student_id,
               s.full_name           AS student_name,
               s.nis,
               c.name                AS classroom_name,
               COUNT(a.id) FILTER (WHERE a.status = 'hadir')::bigint     AS hadir,
               COUNT(a.id) FILTER (WHERE a.status = 'terlambat')::bigint AS terlambat,
               COUNT(a.id) FILTER (WHERE a.status = 'izin')::bigint      AS izin,
               COUNT(a.id) FILTER (WHERE a.status = 'sakit')::bigint     AS sakit,
               COUNT(a.id) FILTER (WHERE a.status = 'alfa')::bigint      AS alfa,
               COALESCE(SUM(a.late_minutes), 0)::bigint                  AS total_late_minutes,
               (SELECT n FROM hari_efektif)                              AS effective_days
        FROM students s
        LEFT JOIN classrooms c ON c.id = s.current_classroom_id
        LEFT JOIN attendances a
               ON a.student_id = s.id
              AND a.attendance_date BETWEEN $3 AND $4
        WHERE s.school_id = $1
          AND s.deleted_at IS NULL
          AND s.status = 'aktif'
          AND ($2::uuid IS NULL OR s.current_classroom_id = $2)
        GROUP BY s.id, s.full_name, s.nis, c.name
        ORDER BY c.name NULLS LAST, s.full_name
        "#,
    )
    .bind(school_id)
    .bind(classroom_id)
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Cakupan pendaftaran wajah.
pub async fn face_coverage(pool: &PgPool, school_id: Option<Uuid>) -> ApiResult<FaceCoverage> {
    let row: FaceCoverage = sqlx::query_as(
        r#"
        SELECT
            COUNT(*)::bigint                                                  AS total_students,
            COUNT(*) FILTER (WHERE face_enrolled)::bigint                     AS enrolled,
            COUNT(*) FILTER (WHERE NOT face_enrolled)::bigint                 AS not_enrolled,
            COUNT(*) FILTER (WHERE face_enrolled AND face_sample_count < 3)::bigint AS under_sampled,
            (SELECT COUNT(*)::bigint FROM face_enrollments fe
              WHERE fe.status = 'pending'
                AND ($1::uuid IS NULL OR fe.school_id = $1))                  AS pending_review
        FROM students s
        WHERE s.deleted_at IS NULL AND s.status = 'aktif'
          AND ($1::uuid IS NULL OR s.school_id = $1)
        "#,
    )
    .bind(school_id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Ikhtisar provinsi untuk dashboard Superadmin.
pub async fn province_overview(pool: &PgPool, date: NaiveDate) -> ApiResult<ProvinceOverview> {
    let attendance = daily_summary(pool, None, date).await?;
    let coverage = face_coverage(pool, None).await?;

    let (total_schools, active_schools): (i64, i64) = sqlx::query_as(
        r#"
        SELECT COUNT(*)::bigint,
               COUNT(*) FILTER (WHERE is_active)::bigint
        FROM schools WHERE deleted_at IS NULL
        "#,
    )
    .fetch_one(pool)
    .await?;

    let (reporting_schools,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(DISTINCT school_id)::bigint
        FROM attendances WHERE attendance_date = $1
        "#,
    )
    .bind(date)
    .fetch_one(pool)
    .await?;

    let (total_devices, online_devices): (i64, i64) = sqlx::query_as(
        r#"
        SELECT COUNT(*)::bigint,
               COUNT(*) FILTER (
                   WHERE last_seen_at > NOW() - INTERVAL '10 minutes'
               )::bigint
        FROM devices WHERE deleted_at IS NULL AND is_active
        "#,
    )
    .fetch_one(pool)
    .await?;

    let rates = school_rates(pool, date).await?;
    let mut sorted = rates;
    sorted.sort_by(|a, b| b.rate.partial_cmp(&a.rate).unwrap_or(std::cmp::Ordering::Equal));

    let top: Vec<SchoolRate> = sorted.iter().take(10).cloned().collect();
    let lowest: Vec<SchoolRate> = sorted.iter().rev().take(10).cloned().collect();

    Ok(ProvinceOverview {
        summary_date: date,
        total_schools,
        active_schools,
        reporting_schools,
        total_students: coverage.total_students,
        enrolled_students: coverage.enrolled,
        total_devices,
        online_devices,
        attendance,
        top_schools_by_rate: top,
        lowest_schools_by_rate: lowest,
    })
}

/// Tingkat kehadiran per sekolah pada satu tanggal.
///
/// Hanya sekolah yang benar-benar melaporkan absensi disertakan; sekolah yang
/// belum memasang tablet akan tampak sebagai 0% dan mengubur sekolah yang
/// betulan bermasalah.
pub async fn school_rates(pool: &PgPool, date: NaiveDate) -> ApiResult<Vec<SchoolRate>> {
    let rows: Vec<SchoolRate> = sqlx::query_as(
        r#"
        WITH aktif AS (
            SELECT school_id, COUNT(*)::bigint AS total
            FROM students
            WHERE deleted_at IS NULL AND status = 'aktif'
            GROUP BY school_id
        ),
        hadir AS (
            SELECT school_id,
                   COUNT(*) FILTER (
                       WHERE status IN ('hadir','terlambat','dispensasi')
                   )::bigint AS present,
                   COUNT(*)::bigint AS tercatat
            FROM attendances
            WHERE attendance_date = $1
            GROUP BY school_id
        )
        SELECT s.id       AS school_id,
               s.name     AS school_name,
               s.jenjang,
               COALESCE(a.total, 0)    AS total_students,
               COALESCE(h.present, 0)  AS present,
               CASE WHEN COALESCE(a.total, 0) = 0 THEN 0::double precision
                    ELSE (COALESCE(h.present, 0)::double precision / a.total * 100.0)
               END AS rate
        FROM schools s
        JOIN aktif a ON a.school_id = s.id
        JOIN hadir h ON h.school_id = s.id
        WHERE s.deleted_at IS NULL AND s.is_active
        "#,
    )
    .bind(date)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Isi ulang `attendance_daily_summary` untuk satu tanggal.
/// Dijalankan worker rollup setiap malam (dan bisa dipicu manual).
pub async fn rebuild_daily_rollup(pool: &PgPool, date: NaiveDate) -> ApiResult<u64> {
    let result = sqlx::query(
        r#"
        INSERT INTO attendance_daily_summary AS t (
            school_id, classroom_id, summary_date, total_students,
            hadir, terlambat, izin, sakit, alfa, dispensasi, avg_check_in, updated_at
        )
        SELECT a.school_id,
               COALESCE(a.classroom_id, '00000000-0000-0000-0000-000000000000'::uuid),
               a.attendance_date,
               COUNT(*)::int,
               COUNT(*) FILTER (WHERE a.status = 'hadir')::int,
               COUNT(*) FILTER (WHERE a.status = 'terlambat')::int,
               COUNT(*) FILTER (WHERE a.status = 'izin')::int,
               COUNT(*) FILTER (WHERE a.status = 'sakit')::int,
               COUNT(*) FILTER (WHERE a.status = 'alfa')::int,
               COUNT(*) FILTER (WHERE a.status = 'dispensasi')::int,
               -- Rata-rata jam masuk dalam waktu setempat (WIB).
               (AVG(EXTRACT(EPOCH FROM (a.check_in_at AT TIME ZONE 'Asia/Jakarta')::time))
                   * INTERVAL '1 second')::time,
               NOW()
        FROM attendances a
        WHERE a.attendance_date = $1
        GROUP BY a.school_id, a.classroom_id, a.attendance_date
        ON CONFLICT (school_id, summary_date, classroom_id) DO UPDATE SET
            total_students = EXCLUDED.total_students,
            hadir          = EXCLUDED.hadir,
            terlambat      = EXCLUDED.terlambat,
            izin           = EXCLUDED.izin,
            sakit          = EXCLUDED.sakit,
            alfa           = EXCLUDED.alfa,
            dispensasi     = EXCLUDED.dispensasi,
            avg_check_in   = EXCLUDED.avg_check_in,
            updated_at     = NOW()
        "#,
    )
    .bind(date)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// Tandai siswa yang tidak pernah discan sebagai `alfa`, lalu antrikan
/// notifikasi ke wali murid.
///
/// Dijalankan worker setelah `absent_notify_after` pada tiap sekolah. Tanpa
/// langkah ini, siswa yang tidak masuk sama sekali tidak akan punya baris
/// absensi — dan orang tuanya tidak pernah diberi tahu.
pub async fn mark_absentees(pool: &PgPool, school_id: Uuid, date: NaiveDate) -> ApiResult<u64> {
    let result = sqlx::query(
        r#"
        INSERT INTO attendances (
            attendance_date, school_id, student_id, classroom_id, academic_year_id,
            student_name, student_nis, classroom_name, school_name, status
        )
        SELECT $2, s.school_id, s.id, s.current_classroom_id, c.academic_year_id,
               s.full_name, s.nis, c.name, sc.name, 'alfa'
        FROM students s
        JOIN schools sc ON sc.id = s.school_id
        LEFT JOIN classrooms c ON c.id = s.current_classroom_id
        WHERE s.school_id = $1
          AND s.deleted_at IS NULL
          AND s.status = 'aktif'
          AND NOT EXISTS (
              SELECT 1 FROM attendances a
              WHERE a.attendance_date = $2 AND a.student_id = s.id
          )
        ON CONFLICT (attendance_date, student_id) DO NOTHING
        "#,
    )
    .bind(school_id)
    .bind(date)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}
