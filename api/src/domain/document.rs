//! DTO Pemberkasan — unggah berkas kepegawaian guru.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

pub const PURPOSES: [&str; 6] = [
    "kenaikan_pangkat",
    "sertifikasi",
    "tunjangan",
    "mutasi",
    "pensiun",
    "umum",
];

pub const SUBMISSION_STATUSES: [&str; 6] = [
    "draft",
    "diajukan",
    "diperiksa",
    "revisi",
    "disetujui",
    "ditolak",
];

/// Status yang masih boleh diubah pemiliknya.
///
/// Setelah `diajukan`, guru tidak boleh lagi menambah atau mengganti berkas —
/// kalau tidak, verifikator bisa menyetujui berkas yang sudah ditukar.
pub const EDITABLE_STATUSES: [&str; 2] = ["draft", "revisi"];

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct DocumentType {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub purpose: String,
    pub is_required: bool,
    pub max_bytes: i32,
    pub allowed_mime: Vec<String>,
    pub sort_order: i16,
}

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct SubmissionListItem {
    pub id: Uuid,
    pub user_id: Uuid,
    pub owner_name: String,
    pub school_id: Option<Uuid>,
    pub school_name: Option<String>,
    pub purpose: String,
    pub period: Option<String>,
    pub title: String,
    pub status: String,
    pub file_count: i16,
    pub approved_file_count: i16,
    pub rejected_file_count: i16,
    pub submitted_at: Option<DateTime<Utc>>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SubmissionDetail {
    #[serde(flatten)]
    pub submission: SubmissionListItem,
    pub note: Option<String>,
    pub review_note: Option<String>,
    pub reviewer_name: Option<String>,
    pub files: Vec<SubmissionFile>,
    /// Jenis dokumen yang diminta untuk keperluan ini beserta status
    /// keterisiannya — inilah yang membuat guru tahu apa lagi yang kurang.
    pub checklist: Vec<ChecklistItem>,
    pub timeline: Vec<SubmissionEvent>,
    /// `true` bila berkas masih boleh ditambah/diganti.
    pub is_editable: bool,
}

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct SubmissionFile {
    pub id: Uuid,
    pub document_type_id: Option<Uuid>,
    pub document_type_name: Option<String>,
    pub original_name: String,
    pub mime_type: String,
    pub bytes: i32,
    pub status: String,
    pub reject_reason: Option<String>,
    pub file_url: String,
    pub uploaded_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ChecklistItem {
    pub document_type_id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub is_required: bool,
    pub uploaded: bool,
    pub status: Option<String>,
    pub reject_reason: Option<String>,
}

#[derive(Debug, Serialize, FromRow, ToSchema)]
pub struct SubmissionEvent {
    pub status: String,
    pub note: Option<String>,
    pub actor_label: Option<String>,
    pub created_at: DateTime<Utc>,
}

// =====================================================================
// Request
// =====================================================================

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateSubmissionRequest {
    /// `kenaikan_pangkat` / `sertifikasi` / `tunjangan` / `mutasi` /
    /// `pensiun` / `umum`.
    pub purpose: String,
    #[validate(length(min = 5, max = 150, message = "judul pengajuan 5-150 karakter"))]
    pub title: String,
    /// Periode usulan, mis. `April 2027`.
    #[validate(length(max = 40))]
    pub period: Option<String>,
    #[validate(length(max = 2000))]
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UploadFileRequest {
    /// Jenis dokumen dari daftar `checklist`. Kosongkan untuk berkas
    /// pendukung yang tidak masuk daftar.
    pub document_type_id: Option<Uuid>,
    #[validate(length(min = 1, max = 200))]
    pub original_name: String,
    /// Isi berkas (PDF/JPEG/PNG) dalam base64, tanpa prefix data URI.
    #[validate(length(min = 100, message = "berkas tidak boleh kosong"))]
    pub content_base64: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UploadFileResponse {
    pub file_id: Uuid,
    pub document_type_name: Option<String>,
    pub bytes: i32,
    /// Sisa dokumen wajib yang belum diunggah.
    pub missing_required: Vec<String>,
    pub message: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ReviewSubmissionRequest {
    /// `diperiksa` / `revisi` / `disetujui` / `ditolak`.
    pub status: String,
    #[validate(length(min = 3, max = 2000, message = "catatan pemeriksaan wajib diisi"))]
    pub note: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ReviewFileRequest {
    /// `disetujui` atau `ditolak`.
    pub status: String,
    #[validate(length(max = 300))]
    pub reject_reason: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SubmissionFilter {
    pub school_id: Option<Uuid>,
    pub purpose: Option<String>,
    pub status: Option<String>,
    /// `true` = hanya pengajuan saya sendiri.
    pub mine: Option<bool>,
}

impl ReviewSubmissionRequest {
    /// Menolak atau meminta revisi tanpa alasan yang jelas membuat guru
    /// mengunggah ulang berkas yang sama berkali-kali.
    pub fn validate_transition(&self) -> Result<(), crate::error::ApiError> {
        use crate::error::ApiError;

        if !SUBMISSION_STATUSES.contains(&self.status.as_str()) {
            return Err(ApiError::field(
                "status",
                &format!("pilih salah satu: {}", SUBMISSION_STATUSES.join(", ")),
            ));
        }
        if self.status == "draft" || self.status == "diajukan" {
            return Err(ApiError::field(
                "status",
                "verifikator tidak dapat mengembalikan pengajuan ke draft/diajukan; \
                 gunakan `revisi` bila ada berkas yang harus diperbaiki",
            ));
        }
        if matches!(self.status.as_str(), "revisi" | "ditolak") && self.note.trim().len() < 10 {
            return Err(ApiError::field(
                "note",
                "sebutkan berkas mana yang bermasalah dan apa yang harus diperbaiki \
                 (minimal 10 karakter)",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_yang_masih_bisa_diubah() {
        assert!(EDITABLE_STATUSES.contains(&"draft"));
        assert!(EDITABLE_STATUSES.contains(&"revisi"));
        // Setelah diajukan, berkas dikunci agar tidak bisa ditukar setelah
        // sebagian diperiksa.
        assert!(!EDITABLE_STATUSES.contains(&"diajukan"));
        assert!(!EDITABLE_STATUSES.contains(&"disetujui"));
    }

    fn review(status: &str, note: &str) -> ReviewSubmissionRequest {
        ReviewSubmissionRequest {
            status: status.into(),
            note: note.into(),
        }
    }

    #[test]
    fn penolakan_wajib_menjelaskan_alasan() {
        assert!(review("ditolak", "tidak").validate_transition().is_err());
        assert!(review("revisi", "kurang").validate_transition().is_err());
        assert!(review("revisi", "SKP tahun 2025 belum diunggah")
            .validate_transition()
            .is_ok());
    }

    #[test]
    fn persetujuan_tidak_menuntut_catatan_panjang() {
        assert!(review("disetujui", "ok").validate_transition().is_ok());
    }

    #[test]
    fn verifikator_tidak_bisa_mengembalikan_ke_draft() {
        assert!(review("draft", "kembalikan").validate_transition().is_err());
        assert!(review("diajukan", "kembalikan").validate_transition().is_err());
    }

    #[test]
    fn status_asing_ditolak() {
        assert!(review("dibatalkan", "apa saja").validate_transition().is_err());
    }

    #[test]
    fn pengajuan_wajib_berjudul() {
        let r = CreateSubmissionRequest {
            purpose: "kenaikan_pangkat".into(),
            title: "KP".into(),
            period: None,
            note: None,
        };
        assert!(r.validate().is_err());
    }
}
