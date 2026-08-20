//! Spesifikasi OpenAPI 3.1 — sumber tunggal dokumentasi API.
//!
//! Dihasilkan dari anotasi `#[utoipa::path]` pada tiap handler, sehingga
//! dokumentasi tidak bisa "menua" tanpa kode ikut berubah. Disajikan di
//! `/docs` (Swagger UI) dan `/api-docs/openapi.json`.

use utoipa::openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Jargon GO API",
        version = "1.0.0",
        description = r#"
API absensi siswa berbasis pengenalan wajah untuk seluruh sekolah di bawah
**Dinas Pendidikan Provinsi Sumatera Utara** (700.000+ siswa).

## Bentuk respons

Semua respons sukses memakai envelope yang sama:

```json
{ "success": true, "data": <isi>, "message": "opsional" }
```

Endpoint berdaftar juga menyertakan `meta`:

```json
{ "success": true, "data": [ ... ], "meta": { "page": 1, "per_page": 25, "total": 431, "total_pages": 18 } }
```

Semua error memakai bentuk:

```json
{ "success": false, "code": "validation_error", "message": "Data yang dikirim tidak valid.",
  "errors": [ { "field": "nisn", "message": "NISN harus 10 digit" } ] }
```

Pada skema di bawah, `body` menunjuk ke tipe **isi** `data` — bukan envelope-nya.

## Tiga jenis kredensial

| Pemanggil | Header | Contoh |
|---|---|---|
| Pengguna (guru/kepsek/staff/dinas) | `Authorization: Bearer <jwt>` | dashboard, aplikasi guru |
| Tablet kios | `Authorization: Device <device_token>` | absensi harian |
| Layanan internal | `X-Api-Key` + `X-Api-Secret` | dashboard Laravel -> API |

## Cakupan data (multi-tenant)

Tenant adalah **sekolah**. Guru, staff, dan kepala sekolah hanya dapat
mengakses data sekolahnya; menyebut `school_id` sekolah lain akan menghasilkan
`403`. Peran `superadmin` dan `admin_dinas` bercakupan provinsi: bila
`school_id` dikosongkan, data seluruh provinsi yang dikembalikan.

## Privasi data biometrik

* **Gambar wajah** hanya dikirim & disimpan pada saat pendaftaran
  (`POST /v1/students/{id}/face`).
* **Absensi harian** (`POST /v1/kiosk/recognize`) hanya mengirim vektor
  embedding; server memakainya untuk mencocokkan lalu membuangnya.
* Yang tersimpan pada absensi: id & nama siswa, id & nama kelas, id & nama
  sekolah, jam masuk, jam pulang, dan status.
* Menghapus siswa memusnahkan seluruh gambar dan vektornya.
"#,
        contact(
            name = "Dinas Pendidikan Provinsi Sumatera Utara",
            url = "https://disdik.sumutprov.go.id"
        ),
        license(name = "Proprietary")
    ),
    servers(
        (url = "/", description = "Server saat ini"),
        (url = "http://localhost:8080", description = "Pengembangan lokal")
    ),
    tags(
        (name = "Sistem", description = "Kesehatan & status layanan"),
        (name = "Autentikasi", description = "Login, refresh token, profil"),
        (name = "Dashboard", description = "Angka ringkas untuk layar utama"),
        (name = "Sekolah", description = "Master sekolah, wilayah, tahun ajaran"),
        (name = "Kelas", description = "Rombongan belajar"),
        (name = "Siswa", description = "Data siswa & wali murid"),
        (name = "Biometrik", description = "Pendaftaran wajah siswa"),
        (name = "Absensi", description = "Monitoring, koreksi, rekap, aturan jam"),
        (name = "Perangkat", description = "Tablet kios & pairing"),
        (name = "Kios", description = "Endpoint yang dipanggil tablet"),
        (name = "Notifikasi", description = "Pesan ke wali murid (WA/Telegram/Email)"),
        (name = "Pengguna", description = "Akun guru, staff, kepala sekolah, dinas"),
        (name = "Berkas", description = "Foto pendaftaran & hasil ekspor"),
        (name = "Jargon GO", description = "Beranda & data milik pengguna aplikasi"),
        (name = "Panic Button", description = "Kanal pengaduan anonim warga sekolah"),
        (name = "Pemberkasan", description = "Unggah & verifikasi berkas kepegawaian")
    ),
    paths(
        // Sistem
        crate::routes::health::health,
        crate::routes::health::live,
        crate::routes::health::ready,
        // Autentikasi
        crate::routes::auth::login,
        crate::routes::auth::refresh,
        crate::routes::auth::logout,
        crate::routes::auth::me,
        crate::routes::auth::change_password,
        // Dashboard
        crate::routes::dashboard::dashboard,
        crate::routes::dashboard::province,
        crate::routes::dashboard::live_feed,
        // Sekolah
        crate::routes::schools::list,
        crate::routes::schools::detail,
        crate::routes::schools::create,
        crate::routes::schools::update,
        crate::routes::schools::soft_delete,
        crate::routes::schools::restore,
        crate::routes::schools::list_regions,
        crate::routes::schools::list_academic_years,
        // Kelas
        crate::routes::classrooms::list,
        crate::routes::classrooms::detail,
        crate::routes::classrooms::create,
        crate::routes::classrooms::update,
        crate::routes::classrooms::remove,
        // Siswa & biometrik
        crate::routes::students::list,
        crate::routes::students::detail,
        crate::routes::students::create,
        crate::routes::students::update,
        crate::routes::students::remove,
        crate::routes::students::list_guardians,
        crate::routes::students::add_guardian,
        crate::routes::students::update_guardian,
        crate::routes::students::delete_guardian,
        crate::routes::students::enroll_face,
        crate::routes::students::enroll_face_kiosk,
        crate::routes::students::list_face_samples,
        crate::routes::students::list_enrollments,
        crate::routes::students::delete_enrollment,
        // Absensi
        crate::routes::attendance::list,
        crate::routes::attendance::student_history,
        crate::routes::attendance::manual,
        crate::routes::attendance::bulk,
        crate::routes::attendance::summary,
        crate::routes::attendance::by_classroom,
        crate::routes::attendance::recap,
        crate::routes::attendance::list_rules,
        crate::routes::attendance::upsert_rule,
        // Perangkat & kios
        crate::routes::devices::list,
        crate::routes::devices::detail,
        crate::routes::devices::create,
        crate::routes::devices::update,
        crate::routes::devices::remove,
        crate::routes::devices::regenerate_pairing,
        crate::routes::devices::revoke,
        crate::routes::devices::pair,
        crate::routes::kiosk::recognize,
        crate::routes::kiosk::heartbeat,
        crate::routes::kiosk::roster,
        crate::routes::kiosk::config,
        // Notifikasi
        crate::routes::notifications::list_templates,
        crate::routes::notifications::upsert_template,
        crate::routes::notifications::get_policy,
        crate::routes::notifications::update_policy,
        crate::routes::notifications::list_outbox,
        crate::routes::notifications::retry,
        crate::routes::notifications::send_manual,
        crate::routes::notifications::stats,
        // Pengguna
        crate::routes::users::list,
        crate::routes::users::detail,
        crate::routes::users::create,
        crate::routes::users::update,
        crate::routes::users::remove,
        crate::routes::users::ban,
        crate::routes::users::unban,
        crate::routes::users::bulk_student_accounts,
        crate::routes::users::link_child,
        crate::routes::users::unlink_child,
        // Jargon GO — data milik pengguna
        crate::routes::me::home,
        crate::routes::me::attendance,
        crate::routes::me::recap,
        // Panic Button
        crate::routes::panic::categories,
        crate::routes::panic::feed,
        crate::routes::panic::detail,
        crate::routes::panic::create_report,
        crate::routes::panic::toggle_support,
        crate::routes::panic::add_comment,
        crate::routes::panic::moderate,
        crate::routes::panic::update_status,
        crate::routes::panic::unmask,
        crate::routes::panic::stats,
        // Pemberkasan
        crate::routes::documents::list_types,
        crate::routes::documents::list_submissions,
        crate::routes::documents::submission_detail,
        crate::routes::documents::create_submission,
        crate::routes::documents::upload_file,
        crate::routes::documents::delete_file,
        crate::routes::documents::submit,
        crate::routes::documents::review_submission,
        crate::routes::documents::review_file,
        // Berkas
        crate::routes::files::serve,
    ),
    components(
        schemas(
            crate::error::ErrorBody,
            crate::error::FieldError,
            crate::util::PageMeta,
            // Sekolah
            crate::domain::school::School,
            crate::domain::school::SchoolListItem,
            crate::domain::school::CreateSchoolRequest,
            crate::domain::school::UpdateSchoolRequest,
            crate::domain::school::Region,
            crate::domain::school::AcademicYear,
            crate::domain::school::Classroom,
            crate::domain::school::CreateClassroomRequest,
            crate::domain::school::UpdateClassroomRequest,
            // Siswa
            crate::domain::student::Student,
            crate::domain::student::StudentListItem,
            crate::domain::student::CreateStudentRequest,
            crate::domain::student::UpdateStudentRequest,
            crate::domain::student::Guardian,
            crate::domain::student::CreateGuardianRequest,
            crate::domain::student::UpdateGuardianRequest,
            // Biometrik
            crate::domain::face::EnrollFaceRequest,
            crate::domain::face::EnrollFaceResponse,
            crate::domain::face::FaceEnrollmentItem,
            crate::domain::face::FaceCoverage,
            crate::domain::face::ReviewEnrollmentRequest,
            crate::face::quality::QualityReport,
            // Absensi
            crate::domain::attendance::AttendanceStatus,
            crate::domain::attendance::ScanDirection,
            crate::domain::attendance::RecognizeRequest,
            crate::domain::attendance::RecognizeResponse,
            crate::domain::attendance::RecognizeAction,
            crate::domain::attendance::RecognizedStudent,
            crate::domain::attendance::AttendanceRecord,
            crate::domain::attendance::ManualAttendanceRequest,
            crate::domain::attendance::BulkAttendanceRequest,
            crate::domain::attendance::BulkAttendanceResponse,
            crate::domain::attendance::AttendanceSummary,
            crate::domain::attendance::ClassroomSummary,
            crate::domain::attendance::StudentAttendanceRecap,
            crate::domain::attendance::ProvinceOverview,
            crate::domain::attendance::SchoolRate,
            crate::domain::attendance::AttendanceRule,
            crate::domain::attendance::UpsertAttendanceRuleRequest,
            // Perangkat
            crate::domain::device::Device,
            crate::domain::device::CreateDeviceRequest,
            crate::domain::device::UpdateDeviceRequest,
            crate::domain::device::PairingCodeResponse,
            crate::domain::device::PairDeviceRequest,
            crate::domain::device::PairDeviceResponse,
            crate::domain::device::DeviceRuntimeConfig,
            crate::domain::device::DeviceHeartbeatRequest,
            crate::domain::device::DeviceHeartbeatResponse,
            crate::domain::device::TodayWindows,
            crate::domain::device::RosterEntry,
            // Notifikasi
            crate::domain::notification::NotificationTemplate,
            crate::domain::notification::UpsertTemplateRequest,
            crate::domain::notification::OutboxItem,
            crate::domain::notification::SendMessageRequest,
            crate::domain::notification::SendMessageResponse,
            crate::domain::notification::SkippedRecipient,
            crate::domain::notification::NotificationPolicy,
            crate::domain::notification::UpdatePolicyRequest,
            crate::domain::notification::NotificationStats,
            crate::domain::notification::ChannelStat,
            // Pengguna
            crate::domain::user::LoginRequest,
            crate::domain::user::LoginResponse,
            crate::domain::user::RefreshRequest,
            crate::domain::user::UserProfile,
            crate::domain::user::HomeroomRef,
            crate::domain::user::ChangePasswordRequest,
            crate::domain::user::UserListItem,
            crate::domain::user::CreateUserRequest,
            crate::domain::user::UpdateUserRequest,
            crate::domain::user::LinkedStudent,
            crate::domain::user::LinkChildRequest,
            crate::domain::user::BulkStudentAccountRequest,
            crate::domain::user::BulkStudentAccountResponse,
            crate::domain::user::InitialCredential,
            // Panic Button
            crate::domain::panic::PanicCategory,
            crate::domain::panic::PanicFeedItem,
            crate::domain::panic::PanicReportDetail,
            crate::domain::panic::PanicComment,
            crate::domain::panic::PanicTimelineEntry,
            crate::domain::panic::CreateReportRequest,
            crate::domain::panic::CreateReportResponse,
            crate::domain::panic::CreateCommentRequest,
            crate::domain::panic::ModerateReportRequest,
            crate::domain::panic::UpdateReportStatusRequest,
            crate::domain::panic::UnmaskRequest,
            crate::domain::panic::UnmaskedAuthor,
            crate::domain::panic::PanicStats,
            crate::domain::panic::CategoryCount,
            // Pemberkasan
            crate::domain::document::DocumentType,
            crate::domain::document::SubmissionListItem,
            crate::domain::document::SubmissionDetail,
            crate::domain::document::SubmissionFile,
            crate::domain::document::ChecklistItem,
            crate::domain::document::SubmissionEvent,
            crate::domain::document::CreateSubmissionRequest,
            crate::domain::document::UploadFileRequest,
            crate::domain::document::UploadFileResponse,
            crate::domain::document::ReviewSubmissionRequest,
            crate::domain::document::ReviewFileRequest,
            // Jargon GO
            crate::routes::me::HomeSummary,
            crate::routes::me::StudentTodayCard,
            crate::routes::me::SchoolTodayCard,
            // Dashboard
            crate::routes::dashboard::SchoolDashboard,
            crate::routes::dashboard::DeviceHealth,
            crate::routes::dashboard::TrendPoint,
            crate::routes::dashboard::NotificationBrief,
            // Sistem
            crate::routes::health::HealthStatus,
            crate::routes::health::ComponentStatus,
            crate::routes::health::FaceIndexStatus,
        )
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi
            .components
            .as_mut()
            .expect("komponen selalu ada karena schemas terdaftar");

        components.add_security_scheme(
            "bearer",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .description(Some(
                        "Access token dari POST /v1/auth/login. Berlaku 1 jam; \
                         perbarui dengan POST /v1/auth/refresh.",
                    ))
                    .build(),
            ),
        );

        // Skema `Device` bukan bearer standar, jadi didaftarkan sebagai
        // apiKey pada header agar Swagger UI tetap bisa mengirimkannya.
        components.add_security_scheme(
            "device",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                "Authorization",
                "Isi dengan `Device <device_token>` — token diperoleh dari \
                 POST /v1/devices/pair.",
            ))),
        );

        components.add_security_scheme(
            "api_key",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                "X-Api-Key",
                "Kredensial layanan internal; wajib disertai header X-Api-Secret.",
            ))),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spesifikasi_terbentuk_dan_memuat_endpoint_inti() {
        let doc = ApiDoc::openapi();
        let json = serde_json::to_string(&doc).expect("spesifikasi harus bisa diserialisasi");

        for path in [
            "/v1/auth/login",
            "/v1/kiosk/recognize",
            "/v1/students/{id}/face",
            "/v1/attendances/manual",
            "/v1/dashboard/province",
            "/v1/devices/pair",
        ] {
            assert!(json.contains(path), "endpoint {path} tidak ada di spesifikasi");
        }
    }

    #[test]
    fn skema_keamanan_terdaftar() {
        let doc = ApiDoc::openapi();
        let components = doc.components.expect("components ada");
        assert!(components.security_schemes.contains_key("bearer"));
        assert!(components.security_schemes.contains_key("device"));
        assert!(components.security_schemes.contains_key("api_key"));
    }

    #[test]
    fn skema_absensi_dan_biometrik_terdaftar() {
        let doc = ApiDoc::openapi();
        let components = doc.components.expect("components ada");
        for name in [
            "RecognizeRequest",
            "RecognizeResponse",
            "AttendanceRecord",
            "EnrollFaceRequest",
            "QualityReport",
        ] {
            assert!(
                components.schemas.contains_key(name),
                "skema {name} belum terdaftar"
            );
        }
    }
}
