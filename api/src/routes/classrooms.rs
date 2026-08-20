//! Rombel / kelas.

use axum::extract::{Path, State};
use axum::routing::get;
use axum::Router;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::domain::school::{
    Classroom, ClassroomFilter, CreateClassroomRequest, UpdateClassroomRequest,
};
use crate::error::{ApiError, ApiResult};
use crate::extract::{ValidJson, ValidQuery};
use crate::routes::schools::active_academic_year;
use crate::services::audit::AuditEntry;
use crate::state::AppState;
use crate::util::{ApiResponse, PageQuery, Paginated};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/classrooms", get(list).post(create))
        .route("/classrooms/{id}", get(detail).patch(update).delete(remove))
}

const SORTABLE: [&str; 4] = ["name", "grade_level", "created_at", "student_count"];

/// Daftar kelas. Guru dapat memfilter hanya kelas yang ia ampu (`mine=true`).
#[utoipa::path(
    get, path = "/v1/classrooms", tag = "Kelas",
    params(PageQuery, ClassroomFilter),
    security(("bearer" = [])),
    responses((status = 200, description = "Daftar kelas", body = [Classroom]))
)]
pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(page): ValidQuery<PageQuery>,
    ValidQuery(filter): ValidQuery<ClassroomFilter>,
) -> ApiResult<Paginated<Classroom>> {
    user.require("view_classroom")?;
    let school = user.resolve_school(filter.school_id)?;
    let mine = filter.mine.unwrap_or(false).then_some(user.id);
    let search = page.search_pattern();

    let (total,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)::bigint FROM classrooms c
        WHERE c.deleted_at IS NULL
          AND ($1::uuid IS NULL OR c.school_id = $1)
          AND ($2::uuid IS NULL OR c.academic_year_id = $2)
          AND ($3::smallint IS NULL OR c.grade_level = $3)
          AND ($4::uuid IS NULL OR c.homeroom_teacher_id = $4)
          AND ($5::text IS NULL OR c.name ILIKE $5)
        "#,
    )
    .bind(school)
    .bind(filter.academic_year_id)
    .bind(filter.grade_level)
    .bind(mine)
    .bind(search.as_deref())
    .fetch_one(&state.db)
    .await?;

    let sql = format!(
        r#"
        SELECT c.id, c.school_id, s.name AS school_name, c.academic_year_id,
               ay.name AS academic_year_name, c.name, c.grade_level, c.major,
               c.homeroom_teacher_id, u.name AS homeroom_teacher_name, c.capacity,
               (SELECT COUNT(*)::bigint FROM students st
                 WHERE st.current_classroom_id = c.id
                   AND st.deleted_at IS NULL AND st.status = 'aktif') AS student_count,
               c.is_active
        FROM classrooms c
        JOIN schools s ON s.id = c.school_id
        JOIN academic_years ay ON ay.id = c.academic_year_id
        LEFT JOIN users u ON u.id = c.homeroom_teacher_id
        WHERE c.deleted_at IS NULL
          AND ($1::uuid IS NULL OR c.school_id = $1)
          AND ($2::uuid IS NULL OR c.academic_year_id = $2)
          AND ($3::smallint IS NULL OR c.grade_level = $3)
          AND ($4::uuid IS NULL OR c.homeroom_teacher_id = $4)
          AND ($5::text IS NULL OR c.name ILIKE $5)
        ORDER BY {}, c.name
        LIMIT $6 OFFSET $7
        "#,
        page.order_by(&SORTABLE, "grade_level")
    );

    let items: Vec<Classroom> = sqlx::query_as(&sql)
        .bind(school)
        .bind(filter.academic_year_id)
        .bind(filter.grade_level)
        .bind(mine)
        .bind(search.as_deref())
        .bind(page.per_page())
        .bind(page.offset())
        .fetch_all(&state.db)
        .await?;

    Ok(Paginated::new(items, page.page(), page.per_page(), total))
}

/// Detail satu kelas.
#[utoipa::path(
    get, path = "/v1/classrooms/{id}", tag = "Kelas",
    params(("id" = Uuid, Path, description = "ID kelas")),
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Detail kelas", body = Classroom),
        (status = 404, description = "Tidak ditemukan")
    )
)]
pub async fn detail(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<Classroom>> {
    user.require("view_classroom")?;
    let c = fetch(&state, id).await?;
    user.resolve_school(Some(c.school_id))?;
    Ok(ApiResponse::new(c))
}

/// Buat kelas baru.
#[utoipa::path(
    post, path = "/v1/classrooms", tag = "Kelas",
    request_body = CreateClassroomRequest,
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Kelas dibuat", body = Classroom),
        (status = 409, description = "Nama kelas sudah ada pada tahun ajaran ini")
    )
)]
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    ValidJson(body): ValidJson<CreateClassroomRequest>,
) -> ApiResult<ApiResponse<Classroom>> {
    user.require("create_classroom")?;
    let school_id = user.require_school(body.school_id)?;

    let academic_year_id = match body.academic_year_id {
        Some(id) => id,
        None => active_academic_year(&state).await?,
    };

    if let Some(teacher) = body.homeroom_teacher_id {
        ensure_teacher_in_school(&state, teacher, school_id).await?;
    }

    let (id,): (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO classrooms (
            school_id, academic_year_id, name, grade_level, major,
            homeroom_teacher_id, capacity
        ) VALUES ($1,$2,$3,$4,$5,$6,COALESCE($7,40))
        RETURNING id
        "#,
    )
    .bind(school_id)
    .bind(academic_year_id)
    .bind(body.name.trim())
    .bind(body.grade_level)
    .bind(body.major.as_deref())
    .bind(body.homeroom_teacher_id)
    .bind(body.capacity)
    .fetch_one(&state.db)
    .await?;

    let created = fetch(&state, id).await?;
    AuditEntry::by_user(&user, "classroom.create")
        .school(school_id)
        .entity("classroom", id)
        .after(&created)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        created,
        format!("Kelas {} berhasil dibuat", body.name.trim()),
    ))
}

/// Perbarui kelas.
#[utoipa::path(
    patch, path = "/v1/classrooms/{id}", tag = "Kelas",
    params(("id" = Uuid, Path, description = "ID kelas")),
    request_body = UpdateClassroomRequest,
    security(("bearer" = [])),
    responses((status = 200, description = "Kelas diperbarui", body = Classroom))
)]
pub async fn update(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ValidJson(body): ValidJson<UpdateClassroomRequest>,
) -> ApiResult<ApiResponse<Classroom>> {
    user.require("update_classroom")?;
    let before = fetch(&state, id).await?;
    user.resolve_school(Some(before.school_id))?;

    if let Some(teacher) = body.homeroom_teacher_id {
        ensure_teacher_in_school(&state, teacher, before.school_id).await?;
    }

    sqlx::query(
        r#"
        UPDATE classrooms SET
            name                = COALESCE($2, name),
            grade_level         = COALESCE($3, grade_level),
            major               = COALESCE($4, major),
            homeroom_teacher_id = COALESCE($5, homeroom_teacher_id),
            capacity            = COALESCE($6, capacity),
            is_active           = COALESCE($7, is_active),
            updated_at          = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(body.name.as_deref().map(str::trim))
    .bind(body.grade_level)
    .bind(body.major.as_deref())
    .bind(body.homeroom_teacher_id)
    .bind(body.capacity)
    .bind(body.is_active)
    .execute(&state.db)
    .await?;

    let after = fetch(&state, id).await?;
    AuditEntry::by_user(&user, "classroom.update")
        .school(after.school_id)
        .entity("classroom", id)
        .before(&before)
        .after(&after)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(after, "Kelas diperbarui"))
}

/// Hapus kelas.
///
/// Ditolak bila masih ada siswa aktif di dalamnya — memindahkan siswa ke
/// kelas lain adalah keputusan operator, bukan efek samping penghapusan.
#[utoipa::path(
    delete, path = "/v1/classrooms/{id}", tag = "Kelas",
    params(("id" = Uuid, Path, description = "ID kelas")),
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Kelas dihapus"),
        (status = 409, description = "Masih ada siswa di kelas ini")
    )
)]
pub async fn remove(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require("delete_classroom")?;
    let before = fetch(&state, id).await?;
    user.resolve_school(Some(before.school_id))?;

    if before.student_count > 0 {
        return Err(ApiError::Conflict(format!(
            "Kelas {} masih memiliki {} siswa aktif. Pindahkan siswa terlebih dahulu.",
            before.name, before.student_count
        )));
    }

    sqlx::query("UPDATE classrooms SET deleted_at = NOW(), is_active = FALSE WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    AuditEntry::by_user(&user, "classroom.delete")
        .school(before.school_id)
        .entity("classroom", id)
        .before(&before)
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        serde_json::json!({ "deleted": true }),
        "Kelas dihapus",
    ))
}

// =====================================================================

pub async fn fetch(state: &AppState, id: Uuid) -> ApiResult<Classroom> {
    let row: Option<Classroom> = sqlx::query_as(
        r#"
        SELECT c.id, c.school_id, s.name AS school_name, c.academic_year_id,
               ay.name AS academic_year_name, c.name, c.grade_level, c.major,
               c.homeroom_teacher_id, u.name AS homeroom_teacher_name, c.capacity,
               (SELECT COUNT(*)::bigint FROM students st
                 WHERE st.current_classroom_id = c.id
                   AND st.deleted_at IS NULL AND st.status = 'aktif') AS student_count,
               c.is_active
        FROM classrooms c
        JOIN schools s ON s.id = c.school_id
        JOIN academic_years ay ON ay.id = c.academic_year_id
        LEFT JOIN users u ON u.id = c.homeroom_teacher_id
        WHERE c.id = $1 AND c.deleted_at IS NULL
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    row.ok_or_else(|| ApiError::NotFound(format!("kelas `{id}`")))
}

/// Wali kelas harus pegawai di sekolah yang sama — mencegah operator satu
/// sekolah menautkan guru sekolah lain (yang lalu bisa membaca data siswa).
pub async fn ensure_teacher_in_school(
    state: &AppState,
    teacher_id: Uuid,
    school_id: Uuid,
) -> ApiResult<()> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT id FROM users
        WHERE id = $1 AND deleted_at IS NULL AND is_active
          AND (school_id = $2 OR EXISTS (
              SELECT 1 FROM user_school_scopes uss
              WHERE uss.user_id = $1 AND uss.school_id = $2
          ))
        "#,
    )
    .bind(teacher_id)
    .bind(school_id)
    .fetch_optional(&state.db)
    .await?;

    row.map(|_| ()).ok_or_else(|| {
        ApiError::field(
            "homeroom_teacher_id",
            "guru tidak ditemukan atau bukan pegawai sekolah ini",
        )
    })
}
