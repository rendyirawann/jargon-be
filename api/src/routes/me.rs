//! Endpoint aplikasi Jargon GO untuk data milik pengguna sendiri.
//!
//! Dipisahkan dari `/v1/attendances` yang berorientasi pengelolaan. Perbedaan
//! pentingnya bukan sekadar bentuk respons, melainkan **arah penjagaannya**:
//!
//! * `/v1/attendances` menerima `student_id` dari klien lalu memeriksanya.
//! * `/v1/me/*` tidak pernah mempercayai `student_id` dari klien tanpa
//!   memeriksanya terhadap daftar siswa yang tertaut pada akun. Seorang siswa
//!   tidak dapat melihat absensi temannya walau ia menebak UUID, dan orang tua
//!   hanya melihat anaknya sendiri.

use axum::extract::{Path, State};
use axum::routing::get;
use axum::Router;
use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::domain::attendance::AttendanceRecord;
use crate::error::{ApiError, ApiResult};
use crate::extract::ValidQuery;
use crate::state::AppState;
use crate::util::{self, ApiResponse, DateRangeQuery};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/me/home", get(home))
        .route("/me/attendance", get(attendance))
        .route("/me/attendance/{student_id}/recap", get(recap))
}

/// Ringkasan beranda aplikasi.
#[derive(Debug, Serialize, ToSchema)]
pub struct HomeSummary {
    pub greeting: String,
    pub role_label: String,
    /// Kartu absensi per siswa yang tertaut ke akun.
    pub students: Vec<StudentTodayCard>,
    /// Ringkasan sekolah — hanya untuk guru/staff/kepala sekolah.
    pub school: Option<SchoolTodayCard>,
    /// Jumlah pengaduan milik sendiri yang statusnya berubah dan belum dibuka.
    pub panic_updates: i64,
    /// Pengajuan berkas yang butuh tindakan pengguna.
    pub document_actions: i64,
    /// Menu yang boleh ditampilkan, sesuai izin akun.
    pub available_menus: Vec<String>,
}

/// Peran yang boleh membuka menu pengenalan wajah di aplikasi.
///
/// Sengaja daftar PERAN, bukan izin: izin pendaftaran wajah dimiliki juga
/// guru dan staff TU untuk keperluan dashboard, sedangkan pintu absensi di
/// ponsel harus lebih sempit daripada itu.
const ADMIN_FACE_ROLES: [&str; 2] = ["superadmin", "admin_dinas"];

#[derive(Debug, Serialize, ToSchema)]
pub struct StudentTodayCard {
    pub student_id: Uuid,
    pub full_name: String,
    pub classroom_name: Option<String>,
    pub school_name: String,
    pub relation: String,
    /// Status hari ini: `hadir`, `terlambat`, `alfa`, ..., atau `belum_absen`.
    pub today_status: String,
    pub check_in_time: Option<String>,
    pub check_out_time: Option<String>,
    pub late_minutes: i32,
    /// Rekap bulan berjalan.
    pub month_present: i64,
    pub month_late: i64,
    pub month_absent: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SchoolTodayCard {
    pub school_id: Uuid,
    pub school_name: String,
    pub total_students: i64,
    pub hadir: i64,
    pub terlambat: i64,
    pub belum_absen: i64,
    pub rate: f64,
}

/// Beranda aplikasi: satu panggilan untuk seluruh kartu di layar utama.
#[utoipa::path(
    get, path = "/v1/me/home", tag = "Jargon GO",
    security(("bearer" = [])),
    responses((status = 200, description = "Ringkasan beranda", body = HomeSummary))
)]
pub async fn home(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<ApiResponse<HomeSummary>> {
    user.require("use_mobile_app")?;

    let today = util::today_wib();
    let students = student_cards(&state, &user, today).await?;

    // Kartu sekolah hanya untuk yang memang mengelola sekolah. Siswa dan
    // orang tua tidak berkepentingan dengan angka kehadiran seluruh sekolah,
    // dan menampilkannya justru membuka data yang tidak perlu.
    let school = if user.has_permission("view_attendance") && !user.is_student_scoped() {
        school_card(&state, &user, today).await?
    } else {
        None
    };

    let (panic_updates,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)::bigint FROM panic_reports
        WHERE author_user_id = $1
          AND created_at > NOW() - INTERVAL '180 days'
          AND status <> 'baru'
          AND updated_at > NOW() - INTERVAL '7 days'
        "#,
    )
    .bind(user.id)
    .fetch_one(&state.db)
    .await?;

    // "Butuh tindakan" = milik sendiri dan menunggu perbaikan, ATAU (bagi
    // verifikator) menunggu diperiksa.
    let (document_actions,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)::bigint FROM document_submissions s
        WHERE (s.user_id = $1 AND s.status IN ('draft', 'revisi'))
           OR ($2 AND s.status = 'diajukan'
                 AND ($3::uuid IS NULL OR s.school_id = $3))
        "#,
    )
    .bind(user.id)
    .bind(user.has_permission("verify_document_submission"))
    .bind(user.school_id)
    .fetch_one(&state.db)
    .await?;

    let mut available_menus = Vec::new();
    if user.has_permission("view_own_attendance")
        || user.has_permission("view_children_attendance")
        || user.has_permission("view_attendance")
    {
        available_menus.push("absensi".to_string());
    }
    if user.has_permission("view_panic_feed") {
        available_menus.push("panic_button".to_string());
    }
    if user.has_permission("create_document_submission")
        || user.has_permission("verify_document_submission")
    {
        available_menus.push("pemberkasan".to_string());
    }

    // Menu pengenalan wajah HANYA untuk admin/superadmin.
    //
    // Dibatasi PERAN, bukan izin, dan itu disengaja. Izin
    // `create_face_enrollment` dimiliki juga guru dan staff TU supaya
    // mereka bisa mendaftarkan wajah dari dashboard — tetapi menu ini di
    // aplikasi membuka alat yang MENCATAT KEHADIRAN, dan kehadiran tidak
    // boleh bisa dicatat dari ponsel pribadi siapa pun yang kebetulan
    // memegang izin itu.
    //
    // Guru tetap dapat mendaftarkan wajah lewat dashboard /admin; yang
    // tidak diberikan adalah pintu absensi di ponsel.
    if user.roles.iter().any(|r| ADMIN_FACE_ROLES.contains(&r.as_str())) {
        available_menus.push("face_recognition".to_string());
    }

    Ok(ApiResponse::new(HomeSummary {
        greeting: greeting_for(&user.name),
        role_label: user.role_label().to_string(),
        students,
        school,
        panic_updates,
        document_actions,
        available_menus,
    }))
}

/// Riwayat absensi siswa yang tertaut ke akun.
#[utoipa::path(
    get, path = "/v1/me/attendance", tag = "Jargon GO",
    params(
        DateRangeQuery,
        ("student_id" = Option<Uuid>, Query,
         description = "Batasi ke satu anak (untuk orang tua dengan beberapa anak)")
    ),
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Riwayat absensi", body = [AttendanceRecord]),
        (status = 403, description = "Siswa di luar cakupan akun Anda")
    )
)]
pub async fn attendance(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(range): ValidQuery<DateRangeQuery>,
    ValidQuery(q): ValidQuery<StudentQuery>,
) -> ApiResult<ApiResponse<Vec<AttendanceRecord>>> {
    user.require_any(&[
        "view_own_attendance",
        "view_children_attendance",
        "view_attendance",
    ])?;

    // Inti penjagaan: `student_id` dari klien selalu dipersempit ke daftar
    // siswa yang tertaut pada akun. Untuk guru/staff nilainya `None`, yang
    // berarti tidak dibatasi — tetapi mereka tetap dibatasi oleh sekolah.
    let students = user.resolve_students(q.student_id)?;
    let school = if students.is_none() {
        user.resolve_school(None)?
    } else {
        None
    };

    let (from, to) = range.resolve()?;

    let rows: Vec<AttendanceRecord> = sqlx::query_as(
        r#"
        SELECT id, attendance_date, school_id, school_name, student_id, student_name,
               student_nis, classroom_id, classroom_name, check_in_at, check_out_at,
               status, late_minutes, duration_minutes, check_in_method, check_out_method,
               notes, notification_status
        FROM attendances
        WHERE attendance_date BETWEEN $1 AND $2
          AND ($3::uuid[] IS NULL OR student_id = ANY($3))
          AND ($4::uuid IS NULL OR school_id = $4)
        ORDER BY attendance_date DESC, student_name
        LIMIT 400
        "#,
    )
    .bind(from)
    .bind(to)
    .bind(students.as_deref())
    .bind(school)
    .fetch_all(&state.db)
    .await?;

    Ok(ApiResponse::new(rows))
}

#[derive(Debug, serde::Deserialize)]
pub struct StudentQuery {
    pub student_id: Option<Uuid>,
}

/// Rekap kehadiran satu siswa pada rentang tanggal.
#[utoipa::path(
    get, path = "/v1/me/attendance/{student_id}/recap", tag = "Jargon GO",
    params(("student_id" = Uuid, Path, description = "ID siswa"), DateRangeQuery),
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Rekap kehadiran"),
        (status = 403, description = "Siswa di luar cakupan akun Anda")
    )
)]
pub async fn recap(
    State(state): State<AppState>,
    user: AuthUser,
    Path(student_id): Path<Uuid>,
    ValidQuery(range): ValidQuery<DateRangeQuery>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require_any(&[
        "view_own_attendance",
        "view_children_attendance",
        "view_attendance",
    ])?;
    user.require_student(student_id)?;

    // Guru boleh melihat siswa mana pun, tetapi tetap hanya di sekolahnya.
    if !user.is_student_scoped() {
        let row: Option<(Uuid,)> =
            sqlx::query_as("SELECT school_id FROM students WHERE id = $1 AND deleted_at IS NULL")
                .bind(student_id)
                .fetch_optional(&state.db)
                .await?;
        let (school_id,) = row.ok_or_else(|| ApiError::NotFound("siswa".into()))?;
        user.resolve_school(Some(school_id))?;
    }

    let (from, to) = range.resolve()?;

    let row: (i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FILTER (WHERE status = 'hadir')::bigint,
               COUNT(*) FILTER (WHERE status = 'terlambat')::bigint,
               COUNT(*) FILTER (WHERE status = 'izin')::bigint,
               COUNT(*) FILTER (WHERE status = 'sakit')::bigint,
               COUNT(*) FILTER (WHERE status = 'alfa')::bigint,
               COALESCE(SUM(late_minutes), 0)::bigint,
               COUNT(DISTINCT attendance_date)::bigint
        FROM attendances
        WHERE student_id = $1 AND attendance_date BETWEEN $2 AND $3
        "#,
    )
    .bind(student_id)
    .bind(from)
    .bind(to)
    .fetch_one(&state.db)
    .await?;

    let hadir = row.0 + row.1;
    let hari = row.6.max(1);

    Ok(ApiResponse::new(serde_json::json!({
        "student_id": student_id,
        "from": from,
        "to": to,
        "hadir": row.0,
        "terlambat": row.1,
        "izin": row.2,
        "sakit": row.3,
        "alfa": row.4,
        "total_menit_terlambat": row.5,
        "hari_tercatat": row.6,
        "persentase_kehadiran": (hadir as f64 / hari as f64 * 100.0 * 10.0).round() / 10.0,
    })))
}

// =====================================================================
// Helper
// =====================================================================

async fn student_cards(
    state: &AppState,
    user: &AuthUser,
    today: NaiveDate,
) -> ApiResult<Vec<StudentTodayCard>> {
    // Akun tanpa tautan siswa (guru, staff, dinas) tidak menampilkan kartu ini.
    let Some(student_ids) = user.accessible_students() else {
        return Ok(Vec::new());
    };
    if student_ids.is_empty() {
        return Ok(Vec::new());
    }

    let month_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);

    #[derive(sqlx::FromRow)]
    struct Row {
        student_id: Uuid,
        full_name: String,
        classroom_name: Option<String>,
        school_name: String,
        relation: Option<String>,
        today_status: Option<String>,
        check_in_at: Option<chrono::DateTime<chrono::Utc>>,
        check_out_at: Option<chrono::DateTime<chrono::Utc>>,
        late_minutes: Option<i32>,
        month_present: i64,
        month_late: i64,
        month_absent: i64,
    }

    let rows: Vec<Row> = sqlx::query_as(
        r#"
        SELECT st.id AS student_id, st.full_name, c.name AS classroom_name,
               sc.name AS school_name,
               g.relation,
               a.status AS today_status, a.check_in_at, a.check_out_at, a.late_minutes,
               COALESCE(m.hadir, 0)     AS month_present,
               COALESCE(m.terlambat, 0) AS month_late,
               COALESCE(m.alfa, 0)      AS month_absent
        FROM students st
        JOIN schools sc ON sc.id = st.school_id
        LEFT JOIN classrooms c ON c.id = st.current_classroom_id
        LEFT JOIN student_guardians g
               ON g.student_id = st.id AND g.user_id = $2
        LEFT JOIN attendances a
               ON a.student_id = st.id AND a.attendance_date = $3
        LEFT JOIN LATERAL (
            SELECT COUNT(*) FILTER (WHERE status = 'hadir')::bigint     AS hadir,
                   COUNT(*) FILTER (WHERE status = 'terlambat')::bigint AS terlambat,
                   COUNT(*) FILTER (WHERE status = 'alfa')::bigint      AS alfa
            FROM attendances mm
            WHERE mm.student_id = st.id
              AND mm.attendance_date BETWEEN $4 AND $3
        ) m ON TRUE
        WHERE st.id = ANY($1) AND st.deleted_at IS NULL
        ORDER BY st.full_name
        "#,
    )
    .bind(student_ids)
    .bind(user.id)
    .bind(today)
    .bind(month_start)
    .fetch_all(&state.db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| StudentTodayCard {
            student_id: r.student_id,
            full_name: r.full_name,
            classroom_name: r.classroom_name,
            school_name: r.school_name,
            relation: r.relation.unwrap_or_else(|| "diri_sendiri".into()),
            // Tidak ada baris absensi berarti belum discan sama sekali —
            // itu berbeda dari `alfa` yang sudah ditetapkan sistem.
            today_status: r.today_status.unwrap_or_else(|| "belum_absen".into()),
            check_in_time: util::format_time_wib(r.check_in_at),
            check_out_time: util::format_time_wib(r.check_out_at),
            late_minutes: r.late_minutes.unwrap_or(0),
            month_present: r.month_present,
            month_late: r.month_late,
            month_absent: r.month_absent,
        })
        .collect())
}

async fn school_card(
    state: &AppState,
    user: &AuthUser,
    today: NaiveDate,
) -> ApiResult<Option<SchoolTodayCard>> {
    let Some(school_id) = user.school_id else {
        return Ok(None);
    };

    let row: Option<(String, i64, i64, i64, i64)> = sqlx::query_as(
        r#"
        SELECT s.name,
               (SELECT COUNT(*)::bigint FROM students st
                 WHERE st.school_id = s.id AND st.deleted_at IS NULL
                   AND st.status = 'aktif') AS total_students,
               COALESCE(a.hadir, 0)     AS hadir,
               COALESCE(a.terlambat, 0) AS terlambat,
               COALESCE(a.tercatat, 0)  AS tercatat
        FROM schools s
        LEFT JOIN LATERAL (
            SELECT COUNT(*) FILTER (WHERE status = 'hadir')::bigint     AS hadir,
                   COUNT(*) FILTER (WHERE status = 'terlambat')::bigint AS terlambat,
                   COUNT(*)::bigint                                     AS tercatat
            FROM attendances
            WHERE school_id = s.id AND attendance_date = $2
        ) a ON TRUE
        WHERE s.id = $1
        "#,
    )
    .bind(school_id)
    .bind(today)
    .fetch_optional(&state.db)
    .await?;

    Ok(row.map(|(name, total, hadir, terlambat, tercatat)| {
        let present = hadir + terlambat;
        SchoolTodayCard {
            school_id,
            school_name: name,
            total_students: total,
            hadir,
            terlambat,
            belum_absen: (total - tercatat).max(0),
            rate: if total == 0 {
                0.0
            } else {
                (present as f64 / total as f64 * 1000.0).round() / 10.0
            },
        }
    }))
}

/// Sapaan mengikuti waktu setempat.
fn greeting_for(name: &str) -> String {
    let hour = util::now_wib().hour_wib();
    let sapaan = match hour {
        0..=10 => "Selamat pagi",
        11..=14 => "Selamat siang",
        15..=18 => "Selamat sore",
        _ => "Selamat malam",
    };
    let panggilan = name.split_whitespace().next().unwrap_or(name);
    format!("{sapaan}, {panggilan}")
}

trait HourWib {
    fn hour_wib(&self) -> u32;
}

impl HourWib for chrono::DateTime<chrono::FixedOffset> {
    fn hour_wib(&self) -> u32 {
        use chrono::Timelike;
        self.hour()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sapaan_memakai_nama_panggilan() {
        let s = greeting_for("Budi Santoso Wijaya");
        assert!(s.ends_with(", Budi"), "sapaan = {s}");
        assert!(s.starts_with("Selamat "));
    }

    #[test]
    fn sapaan_untuk_nama_satu_kata() {
        assert!(greeting_for("Aisyah").ends_with(", Aisyah"));
    }
}
