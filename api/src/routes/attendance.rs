//! Monitoring & koreksi absensi.
//!
//! Inilah layar utama guru dan kepala sekolah: siapa yang sudah datang, siapa
//! yang terlambat, siapa yang belum hadir — dan kemampuan mengoreksi ketika
//! sistem atau siswa keliru (lupa absen, sakit, izin).

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::Router;
use chrono::NaiveDate;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::domain::attendance::{
    AttendanceFilter, AttendanceRecord, AttendanceRule, AttendanceStatus, AttendanceSummary,
    BulkAttendanceRequest, BulkAttendanceResponse, ClassroomSummary, ManualAttendanceRequest,
    StudentAttendanceRecap, UpsertAttendanceRuleRequest,
};
use crate::error::{ApiError, ApiResult};
use crate::extract::{ValidJson, ValidQuery};
use crate::services::audit::AuditEntry;
use crate::services::notify::{self, AttendanceNotifyContext, NotifyEvent};
use crate::services::reports;
use crate::state::AppState;
use crate::util::{self, ApiResponse, DateRangeQuery, PageQuery, Paginated};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/attendances", get(list))
        .route("/attendances/manual", post(manual))
        .route("/attendances/bulk", post(bulk))
        .route("/attendances/summary", get(summary))
        .route("/attendances/by-classroom", get(by_classroom))
        .route("/attendances/recap", get(recap))
        .route("/attendances/student/{student_id}", get(student_history))
        .route("/attendance-rules", get(list_rules).post(upsert_rule))
}

/// Daftar absensi pada rentang tanggal.
#[utoipa::path(
    get, path = "/v1/attendances", tag = "Absensi",
    params(PageQuery, DateRangeQuery, AttendanceFilter),
    security(("bearer" = [])),
    responses((status = 200, description = "Daftar absensi", body = [AttendanceRecord]))
)]
pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(page): ValidQuery<PageQuery>,
    ValidQuery(range): ValidQuery<DateRangeQuery>,
    ValidQuery(filter): ValidQuery<AttendanceFilter>,
) -> ApiResult<Paginated<AttendanceRecord>> {
    user.require("view_attendance")?;
    let school = user.resolve_school(filter.school_id)?;
    let (from, to) = range.resolve()?;

    if let Some(s) = filter.status.as_deref() {
        if AttendanceStatus::parse(s).is_none() {
            return Err(ApiError::field("status", "status absensi tidak dikenal"));
        }
    }

    let search = page.search_pattern();
    let missing_check_out = filter.missing_check_out.unwrap_or(false);

    let (total,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)::bigint FROM attendances a
        WHERE a.attendance_date BETWEEN $1 AND $2
          AND ($3::uuid IS NULL OR a.school_id = $3)
          AND ($4::uuid IS NULL OR a.classroom_id = $4)
          AND ($5::uuid IS NULL OR a.student_id = $5)
          AND ($6::text IS NULL OR a.status = $6)
          AND (NOT $7::boolean OR a.check_out_at IS NULL)
          AND ($8::text IS NULL OR a.student_name ILIKE $8 OR a.student_nis ILIKE $8)
        "#,
    )
    .bind(from)
    .bind(to)
    .bind(school)
    .bind(filter.classroom_id)
    .bind(filter.student_id)
    .bind(filter.status.as_deref())
    .bind(missing_check_out)
    .bind(search.as_deref())
    .fetch_one(&state.db)
    .await?;

    let items: Vec<AttendanceRecord> = sqlx::query_as(
        r#"
        SELECT a.id, a.attendance_date, a.school_id, a.school_name, a.student_id,
               a.student_name, a.student_nis, a.classroom_id, a.classroom_name,
               a.check_in_at, a.check_out_at, a.status, a.late_minutes,
               a.duration_minutes, a.check_in_method, a.check_out_method,
               a.notes, a.notification_status
        FROM attendances a
        WHERE a.attendance_date BETWEEN $1 AND $2
          AND ($3::uuid IS NULL OR a.school_id = $3)
          AND ($4::uuid IS NULL OR a.classroom_id = $4)
          AND ($5::uuid IS NULL OR a.student_id = $5)
          AND ($6::text IS NULL OR a.status = $6)
          AND (NOT $7::boolean OR a.check_out_at IS NULL)
          AND ($8::text IS NULL OR a.student_name ILIKE $8 OR a.student_nis ILIKE $8)
        ORDER BY a.attendance_date DESC, a.check_in_at DESC NULLS LAST, a.student_name
        LIMIT $9 OFFSET $10
        "#,
    )
    .bind(from)
    .bind(to)
    .bind(school)
    .bind(filter.classroom_id)
    .bind(filter.student_id)
    .bind(filter.status.as_deref())
    .bind(missing_check_out)
    .bind(search.as_deref())
    .bind(page.per_page())
    .bind(page.offset())
    .fetch_all(&state.db)
    .await?;

    Ok(Paginated::new(items, page.page(), page.per_page(), total))
}

/// Riwayat absensi satu siswa.
#[utoipa::path(
    get, path = "/v1/attendances/student/{student_id}", tag = "Absensi",
    params(("student_id" = Uuid, Path, description = "ID siswa"), DateRangeQuery),
    security(("bearer" = [])),
    responses((status = 200, description = "Riwayat absensi siswa", body = [AttendanceRecord]))
)]
pub async fn student_history(
    State(state): State<AppState>,
    user: AuthUser,
    Path(student_id): Path<Uuid>,
    ValidQuery(range): ValidQuery<DateRangeQuery>,
) -> ApiResult<ApiResponse<Vec<AttendanceRecord>>> {
    user.require("view_attendance")?;
    let student = crate::routes::students::fetch(&state, student_id).await?;
    user.resolve_school(Some(student.school_id))?;
    let (from, to) = range.resolve()?;

    let items: Vec<AttendanceRecord> = sqlx::query_as(
        r#"
        SELECT id, attendance_date, school_id, school_name, student_id, student_name,
               student_nis, classroom_id, classroom_name, check_in_at, check_out_at,
               status, late_minutes, duration_minutes, check_in_method, check_out_method,
               notes, notification_status
        FROM attendances
        WHERE student_id = $1 AND attendance_date BETWEEN $2 AND $3
        ORDER BY attendance_date DESC
        "#,
    )
    .bind(student_id)
    .bind(from)
    .bind(to)
    .fetch_all(&state.db)
    .await?;

    Ok(ApiResponse::new(items))
}

/// Koreksi absensi satu siswa (izin, sakit, lupa absen).
#[utoipa::path(
    post, path = "/v1/attendances/manual", tag = "Absensi",
    request_body = ManualAttendanceRequest,
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Absensi dikoreksi", body = AttendanceRecord),
        (status = 403, description = "Tidak punya izin koreksi")
    )
)]
pub async fn manual(
    State(state): State<AppState>,
    user: AuthUser,
    ValidJson(body): ValidJson<ManualAttendanceRequest>,
) -> ApiResult<ApiResponse<AttendanceRecord>> {
    user.require("override_attendance")?;

    let student = crate::routes::students::fetch(&state, body.student_id).await?;
    user.resolve_school(Some(student.school_id))?;

    let date = body.attendance_date.unwrap_or_else(util::today_wib);
    if date > util::today_wib() {
        return Err(ApiError::field(
            "attendance_date",
            "tidak dapat mengisi absensi untuk tanggal di masa depan",
        ));
    }

    // Status hadir/terlambat tanpa jam masuk akan menghasilkan baris yang
    // membingungkan di laporan ("hadir, jam masuk -"), jadi ditolak di sini.
    if body.status.is_present() && body.check_in_time.is_none() {
        return Err(ApiError::field(
            "check_in_time",
            "jam masuk wajib diisi untuk status hadir/terlambat/dispensasi",
        ));
    }

    let before = crate::services::recognition::load_today_attendance(&state, date, student.id).await?;

    let check_in_at = body.check_in_time.map(|t| util::wib_datetime(date, t));
    let check_out_at = body.check_out_time.map(|t| util::wib_datetime(date, t));

    if let (Some(ci), Some(co)) = (check_in_at, check_out_at) {
        if co <= ci {
            return Err(ApiError::field(
                "check_out_time",
                "jam pulang harus setelah jam masuk",
            ));
        }
    }

    // Keterlambatan tetap dihitung dari aturan resmi, bukan diserahkan ke
    // operator — agar angka di laporan konsisten dengan absensi otomatis.
    let rule = crate::services::rules::resolve_rule(
        &state.db,
        student.school_id,
        student.current_classroom_id,
        date,
    )
    .await?;
    let late_minutes = match (body.status, body.check_in_time) {
        (AttendanceStatus::Terlambat, Some(t)) => rule.classify_check_in(t).1,
        _ => 0,
    };

    let mut tx = state.db.begin().await?;

    let (attendance_id,): (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO attendances (
            attendance_date, school_id, student_id, classroom_id, academic_year_id,
            student_name, student_nis, classroom_name, school_name,
            check_in_at, check_out_at, status, late_minutes,
            check_in_method, notes, marked_by, marked_at
        )
        SELECT $1, s.school_id, s.id, s.current_classroom_id, c.academic_year_id,
               s.full_name, s.nis, c.name, sc.name,
               $2, $3, $4, $5, 'manual', $6, $7, NOW()
        FROM students s
        JOIN schools sc ON sc.id = s.school_id
        LEFT JOIN classrooms c ON c.id = s.current_classroom_id
        WHERE s.id = $8
        ON CONFLICT (attendance_date, student_id) DO UPDATE SET
            check_in_at   = COALESCE(EXCLUDED.check_in_at, attendances.check_in_at),
            check_out_at  = COALESCE(EXCLUDED.check_out_at, attendances.check_out_at),
            status        = EXCLUDED.status,
            late_minutes  = EXCLUDED.late_minutes,
            notes         = EXCLUDED.notes,
            marked_by     = EXCLUDED.marked_by,
            marked_at     = NOW(),
            updated_at    = NOW()
        RETURNING id
        "#,
    )
    .bind(date)
    .bind(check_in_at)
    .bind(check_out_at)
    .bind(body.status.as_str())
    .bind(late_minutes)
    .bind(body.notes.trim())
    .bind(user.id)
    .bind(student.id)
    .fetch_one(&mut *tx)
    .await?;

    if body.notify_guardian && state.cfg.notify.enabled {
        let ctx = AttendanceNotifyContext {
            school_id: student.school_id,
            school_name: student.school_name.clone(),
            student_id: student.id,
            student_name: student.full_name.clone(),
            student_nis: student.nis.clone(),
            classroom_name: student.classroom_name.clone(),
            attendance_id,
            attendance_date: date,
            status: body.status,
            check_in_at,
            check_out_at,
            late_minutes,
        };
        notify::enqueue_attendance(&mut tx, &ctx, NotifyEvent::ManualCorrection).await?;
    }

    tx.commit().await?;

    let after = crate::services::recognition::load_today_attendance(&state, date, student.id)
        .await?
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("absensi tidak ditemukan setelah commit")))?;

    AuditEntry::by_user(&user, "attendance.override")
        .school(student.school_id)
        .entity("attendance", attendance_id)
        .before(&before)
        .after(&after)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        after,
        format!(
            "Absensi {} pada {} diubah menjadi {}",
            student.full_name,
            util::format_date_id(date),
            notify::status_label(body.status)
        ),
    ))
}

/// Koreksi massal (mis. satu kelas mengikuti lomba -> dispensasi).
#[utoipa::path(
    post, path = "/v1/attendances/bulk", tag = "Absensi",
    request_body = BulkAttendanceRequest,
    security(("bearer" = [])),
    responses((status = 200, description = "Hasil koreksi massal", body = BulkAttendanceResponse))
)]
pub async fn bulk(
    State(state): State<AppState>,
    user: AuthUser,
    ValidJson(body): ValidJson<BulkAttendanceRequest>,
) -> ApiResult<ApiResponse<BulkAttendanceResponse>> {
    user.require("override_attendance")?;

    let date = body.attendance_date.unwrap_or_else(util::today_wib);
    if date > util::today_wib() {
        return Err(ApiError::field(
            "attendance_date",
            "tidak dapat mengisi absensi untuk tanggal di masa depan",
        ));
    }

    let mut updated = 0usize;
    let mut skipped = 0usize;
    let mut errors = Vec::new();

    for student_id in &body.student_ids {
        let student = match crate::routes::students::fetch(&state, *student_id).await {
            Ok(s) => s,
            Err(_) => {
                skipped += 1;
                errors.push(format!("Siswa {student_id} tidak ditemukan"));
                continue;
            }
        };
        // Penjaga tenant diterapkan per baris: satu id asing tidak boleh
        // menyelinap lewat operasi massal.
        if user.resolve_school(Some(student.school_id)).is_err() {
            skipped += 1;
            errors.push(format!(
                "{} bukan siswa sekolah Anda",
                student.full_name
            ));
            continue;
        }

        let mut tx = state.db.begin().await?;
        let inserted: Result<(Uuid,), _> = sqlx::query_as(
            r#"
            INSERT INTO attendances (
                attendance_date, school_id, student_id, classroom_id, academic_year_id,
                student_name, student_nis, classroom_name, school_name,
                status, notes, marked_by, marked_at, check_in_method
            )
            SELECT $1, s.school_id, s.id, s.current_classroom_id, c.academic_year_id,
                   s.full_name, s.nis, c.name, sc.name, $2, $3, $4, NOW(), 'manual'
            FROM students s
            JOIN schools sc ON sc.id = s.school_id
            LEFT JOIN classrooms c ON c.id = s.current_classroom_id
            WHERE s.id = $5
            ON CONFLICT (attendance_date, student_id) DO UPDATE SET
                status = EXCLUDED.status, notes = EXCLUDED.notes,
                marked_by = EXCLUDED.marked_by, marked_at = NOW(), updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(date)
        .bind(body.status.as_str())
        .bind(body.notes.trim())
        .bind(user.id)
        .bind(student.id)
        .fetch_one(&mut *tx)
        .await;

        match inserted {
            Ok((attendance_id,)) => {
                if body.notify_guardian && state.cfg.notify.enabled {
                    let ctx = AttendanceNotifyContext {
                        school_id: student.school_id,
                        school_name: student.school_name.clone(),
                        student_id: student.id,
                        student_name: student.full_name.clone(),
                        student_nis: student.nis.clone(),
                        classroom_name: student.classroom_name.clone(),
                        attendance_id,
                        attendance_date: date,
                        status: body.status,
                        check_in_at: None,
                        check_out_at: None,
                        late_minutes: 0,
                    };
                    notify::enqueue_attendance(&mut tx, &ctx, NotifyEvent::ManualCorrection).await?;
                }
                tx.commit().await?;
                updated += 1;
            }
            Err(e) => {
                tx.rollback().await?;
                skipped += 1;
                errors.push(format!("{}: {e}", student.full_name));
            }
        }
    }

    AuditEntry::by_user(&user, "attendance.bulk_override")
        .after(&serde_json::json!({
            "date": date, "status": body.status.as_str(),
            "updated": updated, "skipped": skipped
        }))
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        BulkAttendanceResponse { updated, skipped, errors },
        format!("{updated} absensi diperbarui, {skipped} dilewati"),
    ))
}

/// Ringkasan absensi satu hari.
#[utoipa::path(
    get, path = "/v1/attendances/summary", tag = "Absensi",
    params(
        ("school_id" = Option<Uuid>, Query, description = "Kosongkan untuk seluruh provinsi (khusus Dinas)"),
        ("date" = Option<NaiveDate>, Query, description = "Default hari ini")
    ),
    security(("bearer" = [])),
    responses((status = 200, description = "Ringkasan absensi", body = AttendanceSummary))
)]
pub async fn summary(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(q): ValidQuery<SummaryQuery>,
) -> ApiResult<ApiResponse<AttendanceSummary>> {
    user.require("view_attendance")?;
    let school = user.resolve_school(q.school_id)?;
    let date = q.date.unwrap_or_else(util::today_wib);

    let s = reports::daily_summary(&state.db, school, date).await?;
    Ok(ApiResponse::new(s))
}

#[derive(Debug, serde::Deserialize)]
pub struct SummaryQuery {
    pub school_id: Option<Uuid>,
    pub date: Option<NaiveDate>,
}

/// Ringkasan per kelas — layar utama monitoring kepala sekolah.
#[utoipa::path(
    get, path = "/v1/attendances/by-classroom", tag = "Absensi",
    params(
        ("school_id" = Option<Uuid>, Query, description = "ID sekolah"),
        ("date" = Option<NaiveDate>, Query, description = "Default hari ini")
    ),
    security(("bearer" = [])),
    responses((status = 200, description = "Ringkasan per kelas", body = [ClassroomSummary]))
)]
pub async fn by_classroom(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(q): ValidQuery<SummaryQuery>,
) -> ApiResult<ApiResponse<Vec<ClassroomSummary>>> {
    user.require("view_attendance")?;
    let school_id = user.require_school(q.school_id)?;
    let date = q.date.unwrap_or_else(util::today_wib);

    let rows = reports::classroom_summaries(&state.db, school_id, date).await?;
    Ok(ApiResponse::new(rows))
}

/// Rekap kehadiran per siswa untuk rentang tanggal (bahan rapor).
#[utoipa::path(
    get, path = "/v1/attendances/recap", tag = "Absensi",
    params(DateRangeQuery, AttendanceFilter),
    security(("bearer" = [])),
    responses((status = 200, description = "Rekap per siswa", body = [StudentAttendanceRecap]))
)]
pub async fn recap(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(range): ValidQuery<DateRangeQuery>,
    ValidQuery(filter): ValidQuery<AttendanceFilter>,
) -> ApiResult<ApiResponse<Vec<StudentAttendanceRecap>>> {
    user.require("view_report")?;
    let school_id = user.require_school(filter.school_id)?;
    let (from, to) = range.resolve()?;

    let rows =
        reports::student_recap(&state.db, school_id, filter.classroom_id, from, to).await?;
    Ok(ApiResponse::new(rows))
}

// =====================================================================
// Aturan jam absensi
// =====================================================================

/// Daftar aturan jam absensi.
#[utoipa::path(
    get, path = "/v1/attendance-rules", tag = "Absensi",
    params(("school_id" = Option<Uuid>, Query, description = "ID sekolah")),
    security(("bearer" = [])),
    responses((status = 200, description = "Daftar aturan", body = [AttendanceRule]))
)]
pub async fn list_rules(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(q): ValidQuery<SummaryQuery>,
) -> ApiResult<ApiResponse<Vec<AttendanceRule>>> {
    user.require_any(&["view_attendance", "manage_attendance_rule"])?;
    let school_id = user.require_school(q.school_id)?;

    let rows: Vec<AttendanceRule> = sqlx::query_as(
        r#"
        SELECT id, school_id, classroom_id, name,
               check_in_opens_at, check_in_start_at, check_in_due_at, check_in_closes_at,
               check_out_opens_at, check_out_closes_at,
               late_grace_minutes, active_weekdays, require_check_out, is_active
        FROM attendance_rules
        WHERE school_id = $1
        ORDER BY (classroom_id IS NULL) DESC, name
        "#,
    )
    .bind(school_id)
    .fetch_all(&state.db)
    .await?;

    Ok(ApiResponse::new(rows))
}

/// Atur jam masuk/pulang (per sekolah atau per kelas).
#[utoipa::path(
    post, path = "/v1/attendance-rules", tag = "Absensi",
    request_body = UpsertAttendanceRuleRequest,
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Aturan disimpan", body = AttendanceRule),
        (status = 422, description = "Urutan jam tidak konsisten")
    )
)]
pub async fn upsert_rule(
    State(state): State<AppState>,
    user: AuthUser,
    ValidJson(body): ValidJson<UpsertAttendanceRuleRequest>,
) -> ApiResult<ApiResponse<AttendanceRule>> {
    user.require("manage_attendance_rule")?;
    let school_id = user.require_school(body.school_id)?;
    body.validate_windows()?;

    // Aturan lama dinonaktifkan, bukan dihapus: absensi yang sudah tercatat
    // dinilai dengan aturan yang berlaku saat itu, dan jejaknya harus ada.
    let mut tx = state.db.begin().await?;
    sqlx::query(
        r#"
        UPDATE attendance_rules
           SET is_active = FALSE, effective_to = CURRENT_DATE, updated_at = NOW()
         WHERE school_id = $1
           AND is_active
           AND classroom_id IS NOT DISTINCT FROM $2
        "#,
    )
    .bind(school_id)
    .bind(body.classroom_id)
    .execute(&mut *tx)
    .await?;

    let (id,): (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO attendance_rules (
            school_id, classroom_id, name,
            check_in_opens_at, check_in_start_at, check_in_due_at, check_in_closes_at,
            check_out_opens_at, check_out_closes_at,
            late_grace_minutes, active_weekdays, require_check_out
        ) VALUES (
            $1,$2,COALESCE($3,'Jadwal Reguler'),
            $4,$5,$6,$7,$8,$9,
            COALESCE($10,0), COALESCE($11,31), COALESCE($12,TRUE)
        )
        RETURNING id
        "#,
    )
    .bind(school_id)
    .bind(body.classroom_id)
    .bind(body.name.as_deref())
    .bind(body.check_in_opens_at)
    .bind(body.check_in_start_at)
    .bind(body.check_in_due_at)
    .bind(body.check_in_closes_at)
    .bind(body.check_out_opens_at)
    .bind(body.check_out_closes_at)
    .bind(body.late_grace_minutes)
    .bind(body.active_weekdays)
    .bind(body.require_check_out)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    let row: AttendanceRule = sqlx::query_as(
        r#"
        SELECT id, school_id, classroom_id, name,
               check_in_opens_at, check_in_start_at, check_in_due_at, check_in_closes_at,
               check_out_opens_at, check_out_closes_at,
               late_grace_minutes, active_weekdays, require_check_out, is_active
        FROM attendance_rules WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    AuditEntry::by_user(&user, "attendance_rule.upsert")
        .school(school_id)
        .entity("attendance_rule", id)
        .after(&row)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(row, "Aturan jam absensi disimpan"))
}
