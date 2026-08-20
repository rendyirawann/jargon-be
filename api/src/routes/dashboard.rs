//! Endpoint dashboard: satu panggilan menghasilkan seluruh angka untuk
//! layar utama, sehingga dashboard Laravel tidak perlu 8 request terpisah.

use axum::extract::State;
use axum::routing::get;
use axum::Router;
use chrono::NaiveDate;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::domain::attendance::{
    AttendanceRecord, AttendanceSummary, ClassroomSummary, ProvinceOverview,
};
use crate::domain::face::FaceCoverage;
use crate::error::ApiResult;
use crate::extract::ValidQuery;
use crate::services::reports;
use crate::state::AppState;
use crate::util::{self, ApiResponse};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/dashboard", get(dashboard))
        .route("/dashboard/province", get(province))
        .route("/dashboard/live", get(live_feed))
}

#[derive(Debug, serde::Deserialize)]
pub struct DashboardQuery {
    pub school_id: Option<Uuid>,
    pub date: Option<NaiveDate>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SchoolDashboard {
    pub summary_date: NaiveDate,
    pub school_id: Option<Uuid>,
    pub school_name: Option<String>,
    pub attendance: AttendanceSummary,
    pub face_coverage: FaceCoverage,
    /// Persentase siswa yang sudah punya data wajah.
    pub face_coverage_percent: f64,
    pub attendance_rate: f64,
    pub classrooms: Vec<ClassroomSummary>,
    pub devices: DeviceHealth,
    /// 7 hari terakhir untuk grafik tren.
    pub trend: Vec<TrendPoint>,
    pub notifications: NotificationBrief,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DeviceHealth {
    pub total: i64,
    pub online: i64,
    pub offline: i64,
    pub never_paired: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct TrendPoint {
    pub date: NaiveDate,
    pub hadir: i64,
    pub terlambat: i64,
    pub alfa: i64,
    pub izin_sakit: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NotificationBrief {
    pub queued: i64,
    pub sent_today: i64,
    pub failed_today: i64,
}

/// Dashboard sekolah (guru, staff, kepala sekolah).
#[utoipa::path(
    get, path = "/v1/dashboard", tag = "Dashboard",
    params(
        ("school_id" = Option<Uuid>, Query, description = "ID sekolah (otomatis untuk pengguna sekolah)"),
        ("date" = Option<NaiveDate>, Query, description = "Default hari ini")
    ),
    security(("bearer" = [])),
    responses((status = 200, description = "Data dashboard", body = SchoolDashboard))
)]
pub async fn dashboard(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(q): ValidQuery<DashboardQuery>,
) -> ApiResult<ApiResponse<SchoolDashboard>> {
    user.require("view_dashboard")?;
    let school = user.resolve_school(q.school_id)?;
    let date = q.date.unwrap_or_else(util::today_wib);

    let attendance = reports::daily_summary(&state.db, school, date).await?;
    let face_coverage = reports::face_coverage(&state.db, school).await?;

    let classrooms = match school {
        Some(school_id) => reports::classroom_summaries(&state.db, school_id, date).await?,
        // Cakupan provinsi tidak menampilkan daftar kelas — 700rb siswa
        // tersebar di puluhan ribu rombel, tidak ada gunanya di satu layar.
        None => Vec::new(),
    };

    let school_name = match school {
        Some(id) => {
            let row: Option<(String,)> = sqlx::query_as("SELECT name FROM schools WHERE id = $1")
                .bind(id)
                .fetch_optional(&state.db)
                .await?;
            row.map(|r| r.0)
        }
        None => None,
    };

    let (total, online, never_paired): (i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT COUNT(*)::bigint,
               COUNT(*) FILTER (WHERE last_seen_at > NOW() - INTERVAL '10 minutes')::bigint,
               COUNT(*) FILTER (WHERE token_hash IS NULL)::bigint
        FROM devices
        WHERE deleted_at IS NULL AND is_active
          AND ($1::uuid IS NULL OR school_id = $1)
        "#,
    )
    .bind(school)
    .fetch_one(&state.db)
    .await?;

    let trend: Vec<TrendPoint> = sqlx::query_as(
        r#"
        SELECT attendance_date AS date,
               COUNT(*) FILTER (WHERE status = 'hadir')::bigint     AS hadir,
               COUNT(*) FILTER (WHERE status = 'terlambat')::bigint  AS terlambat,
               COUNT(*) FILTER (WHERE status = 'alfa')::bigint       AS alfa,
               COUNT(*) FILTER (WHERE status IN ('izin','sakit'))::bigint AS izin_sakit
        FROM attendances
        -- `date - integer` menghasilkan date (bukan timestamp), sehingga
        -- partition pruning pada `attendances` tetap bekerja.
        WHERE attendance_date BETWEEN ($2::date - 6) AND $2::date
          AND ($1::uuid IS NULL OR school_id = $1)
        GROUP BY attendance_date
        ORDER BY attendance_date
        "#,
    )
    .bind(school)
    .bind(date)
    .fetch_all(&state.db)
    .await?;

    let (queued, sent_today, failed_today): (i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE status = 'queued')::bigint,
            COUNT(*) FILTER (WHERE status = 'sent'   AND sent_at::date = $2)::bigint,
            COUNT(*) FILTER (WHERE status = 'failed' AND created_at::date = $2)::bigint
        FROM notification_outbox
        WHERE created_at > NOW() - INTERVAL '7 days'
          AND ($1::uuid IS NULL OR school_id = $1)
        "#,
    )
    .bind(school)
    .bind(date)
    .fetch_one(&state.db)
    .await?;

    Ok(ApiResponse::new(SchoolDashboard {
        summary_date: date,
        school_id: school,
        school_name,
        attendance_rate: attendance.attendance_rate(),
        face_coverage_percent: face_coverage.percentage(),
        attendance,
        face_coverage,
        classrooms,
        devices: DeviceHealth {
            total,
            online,
            offline: (total - online).max(0),
            never_paired,
        },
        trend,
        notifications: NotificationBrief { queued, sent_today, failed_today },
    }))
}

/// Ikhtisar seluruh provinsi — khusus Superadmin / Admin Dinas.
#[utoipa::path(
    get, path = "/v1/dashboard/province", tag = "Dashboard",
    params(("date" = Option<NaiveDate>, Query, description = "Default hari ini")),
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Ikhtisar provinsi", body = ProvinceOverview),
        (status = 403, description = "Bukan peran tingkat provinsi")
    )
)]
pub async fn province(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(q): ValidQuery<DashboardQuery>,
) -> ApiResult<ApiResponse<ProvinceOverview>> {
    user.require("view_dashboard")?;
    if !user.is_province_scope() {
        return Err(crate::error::ApiError::Forbidden(
            "Ikhtisar provinsi hanya untuk Superadmin dan Admin Dinas".into(),
        ));
    }
    let date = q.date.unwrap_or_else(util::today_wib);
    Ok(ApiResponse::new(
        reports::province_overview(&state.db, date).await?,
    ))
}

/// Umpan absensi terbaru — dipakai layar monitoring yang menyegar otomatis.
#[utoipa::path(
    get, path = "/v1/dashboard/live", tag = "Dashboard",
    params(("school_id" = Option<Uuid>, Query, description = "ID sekolah")),
    security(("bearer" = [])),
    responses((status = 200, description = "Absensi terbaru hari ini", body = [AttendanceRecord]))
)]
pub async fn live_feed(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(q): ValidQuery<DashboardQuery>,
) -> ApiResult<ApiResponse<Vec<AttendanceRecord>>> {
    user.require("view_attendance")?;
    let school = user.resolve_school(q.school_id)?;
    let date = q.date.unwrap_or_else(util::today_wib);

    let rows: Vec<AttendanceRecord> = sqlx::query_as(
        r#"
        SELECT id, attendance_date, school_id, school_name, student_id, student_name,
               student_nis, classroom_id, classroom_name, check_in_at, check_out_at,
               status, late_minutes, duration_minutes, check_in_method, check_out_method,
               notes, notification_status
        FROM attendances
        WHERE attendance_date = $2
          AND ($1::uuid IS NULL OR school_id = $1)
          AND (check_in_at IS NOT NULL OR check_out_at IS NOT NULL)
        ORDER BY GREATEST(
                   COALESCE(check_out_at, check_in_at),
                   COALESCE(check_in_at, check_out_at)
                 ) DESC
        LIMIT 50
        "#,
    )
    .bind(school)
    .bind(date)
    .fetch_all(&state.db)
    .await?;

    Ok(ApiResponse::new(rows))
}
