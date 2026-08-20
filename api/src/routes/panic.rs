//! Panic Button — kanal pengaduan anonim.
//!
//! SIAPA MELIHAT APA
//!
//! | Peran | Feed | Nama sekolah | Kategori sensitif | Identitas pelapor |
//! |---|---|---|---|---|
//! | siswa, orang tua, guru, staff | seluruh provinsi, sudah dimoderasi | **disamarkan** | terlihat | tidak pernah |
//! | kepala sekolah | sekolahnya sendiri | asli | **disembunyikan** | tidak pernah |
//! | petugas pengaduan, admin dinas | semua, termasuk `terbatas` | asli | terlihat | tidak |
//! | superadmin + izin khusus | semua | asli | terlihat | lewat endpoint unmask, dicatat |
//!
//! DUA KEPUTUSAN YANG PERLU DIJELASKAN
//!
//! **Feed dibuat lintas provinsi dengan nama sekolah disamarkan**, bukan
//! feed per sekolah. Feed per sekolah terasa lebih alami, tetapi pada sekolah
//! kecil, satu laporan yang muncul di layar seluruh sekolah akan langsung
//! memicu pencarian siapa yang menulis. Dengan feed provinsi + nama sekolah
//! disamarkan, siswa tetap melihat bahwa ia tidak sendirian, tanpa
//! menyerahkan daftar keluhan sekolah kepada sekolah itu sendiri.
//!
//! **Kategori paling sensitif tidak pernah ditampilkan ke pihak sekolah.**
//! Laporan kekerasan oleh guru, pelecehan, dan pungli sering kali menyangkut
//! orang yang justru berwenang di sekolah itu. Menampilkannya di dashboard
//! kepala sekolah berarti memberi tahu terlapor bahwa ada yang melapor.
//! Laporan-laporan itu langsung ditangani Dinas.

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::Router;
use base64::Engine as _;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::domain::panic::{
    CategoryCount, CreateCommentRequest, CreateReportRequest, CreateReportResponse, FeedFilter,
    ModerateReportRequest, PanicCategory, PanicComment, PanicFeedItem, PanicReportDetail,
    PanicStats, PanicTimelineEntry, UnmaskRequest, UnmaskedAuthor, UpdateReportStatusRequest,
    MODERATION_STATUSES, SEVERITIES, STATUSES, URGENT_SEVERITIES, VISIBILITIES,
};
use crate::error::{ApiError, ApiResult};
use crate::extract::{ValidJson, ValidQuery};
use crate::face::quality;
use crate::services::anonymity;
use crate::services::audit::AuditEntry;
use crate::state::AppState;
use crate::util::{ApiResponse, PageQuery, Paginated};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/panic/categories", get(categories))
        .route("/panic/reports", get(feed).post(create_report))
        .route("/panic/reports/{id}", get(detail))
        .route("/panic/reports/{id}/support", post(toggle_support))
        .route("/panic/reports/{id}/comments", post(add_comment))
        .route("/panic/reports/{id}/moderate", post(moderate))
        .route("/panic/reports/{id}/status", post(update_status))
        .route("/panic/reports/{id}/unmask", post(unmask))
        .route("/panic/stats", get(stats))
}

/// Kategori yang tidak pernah ditampilkan kepada peran tingkat sekolah.
///
/// Ketiganya lazim melibatkan pihak yang berwenang di sekolah itu sendiri.
const SCHOOL_HIDDEN_CATEGORIES: [&str; 3] = ["kekerasan", "pelecehan", "pungli"];

/// Peran yang berwenang menangani pengaduan di tingkat provinsi.
const HANDLER_ROLES: [&str; 3] = ["petugas_pengaduan", "admin_dinas", "superadmin"];

/// Daftar kategori pengaduan.
#[utoipa::path(
    get, path = "/v1/panic/categories", tag = "Panic Button",
    security(("bearer" = [])),
    responses((status = 200, description = "Daftar kategori", body = [PanicCategory]))
)]
pub async fn categories(
    State(state): State<AppState>,
    _user: AuthUser,
) -> ApiResult<ApiResponse<Vec<PanicCategory>>> {
    let rows: Vec<PanicCategory> = sqlx::query_as(
        r#"
        SELECT id, code, name, description, icon, default_severity
        FROM panic_categories WHERE is_active
        ORDER BY sort_order, name
        "#,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(ApiResponse::new(rows))
}

#[derive(Debug, sqlx::FromRow)]
struct FeedRow {
    id: Uuid,
    created_at: chrono::DateTime<chrono::Utc>,
    school_id: Uuid,
    school_name: String,
    school_jenjang: String,
    category_code: String,
    category_name: String,
    category_icon: Option<String>,
    anonymous_handle: String,
    author_role: String,
    title: String,
    body: String,
    severity: String,
    status: String,
    support_count: i16,
    comment_count: i16,
    is_mine: bool,
    is_supported: bool,
    handled_at: Option<chrono::DateTime<chrono::Utc>>,
    resolved_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Feed pengaduan.
#[utoipa::path(
    get, path = "/v1/panic/reports", tag = "Panic Button",
    params(PageQuery, FeedFilter),
    security(("bearer" = [])),
    responses((status = 200, description = "Daftar laporan", body = [PanicFeedItem]))
)]
pub async fn feed(
    State(state): State<AppState>,
    user: AuthUser,
    ValidQuery(page): ValidQuery<PageQuery>,
    ValidQuery(filter): ValidQuery<FeedFilter>,
) -> ApiResult<Paginated<PanicFeedItem>> {
    user.require("view_panic_feed")?;

    let mut view = ViewerScope::of(&user);
    let mine_only = filter.mine.unwrap_or(false);
    let pending_only = filter.pending_moderation.unwrap_or(false);

    // Petugas boleh mempersempit feed ke satu sekolah. Untuk peran lain,
    // parameter ini diabaikan — bukan ditolak — karena membiarkan warga
    // biasa memfilter per sekolah sama saja dengan membuka penyamaran nama
    // sekolah lewat pintu belakang.
    if let (true, Some(school_id)) = (view.is_handler, filter.school_id) {
        view.school_ids = Some(vec![school_id]);
    }

    if pending_only && !user.has_permission("moderate_panic_report") {
        return Err(ApiError::Forbidden(
            "Hanya petugas yang dapat melihat antrean moderasi".into(),
        ));
    }

    // Batas 180 hari: tabel dipartisi bulanan, dan feed pengaduan yang lebih
    // tua dari itu tidak lagi relevan untuk ditindaklanjuti.
    let where_sql = build_feed_filter(&view, mine_only, pending_only);

    let total: (i64,) = sqlx::query_as(&format!(
        "SELECT COUNT(*)::bigint FROM panic_reports r
         JOIN panic_categories c ON c.id = r.category_id
         WHERE r.created_at > NOW() - INTERVAL '180 days' AND {where_sql}"
    ))
    .bind(user.id)
    .bind(view.school_ids.as_deref())
    .bind(filter.category_code.as_deref())
    .bind(filter.severity.as_deref())
    .bind(filter.status.as_deref())
    .fetch_one(&state.db)
    .await?;

    let rows: Vec<FeedRow> = sqlx::query_as(&format!(
        r#"
        SELECT r.id, r.created_at, r.school_id, s.name AS school_name,
               s.jenjang AS school_jenjang,
               c.code AS category_code, c.name AS category_name, c.icon AS category_icon,
               r.anonymous_handle, r.author_role, r.title, r.body,
               r.severity, r.status, r.support_count, r.comment_count,
               (r.author_user_id = $1) AS is_mine,
               EXISTS (
                   SELECT 1 FROM panic_supports ps
                   WHERE ps.report_id = r.id AND ps.user_id = $1
               ) AS is_supported,
               r.handled_at, r.resolved_at
        FROM panic_reports r
        JOIN panic_categories c ON c.id = r.category_id
        JOIN schools s          ON s.id = r.school_id
        WHERE r.created_at > NOW() - INTERVAL '180 days' AND {where_sql}
        ORDER BY
            -- Laporan darurat yang belum ditangani naik ke atas, sisanya
            -- terbaru dulu.
            (r.severity = 'darurat' AND r.handled_at IS NULL) DESC,
            r.created_at DESC
        LIMIT $6 OFFSET $7
        "#
    ))
    .bind(user.id)
    .bind(view.school_ids.as_deref())
    .bind(filter.category_code.as_deref())
    .bind(filter.severity.as_deref())
    .bind(filter.status.as_deref())
    .bind(page.per_page())
    .bind(page.offset())
    .fetch_all(&state.db)
    .await?;

    let media = load_media_map(&state, rows.iter().map(|r| r.id).collect()).await?;
    let items = rows
        .into_iter()
        .map(|r| to_feed_item(&state, r, &view, &media))
        .collect();

    Ok(Paginated::new(items, page.page(), page.per_page(), total.0))
}

/// Detail laporan beserta lini masa dan komentar.
#[utoipa::path(
    get, path = "/v1/panic/reports/{id}", tag = "Panic Button",
    params(("id" = Uuid, Path, description = "ID laporan")),
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Detail laporan", body = PanicReportDetail),
        (status = 404, description = "Tidak ditemukan atau di luar cakupan Anda")
    )
)]
pub async fn detail(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<PanicReportDetail>> {
    user.require("view_panic_feed")?;
    let view = ViewerScope::of(&user);

    let row = fetch_report(&state, &user, &view, id).await?;
    let media = load_media_map(&state, vec![id]).await?;

    let moderation: (String, Option<String>) = sqlx::query_as(
        "SELECT moderation_status, moderation_note FROM panic_reports WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    let resolution: (Option<String>,) =
        sqlx::query_as("SELECT resolution FROM panic_reports WHERE id = $1")
            .bind(id)
            .fetch_one(&state.db)
            .await?;

    let timeline: Vec<PanicTimelineEntry> = sqlx::query_as(
        r#"
        SELECT status, note, actor_label, created_at
        FROM panic_report_events
        WHERE report_id = $1 AND ($2 OR is_public)
        ORDER BY created_at
        "#,
    )
    .bind(id)
    .bind(view.is_handler)
    .fetch_all(&state.db)
    .await?;

    #[derive(sqlx::FromRow)]
    struct CommentRow {
        id: Uuid,
        anonymous_handle: String,
        is_official: bool,
        official_name: Option<String>,
        official_title: Option<String>,
        body: String,
        is_mine: bool,
        created_at: chrono::DateTime<chrono::Utc>,
    }

    let comments: Vec<CommentRow> = sqlx::query_as(
        r#"
        SELECT id, anonymous_handle, is_official, official_name, official_title,
               body, (author_user_id = $2) AS is_mine, created_at
        FROM panic_comments
        WHERE report_id = $1
          AND deleted_at IS NULL
          AND (moderation_status = 'approved' OR author_user_id = $2 OR $3)
        ORDER BY created_at
        "#,
    )
    .bind(id)
    .bind(user.id)
    .bind(view.is_handler)
    .fetch_all(&state.db)
    .await?;

    let school_name = row.school_name.clone();
    let is_handler = view.is_handler;

    Ok(ApiResponse::new(PanicReportDetail {
        report: to_feed_item(&state, row, &view, &media),
        timeline,
        comments: comments
            .into_iter()
            .map(|c| PanicComment {
                id: c.id,
                // Komentar resmi menampilkan nama petugas; sisanya anonim.
                anonymous_handle: (!c.is_official).then_some(c.anonymous_handle),
                is_official: c.is_official,
                official_name: c.official_name,
                official_title: c.official_title,
                body: c.body,
                is_mine: c.is_mine,
                created_at: c.created_at,
            })
            .collect(),
        school_name: is_handler.then_some(school_name),
        resolution: resolution.0,
        moderation_status: moderation.0,
        moderation_note: moderation.1,
    }))
}

/// Buat laporan baru.
#[utoipa::path(
    post, path = "/v1/panic/reports", tag = "Panic Button",
    request_body = CreateReportRequest,
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Laporan terkirim", body = CreateReportResponse),
        (status = 403, description = "Akun tidak berhak membuat laporan"),
        (status = 429, description = "Terlalu banyak laporan dalam waktu singkat")
    )
)]
pub async fn create_report(
    State(state): State<AppState>,
    user: AuthUser,
    ValidJson(body): ValidJson<CreateReportRequest>,
) -> ApiResult<ApiResponse<CreateReportResponse>> {
    user.require("create_panic_report")?;

    if !VISIBILITIES.contains(&body.visibility.as_str()) {
        return Err(ApiError::field("visibility", "pilih `publik` atau `terbatas`"));
    }
    if let Some(s) = &body.severity {
        if !SEVERITIES.contains(&s.as_str()) {
            return Err(ApiError::field(
                "severity",
                &format!("pilih salah satu: {}", SEVERITIES.join(", ")),
            ));
        }
    }

    // Batas laju: 5 laporan per jam. Cukup longgar untuk keadaan darurat
    // yang sebenarnya, cukup ketat untuk menghentikan spam yang akan
    // menenggelamkan laporan sungguhan.
    state
        .rate_limit(
            &format!("panic:{}", user.id),
            5,
            std::time::Duration::from_secs(3600),
        )
        .await?;

    // Sekolah diambil dari akun, TIDAK dari input. Tanpa ini, seorang siswa
    // bisa mengarang laporan atas nama sekolah lain.
    let school_id = resolve_reporter_school(&state, &user).await?;

    let category: (String, String) =
        sqlx::query_as("SELECT code, default_severity FROM panic_categories WHERE id = $1 AND is_active")
            .bind(body.category_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| ApiError::field("category_id", "kategori tidak dikenal"))?;

    let severity = body.severity.clone().unwrap_or(category.1);
    let author_role = user.roles.first().cloned().unwrap_or_else(|| "warga".into());
    let handle = anonymity::report_handle(&author_role);

    // Laporan mendesak melewati antrean moderasi: menahan laporan kekerasan
    // sampai ada petugas yang sempat memeriksanya bisa berarti terlambat.
    // Yang ditahan hanyalah tampilnya di feed publik, bukan penanganannya.
    let urgent = URGENT_SEVERITIES.contains(&severity.as_str());
    let moderation_status = if urgent { "approved" } else { "pending" };

    let mut tx = state.db.begin().await?;

    let (id, created_at): (Uuid, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        r#"
        INSERT INTO panic_reports (
            school_id, category_id, author_user_id, author_role, anonymous_handle,
            title, body, severity, visibility, moderation_status, status
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'baru')
        RETURNING id, created_at
        "#,
    )
    .bind(school_id)
    .bind(body.category_id)
    .bind(user.id)
    .bind(&author_role)
    .bind(&handle)
    .bind(body.title.trim())
    .bind(body.body.trim())
    .bind(&severity)
    .bind(&body.visibility)
    .bind(moderation_status)
    .fetch_one(&mut *tx)
    .await?;

    // Lampiran: gambar di-decode ulang sehingga metadata EXIF — termasuk
    // koordinat GPS — hilang. Tanpa langkah ini, foto bukti bisa
    // membocorkan lokasi pelapor dan mematahkan seluruh anonimitas.
    for (idx, raw) in body.media_base64.iter().enumerate() {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(strip_data_uri(raw))
            .map_err(|_| {
                ApiError::field("media_base64", &format!("lampiran ke-{} bukan base64 valid", idx + 1))
            })?;

        if bytes.len() > state.cfg.max_upload_bytes {
            return Err(ApiError::field(
                "media_base64",
                &format!("lampiran ke-{} melebihi batas ukuran", idx + 1),
            ));
        }
        quality::sniff_mime(&bytes)?;

        let sanitized = strip_metadata(&bytes)?;
        let key = format!(
            "panic/{}/{}.jpg",
            created_at.format("%Y%m"),
            Uuid::new_v4()
        );
        state.storage.put(&key, &sanitized).await?;

        sqlx::query(
            r#"
            INSERT INTO panic_report_media
                (report_id, report_created_at, file_key, mime_type, bytes, exif_stripped)
            VALUES ($1,$2,$3,'image/jpeg',$4,TRUE)
            "#,
        )
        .bind(id)
        .bind(created_at)
        .bind(&key)
        .bind(sanitized.len() as i32)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        r#"
        INSERT INTO panic_report_events (report_id, report_created_at, status, note, is_public)
        VALUES ($1,$2,'baru',$3,TRUE)
        "#,
    )
    .bind(id)
    .bind(created_at)
    .bind(if urgent {
        "Laporan diterima dan ditandai mendesak. Diteruskan langsung ke Dinas Pendidikan."
    } else {
        "Laporan diterima dan sedang diperiksa petugas sebelum tampil di beranda."
    })
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    // Audit TIDAK memuat isi laporan maupun identitas pelapor — mencatat
    // keduanya di sini akan membuat jejak audit menjadi pintu belakang
    // menuju identitas yang seharusnya terlindungi.
    AuditEntry::by_system("panic.report_created")
        .school(school_id)
        .entity("panic_report", id)
        .after(&serde_json::json!({
            "severity": severity,
            "category": category.0,
            "visibility": body.visibility,
        }))
        .write(&state.db)
        .await;

    let message = if urgent {
        "Laporan Anda diterima dan ditandai MENDESAK. Petugas Dinas Pendidikan \
         akan segera menindaklanjuti. Identitas Anda tidak ditampilkan kepada \
         pihak sekolah."
            .to_string()
    } else {
        "Laporan Anda diterima. Setelah diperiksa petugas, laporan akan tampil \
         di beranda secara anonim. Anda dapat memantau perkembangannya di menu \
         \"Laporan Saya\"."
            .to_string()
    };

    Ok(ApiResponse::with_message(
        CreateReportResponse {
            id,
            anonymous_handle: handle,
            severity,
            moderation_status: moderation_status.to_string(),
            message: message.clone(),
        },
        message,
    ))
}

/// Tandai / batalkan "saya juga mengalami hal serupa".
#[utoipa::path(
    post, path = "/v1/panic/reports/{id}/support", tag = "Panic Button",
    params(("id" = Uuid, Path, description = "ID laporan")),
    security(("bearer" = [])),
    responses((status = 200, description = "Status dukungan diperbarui"))
)]
pub async fn toggle_support(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require("view_panic_feed")?;
    let view = ViewerScope::of(&user);
    fetch_report(&state, &user, &view, id).await?;

    let removed = sqlx::query("DELETE FROM panic_supports WHERE report_id = $1 AND user_id = $2")
        .bind(id)
        .bind(user.id)
        .execute(&state.db)
        .await?
        .rows_affected();

    let supported = if removed == 0 {
        sqlx::query("INSERT INTO panic_supports (report_id, user_id) VALUES ($1,$2)")
            .bind(id)
            .bind(user.id)
            .execute(&state.db)
            .await?;
        true
    } else {
        false
    };

    let (count,): (i16,) =
        sqlx::query_as("SELECT support_count FROM panic_reports WHERE id = $1")
            .bind(id)
            .fetch_one(&state.db)
            .await?;

    Ok(ApiResponse::new(serde_json::json!({
        "supported": supported,
        "support_count": count,
    })))
}

/// Tambah komentar pada laporan.
#[utoipa::path(
    post, path = "/v1/panic/reports/{id}/comments", tag = "Panic Button",
    params(("id" = Uuid, Path, description = "ID laporan")),
    request_body = CreateCommentRequest,
    security(("bearer" = [])),
    responses((status = 200, description = "Komentar terkirim", body = PanicComment))
)]
pub async fn add_comment(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ValidJson(body): ValidJson<CreateCommentRequest>,
) -> ApiResult<ApiResponse<PanicComment>> {
    user.require("comment_panic_report")?;
    let view = ViewerScope::of(&user);
    let report = fetch_report(&state, &user, &view, id).await?;

    state
        .rate_limit(
            &format!("panic_comment:{}", user.id),
            20,
            std::time::Duration::from_secs(600),
        )
        .await?;

    // Komentar resmi hanya untuk petugas — kalau tidak, siapa pun bisa
    // menyamar sebagai pihak berwenang dan memberi janji palsu kepada pelapor.
    let as_official = body.as_official && user.has_permission("handle_panic_report");

    let author_role = user.roles.first().cloned().unwrap_or_else(|| "warga".into());
    let handle = anonymity::comment_handle(
        state.cfg.jwt_secret.as_bytes(),
        id,
        user.id,
        &author_role,
    );

    let (comment_id, created_at): (Uuid, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        r#"
        INSERT INTO panic_comments (
            report_id, report_created_at, author_user_id, anonymous_handle,
            is_official, official_name, official_title, body, moderation_status
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'approved')
        RETURNING id, created_at
        "#,
    )
    .bind(id)
    .bind(report.created_at)
    .bind(user.id)
    .bind(&handle)
    .bind(as_official)
    .bind(as_official.then(|| user.name.clone()))
    .bind(as_official.then(|| user.role_label().to_string()))
    .bind(body.body.trim())
    .fetch_one(&state.db)
    .await?;

    Ok(ApiResponse::with_message(
        PanicComment {
            id: comment_id,
            anonymous_handle: (!as_official).then_some(handle),
            is_official: as_official,
            official_name: as_official.then(|| user.name.clone()),
            official_title: as_official.then(|| user.role_label().to_string()),
            body: body.body.trim().to_string(),
            is_mine: true,
            created_at,
        },
        "Komentar terkirim",
    ))
}

/// Setujui atau tolak tampilnya laporan di beranda.
#[utoipa::path(
    post, path = "/v1/panic/reports/{id}/moderate", tag = "Panic Button",
    params(("id" = Uuid, Path, description = "ID laporan")),
    request_body = ModerateReportRequest,
    security(("bearer" = [])),
    responses((status = 200, description = "Moderasi disimpan"))
)]
pub async fn moderate(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ValidJson(body): ValidJson<ModerateReportRequest>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require("moderate_panic_report")?;

    if !MODERATION_STATUSES.contains(&body.moderation_status.as_str()) {
        return Err(ApiError::field(
            "moderation_status",
            "pilih `approved` atau `rejected`",
        ));
    }

    let affected = sqlx::query(
        r#"
        UPDATE panic_reports
           SET moderation_status = $2, moderation_note = $3,
               moderated_by = $4, moderated_at = NOW(), updated_at = NOW()
         WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(&body.moderation_status)
    .bind(body.note.as_deref())
    .bind(user.id)
    .execute(&state.db)
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(ApiError::NotFound(format!("laporan `{id}`")));
    }

    AuditEntry::by_user(&user, "panic.moderate")
        .entity("panic_report", id)
        .after(&serde_json::json!({ "moderation_status": body.moderation_status }))
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        serde_json::json!({ "moderation_status": body.moderation_status }),
        "Status moderasi disimpan",
    ))
}

/// Perbarui status penanganan laporan.
#[utoipa::path(
    post, path = "/v1/panic/reports/{id}/status", tag = "Panic Button",
    params(("id" = Uuid, Path, description = "ID laporan")),
    request_body = UpdateReportStatusRequest,
    security(("bearer" = [])),
    responses((status = 200, description = "Status diperbarui"))
)]
pub async fn update_status(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ValidJson(body): ValidJson<UpdateReportStatusRequest>,
) -> ApiResult<ApiResponse<serde_json::Value>> {
    user.require("handle_panic_report")?;

    if !STATUSES.contains(&body.status.as_str()) {
        return Err(ApiError::field(
            "status",
            &format!("pilih salah satu: {}", STATUSES.join(", ")),
        ));
    }
    if body.status == "selesai" && body.resolution.as_deref().unwrap_or("").trim().len() < 10 {
        return Err(ApiError::field(
            "resolution",
            "hasil penanganan wajib dijelaskan sebelum laporan ditutup",
        ));
    }

    let view = ViewerScope::of(&user);
    let report = fetch_report(&state, &user, &view, id).await?;

    let mut tx = state.db.begin().await?;

    sqlx::query(
        r#"
        UPDATE panic_reports
           SET status = $2,
               handled_by = COALESCE(handled_by, $3),
               handled_at = COALESCE(handled_at, NOW()),
               resolution = COALESCE($4, resolution),
               resolved_at = CASE WHEN $2 IN ('selesai','ditolak') THEN NOW() ELSE resolved_at END,
               updated_at = NOW()
         WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(&body.status)
    .bind(user.id)
    .bind(body.resolution.as_deref())
    .execute(&mut *tx)
    .await?;

    // Lini masa inilah yang membuat pelapor tahu laporannya tidak diabaikan.
    // Pelapor yang tidak pernah tahu tindak lanjutnya akan berhenti melapor.
    sqlx::query(
        r#"
        INSERT INTO panic_report_events
            (report_id, report_created_at, status, note, actor_user_id, actor_label, is_public)
        VALUES ($1,$2,$3,$4,$5,$6,$7)
        "#,
    )
    .bind(id)
    .bind(report.created_at)
    .bind(&body.status)
    .bind(body.note.trim())
    .bind(user.id)
    .bind(format!("{} ({})", user.name, user.role_label()))
    .bind(body.visible_to_reporter)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    AuditEntry::by_user(&user, "panic.status_update")
        .entity("panic_report", id)
        .after(&serde_json::json!({ "status": body.status }))
        .write(&state.db)
        .await;

    Ok(ApiResponse::with_message(
        serde_json::json!({ "status": body.status }),
        "Status penanganan diperbarui",
    ))
}

/// **Buka identitas pelapor.**
///
/// Satu-satunya jalan mengetahui siapa penulis sebuah laporan. Dijaga izin
/// `unmask_panic_report`, mewajibkan alasan tertulis, dan setiap pemakaiannya
/// dicatat permanen di `panic_unmask_logs` — catatan itu tidak dapat dihapus
/// lewat API mana pun.
#[utoipa::path(
    post, path = "/v1/panic/reports/{id}/unmask", tag = "Panic Button",
    params(("id" = Uuid, Path, description = "ID laporan")),
    request_body = UnmaskRequest,
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Identitas pelapor", body = UnmaskedAuthor),
        (status = 403, description = "Tidak memiliki izin membuka identitas")
    )
)]
pub async fn unmask(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ValidJson(body): ValidJson<UnmaskRequest>,
) -> ApiResult<ApiResponse<UnmaskedAuthor>> {
    user.require("unmask_panic_report")?;

    // Sengaja tidak memakai `is_superadmin()` sebagai jalan pintas: izin ini
    // harus diberikan secara sadar, bukan diwarisi karena kebetulan berperan
    // superadmin.
    if !user.permissions.contains("unmask_panic_report") {
        return Err(ApiError::Forbidden(
            "Membuka identitas pelapor memerlukan izin `unmask_panic_report` \
             yang diberikan secara eksplisit."
                .into(),
        ));
    }

    #[derive(sqlx::FromRow)]
    struct AuthorRow {
        user_id: Uuid,
        name: String,
        identity_number: Option<String>,
        role: String,
        school_name: Option<String>,
    }

    let row: Option<AuthorRow> = sqlx::query_as(
        r#"
        SELECT u.id AS user_id, u.name, u.identity_number,
               r.author_role AS role, s.name AS school_name
        FROM panic_reports r
        JOIN users u   ON u.id = r.author_user_id
        LEFT JOIN schools s ON s.id = u.school_id
        WHERE r.id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    let author = row.ok_or_else(|| ApiError::NotFound(format!("laporan `{id}`")))?;

    // Catat SEBELUM mengembalikan data. Bila pencatatan gagal, identitas
    // tidak jadi dibuka — jejak audit tidak boleh opsional di sini.
    sqlx::query(
        r#"
        INSERT INTO panic_unmask_logs (report_id, actor_user_id, actor_label, reason)
        VALUES ($1,$2,$3,$4)
        "#,
    )
    .bind(id)
    .bind(user.id)
    .bind(format!("{} ({})", user.name, user.username))
    .bind(body.reason.trim())
    .execute(&state.db)
    .await?;

    AuditEntry::by_user(&user, "panic.unmask")
        .entity("panic_report", id)
        .after(&serde_json::json!({ "reason": body.reason.trim() }))
        .write(&state.db)
        .await;

    tracing::warn!(
        report_id = %id,
        actor = %user.username,
        "identitas pelapor Panic Button dibuka"
    );

    Ok(ApiResponse::new(UnmaskedAuthor {
        report_id: id,
        user_id: author.user_id,
        name: author.name,
        identity_number: author.identity_number,
        role: author.role,
        school_name: author.school_name,
        notice: "Pembukaan identitas ini telah dicatat permanen beserta nama dan \
                 alasan Anda. Data pelapor hanya boleh dipakai untuk keperluan \
                 yang disebutkan."
            .into(),
    }))
}

/// Statistik pengaduan untuk dashboard penanganan.
#[utoipa::path(
    get, path = "/v1/panic/stats", tag = "Panic Button",
    security(("bearer" = [])),
    responses((status = 200, description = "Statistik pengaduan", body = PanicStats))
)]
pub async fn stats(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<ApiResponse<PanicStats>> {
    user.require_any(&["handle_panic_report", "moderate_panic_report"])?;
    let view = ViewerScope::of(&user);

    let row: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT COUNT(*)::bigint,
               COUNT(*) FILTER (WHERE status = 'baru')::bigint,
               COUNT(*) FILTER (WHERE status = 'ditindaklanjuti')::bigint,
               COUNT(*) FILTER (WHERE status = 'selesai')::bigint,
               COUNT(*) FILTER (WHERE moderation_status = 'pending')::bigint,
               COUNT(*) FILTER (WHERE severity = 'darurat' AND handled_at IS NULL)::bigint
        FROM panic_reports
        WHERE created_at > NOW() - INTERVAL '180 days'
          AND ($1::uuid[] IS NULL OR school_id = ANY($1))
        "#,
    )
    .bind(view.school_ids.as_deref())
    .fetch_one(&state.db)
    .await?;

    let per_kategori: Vec<CategoryCount> = sqlx::query_as(
        r#"
        SELECT c.name AS category_name, COUNT(*)::bigint AS total
        FROM panic_reports r
        JOIN panic_categories c ON c.id = r.category_id
        WHERE r.created_at > NOW() - INTERVAL '180 days'
          AND ($1::uuid[] IS NULL OR r.school_id = ANY($1))
        GROUP BY c.name
        ORDER BY total DESC
        "#,
    )
    .bind(view.school_ids.as_deref())
    .fetch_all(&state.db)
    .await?;

    Ok(ApiResponse::new(PanicStats {
        total: row.0,
        baru: row.1,
        ditindaklanjuti: row.2,
        selesai: row.3,
        menunggu_moderasi: row.4,
        darurat_belum_ditangani: row.5,
        per_kategori,
    }))
}

// =====================================================================
// Cakupan penglihatan
// =====================================================================

/// Apa yang boleh dilihat seorang pengguna pada Panic Button.
struct ViewerScope {
    /// Peran yang menangani pengaduan di tingkat provinsi.
    is_handler: bool,
    /// Peran tingkat sekolah yang boleh melihat laporan sekolahnya.
    is_school_handler: bool,
    /// Batasan sekolah; `None` = seluruh provinsi.
    school_ids: Option<Vec<Uuid>>,
}

impl ViewerScope {
    fn of(user: &AuthUser) -> Self {
        let is_handler = user
            .roles
            .iter()
            .any(|r| HANDLER_ROLES.contains(&r.as_str()));

        // Kepala sekolah boleh menangani laporan sekolahnya sendiri, tetapi
        // hanya untuk kategori yang tidak menyangkut dirinya (lihat
        // SCHOOL_HIDDEN_CATEGORIES).
        let is_school_handler = !is_handler && user.has_permission("handle_panic_report");

        Self {
            is_handler,
            is_school_handler,
            school_ids: if is_handler {
                None
            } else {
                user.accessible_schools()
            },
        }
    }

    /// Nama sekolah ditampilkan asli hanya kepada yang menangani.
    fn shows_real_school(&self) -> bool {
        self.is_handler || self.is_school_handler
    }
}

/// Bangun klausa WHERE feed sesuai cakupan.
///
/// Parameter: $1 user_id, $2 school_ids, $3 category_code, $4 severity,
/// $5 status.
fn build_feed_filter(view: &ViewerScope, mine_only: bool, pending_only: bool) -> String {
    let mut clauses = vec![
        "($3::text IS NULL OR c.code = $3)".to_string(),
        "($4::text IS NULL OR r.severity = $4)".to_string(),
        "($5::text IS NULL OR r.status = $5)".to_string(),
    ];

    if mine_only {
        // Pelapor selalu bisa melihat laporannya sendiri, apa pun status
        // moderasinya — kalau tidak, ia akan mengira laporannya hilang.
        clauses.push("r.author_user_id = $1".to_string());
        return clauses.join(" AND ");
    }

    if pending_only {
        clauses.push("r.moderation_status = 'pending'".to_string());
    }

    if view.is_handler {
        // Petugas provinsi melihat semuanya, termasuk laporan `terbatas`.
        // Klausa sekolah tetap dipasang: nilainya NULL kecuali petugas
        // sengaja mempersempit ke satu sekolah lewat parameter.
        clauses.push("($2::uuid[] IS NULL OR r.school_id = ANY($2))".to_string());
        return clauses.join(" AND ");
    }

    if view.is_school_handler {
        // Kepala sekolah: laporan sekolahnya, TANPA kategori sensitif yang
        // lazim menyangkut pihak sekolah sendiri.
        clauses.push("($2::uuid[] IS NULL OR r.school_id = ANY($2))".to_string());
        clauses.push(format!(
            "c.code NOT IN ('{}')",
            SCHOOL_HIDDEN_CATEGORIES.join("','")
        ));
        clauses.push("r.visibility = 'publik'".to_string());
        clauses.push("(r.moderation_status = 'approved' OR r.author_user_id = $1)".to_string());
        return clauses.join(" AND ");
    }

    // Warga biasa: feed provinsi, hanya yang sudah dimoderasi dan publik,
    // ditambah laporannya sendiri.
    clauses.push(
        "((r.moderation_status = 'approved' AND r.visibility = 'publik') \
          OR r.author_user_id = $1)"
            .to_string(),
    );
    clauses.join(" AND ")
}

/// Pastikan pengguna boleh membuka lampiran sebuah laporan.
///
/// Dipakai `routes::files` supaya lampiran mengikuti visibilitas laporannya
/// tanpa menyalin aturannya. Menyalin akan menghasilkan pintu belakang: foto
/// laporan kekerasan tetap terbuka lewat `/files` bagi kepala sekolah yang
/// laporannya sendiri disembunyikan — dan foto justru yang paling mudah
/// mengidentifikasi pelapor.
pub(crate) async fn authorize_media(
    state: &AppState,
    user: &AuthUser,
    report_id: Uuid,
) -> ApiResult<()> {
    let view = ViewerScope::of(user);
    fetch_report(state, user, &view, report_id).await.map(|_| ())
}

async fn fetch_report(
    state: &AppState,
    user: &AuthUser,
    view: &ViewerScope,
    id: Uuid,
) -> ApiResult<FeedRow> {
    let where_sql = build_feed_filter(view, false, false);

    let row: Option<FeedRow> = sqlx::query_as(&format!(
        r#"
        SELECT r.id, r.created_at, r.school_id, s.name AS school_name,
               s.jenjang AS school_jenjang,
               c.code AS category_code, c.name AS category_name, c.icon AS category_icon,
               r.anonymous_handle, r.author_role, r.title, r.body,
               r.severity, r.status, r.support_count, r.comment_count,
               (r.author_user_id = $1) AS is_mine,
               EXISTS (
                   SELECT 1 FROM panic_supports ps
                   WHERE ps.report_id = r.id AND ps.user_id = $1
               ) AS is_supported,
               r.handled_at, r.resolved_at
        FROM panic_reports r
        JOIN panic_categories c ON c.id = r.category_id
        JOIN schools s          ON s.id = r.school_id
        WHERE r.id = $6 AND {where_sql}
        "#
    ))
    .bind(user.id)
    .bind(view.school_ids.as_deref())
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    // 404 (bukan 403) untuk laporan di luar cakupan: membedakan keduanya akan
    // memberi tahu penanya bahwa laporan itu ADA, yang sudah merupakan
    // kebocoran informasi tersendiri.
    row.ok_or_else(|| ApiError::NotFound(format!("laporan `{id}`")))
}

async fn load_media_map(
    state: &AppState,
    report_ids: Vec<Uuid>,
) -> ApiResult<std::collections::HashMap<Uuid, Vec<String>>> {
    let mut map: std::collections::HashMap<Uuid, Vec<String>> = std::collections::HashMap::new();
    if report_ids.is_empty() {
        return Ok(map);
    }

    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT report_id, file_key FROM panic_report_media
         WHERE report_id = ANY($1) ORDER BY created_at",
    )
    .bind(&report_ids)
    .fetch_all(&state.db)
    .await?;

    for (report_id, key) in rows {
        map.entry(report_id)
            .or_default()
            .push(state.storage.public_url(&key));
    }
    Ok(map)
}

fn to_feed_item(
    _state: &AppState,
    row: FeedRow,
    view: &ViewerScope,
    media: &std::collections::HashMap<Uuid, Vec<String>>,
) -> PanicFeedItem {
    let school_label = if view.shows_real_school() || row.is_mine {
        row.school_name.clone()
    } else {
        anonymity::blur_school_name(&row.school_name, &row.school_jenjang)
    };

    PanicFeedItem {
        media: media.get(&row.id).cloned().unwrap_or_default(),
        // Id sekolah hanya untuk yang menangani; membocorkannya ke warga biasa
        // akan membatalkan penyamaran nama sekolah di atas.
        school_id: view.shows_real_school().then_some(row.school_id),
        id: row.id,
        created_at: row.created_at,
        category_code: row.category_code,
        category_name: row.category_name,
        category_icon: row.category_icon,
        anonymous_handle: row.anonymous_handle,
        author_role: row.author_role,
        school_label,
        title: row.title,
        body: row.body,
        severity: row.severity,
        status: row.status,
        support_count: row.support_count,
        comment_count: row.comment_count,
        is_mine: row.is_mine,
        is_supported: row.is_supported,
        handled_at: row.handled_at,
        resolved_at: row.resolved_at,
    }
}

/// Sekolah yang menjadi konteks laporan, diambil dari akun pelapor.
async fn resolve_reporter_school(state: &AppState, user: &AuthUser) -> ApiResult<Uuid> {
    // Guru/staff/kepsek: sekolah tempatnya bertugas.
    if let Some(school_id) = user.school_id {
        return Ok(school_id);
    }

    // Siswa/orang tua: sekolah anak. Untuk orang tua dengan anak di beberapa
    // sekolah, dipakai sekolah anak pertama — API mengembalikan daftar anak
    // pada profil, sehingga aplikasi bisa menyediakan pilihan bila perlu.
    if let Some(students) = user.accessible_students() {
        if let Some(first) = students.first() {
            let row: Option<(Uuid,)> =
                sqlx::query_as("SELECT school_id FROM students WHERE id = $1")
                    .bind(first)
                    .fetch_optional(&state.db)
                    .await?;
            if let Some((school_id,)) = row {
                return Ok(school_id);
            }
        }
    }

    Err(ApiError::Forbidden(
        "Akun Anda belum tertaut ke sekolah mana pun sehingga laporan tidak \
         dapat diarahkan. Hubungi operator sekolah."
            .into(),
    ))
}

/// Buang seluruh metadata gambar dengan cara men-decode dan meng-encode ulang.
///
/// Foto dari ponsel membawa EXIF yang lazim memuat koordinat GPS, merek
/// perangkat, dan waktu pengambilan. Pada kanal anonim, ketiganya cukup untuk
/// mempersempit siapa pelapornya.
fn strip_metadata(bytes: &[u8]) -> ApiResult<Vec<u8>> {
    let img = image::load_from_memory(bytes)
        .map_err(|_| ApiError::field("media_base64", "lampiran bukan gambar yang valid"))?;

    // Skala turun bila terlalu besar: menghemat penyimpanan sekaligus
    // menghilangkan detail latar yang bisa mengidentifikasi lokasi.
    let resized = if img.width() > 1600 || img.height() > 1600 {
        img.resize(1600, 1600, image::imageops::FilterType::Triangle)
    } else {
        img
    };

    let mut out = std::io::Cursor::new(Vec::new());
    resized
        .to_rgb8()
        .write_to(&mut out, image::ImageFormat::Jpeg)
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("gagal memproses lampiran: {e}")))?;

    Ok(out.into_inner())
}

fn strip_data_uri(input: &str) -> &str {
    let trimmed = input.trim();
    match trimmed.find(";base64,") {
        Some(idx) if trimmed.starts_with("data:") => &trimmed[idx + 8..],
        _ => trimmed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn user(roles: &[&str], perms: &[&str], school: Option<Uuid>) -> AuthUser {
        AuthUser {
            id: Uuid::new_v4(),
            username: "uji".into(),
            name: "Uji".into(),
            identity: None,
            school_id: school,
            extra_schools: vec![],
            students: vec![],
            roles: roles.iter().map(|s| s.to_string()).collect(),
            permissions: perms.iter().map(|s| s.to_string()).collect::<HashSet<_>>(),
        }
    }

    #[test]
    fn siswa_melihat_feed_provinsi_dengan_sekolah_disamarkan() {
        let u = user(&["siswa"], &["view_panic_feed"], Some(Uuid::new_v4()));
        let v = ViewerScope::of(&u);
        assert!(!v.is_handler);
        assert!(!v.shows_real_school());
    }

    #[test]
    fn petugas_provinsi_melihat_semua_tanpa_batas_sekolah() {
        let u = user(
            &["petugas_pengaduan"],
            &["view_panic_feed", "handle_panic_report"],
            None,
        );
        let v = ViewerScope::of(&u);
        assert!(v.is_handler);
        assert!(v.school_ids.is_none(), "petugas provinsi tidak dibatasi sekolah");
        assert!(v.shows_real_school());
    }

    #[test]
    fn kepala_sekolah_tidak_melihat_kategori_sensitif() {
        // Inti perlindungan pelapor: laporan kekerasan, pelecehan, dan pungli
        // tidak boleh muncul di layar pihak yang mungkin justru terlapor.
        let u = user(
            &["kepala_sekolah"],
            &["view_panic_feed", "handle_panic_report"],
            Some(Uuid::new_v4()),
        );
        let v = ViewerScope::of(&u);
        assert!(v.is_school_handler);

        let sql = build_feed_filter(&v, false, false);
        for kategori in SCHOOL_HIDDEN_CATEGORIES {
            assert!(
                sql.contains(kategori),
                "kategori `{kategori}` harus disaring dari feed kepala sekolah"
            );
        }
        assert!(sql.contains("NOT IN"));
    }

    #[test]
    fn petugas_provinsi_tidak_menyaring_kategori() {
        let u = user(&["admin_dinas"], &["view_panic_feed"], None);
        let v = ViewerScope::of(&u);
        let sql = build_feed_filter(&v, false, false);
        assert!(
            !sql.contains("NOT IN"),
            "Dinas harus melihat semua kategori, termasuk yang paling sensitif"
        );
        assert!(
            sql.contains("school_id = ANY($2)"),
            "petugas tetap bisa mempersempit ke satu sekolah"
        );
    }

    #[test]
    fn id_sekolah_hanya_untuk_yang_menangani() {
        // Mengembalikan school_id ke warga biasa akan membatalkan penyamaran
        // nama sekolah, karena id itu bisa dicocokkan ke daftar sekolah publik.
        let warga = ViewerScope::of(&user(&["siswa"], &["view_panic_feed"], Some(Uuid::new_v4())));
        assert!(!warga.shows_real_school());

        let petugas = ViewerScope::of(&user(
            &["petugas_pengaduan"],
            &["view_panic_feed", "handle_panic_report"],
            None,
        ));
        assert!(petugas.shows_real_school());
    }

    #[test]
    fn pelapor_selalu_melihat_laporannya_sendiri() {
        // Termasuk yang masih menunggu moderasi — kalau tidak, pelapor akan
        // mengira laporannya hilang dan mengirim ulang berkali-kali.
        let u = user(&["siswa"], &["view_panic_feed"], Some(Uuid::new_v4()));
        let v = ViewerScope::of(&u);

        let mine = build_feed_filter(&v, true, false);
        assert!(mine.contains("r.author_user_id = $1"));
        assert!(!mine.contains("moderation_status = 'approved'"));

        let umum = build_feed_filter(&v, false, false);
        assert!(umum.contains("r.author_user_id = $1"));
    }

    #[test]
    fn warga_biasa_tidak_melihat_laporan_terbatas() {
        let u = user(&["guru"], &["view_panic_feed"], Some(Uuid::new_v4()));
        let v = ViewerScope::of(&u);
        let sql = build_feed_filter(&v, false, false);
        assert!(sql.contains("visibility = 'publik'"));
    }

    #[test]
    fn data_uri_dibuang_dari_lampiran() {
        assert_eq!(strip_data_uri("data:image/jpeg;base64,QUJD"), "QUJD");
        assert_eq!(strip_data_uri("  QUJD  "), "QUJD");
    }

    #[test]
    fn strip_metadata_menghasilkan_jpeg_tanpa_exif() {
        // Gambar sumber dibuat sebagai PNG; keluarannya harus JPEG hasil
        // encode ulang, yang menjamin tidak ada blok EXIF yang terbawa.
        let mut img = image::RgbImage::new(64, 64);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgb([(x % 256) as u8, (y % 256) as u8, 128]);
        }
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();

        let out = strip_metadata(&png.into_inner()).unwrap();
        assert!(out.starts_with(&[0xFF, 0xD8, 0xFF]), "keluaran harus JPEG");
    }

    #[test]
    fn strip_metadata_menolak_berkas_bukan_gambar() {
        assert!(strip_metadata(b"ini bukan gambar sama sekali").is_err());
    }
}
