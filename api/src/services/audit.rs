//! Jejak audit sisi API.
//!
//! Absensi adalah data yang berkonsekuensi administratif: mengubah status
//! seorang siswa dari `alfa` menjadi `hadir` bisa berarti menghapus sanksi.
//! Karena itu setiap perubahan yang tidak berasal dari pemindaian wajah
//! WAJIB tercatat: siapa, kapan, dari IP mana, nilai sebelum dan sesudah.

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::AuthUser;

// `Device` dan `ApiClient` sudah menjadi bagian dari kontrak kolom
// `audit_logs.actor_type` (dengan CHECK constraint di database), jadi varian
// ini harus ada meski entri dari kedua sumber itu saat ini ditulis melalui
// `attendance_events` yang lebih spesifik.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum ActorKind {
    User,
    Device,
    ApiClient,
    System,
}

impl ActorKind {
    fn as_str(self) -> &'static str {
        match self {
            ActorKind::User => "user",
            ActorKind::Device => "device",
            ActorKind::ApiClient => "api_client",
            ActorKind::System => "system",
        }
    }
}

pub struct AuditEntry<'a> {
    pub actor_kind: ActorKind,
    pub actor_id: Option<Uuid>,
    pub actor_label: Option<String>,
    pub school_id: Option<Uuid>,
    pub action: &'a str,
    pub entity_type: Option<&'a str>,
    pub entity_id: Option<Uuid>,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub request_id: Option<String>,
}

impl<'a> AuditEntry<'a> {
    pub fn by_user(user: &AuthUser, action: &'a str) -> Self {
        // Label memuat NIK/NISN bila ada. Pada sistem pemerintahan, jejak
        // audit yang hanya menyebut username sulit dipertanggungjawabkan:
        // username bisa berubah, sedangkan identitas kependudukan tidak.
        let label = match user.identity.as_deref() {
            Some(identity) => format!("{} ({} · {})", user.name, user.username, identity),
            None => format!("{} ({})", user.name, user.username),
        };

        Self {
            actor_kind: ActorKind::User,
            actor_id: Some(user.id),
            actor_label: Some(label),
            school_id: user.school_id,
            action,
            entity_type: None,
            entity_id: None,
            before: None,
            after: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        }
    }

    pub fn by_system(action: &'a str) -> Self {
        Self {
            actor_kind: ActorKind::System,
            actor_id: None,
            actor_label: Some("worker".into()),
            school_id: None,
            action,
            entity_type: None,
            entity_id: None,
            before: None,
            after: None,
            ip_address: None,
            user_agent: None,
            request_id: None,
        }
    }

    pub fn school(mut self, school_id: Uuid) -> Self {
        self.school_id = Some(school_id);
        self
    }

    pub fn entity(mut self, entity_type: &'a str, entity_id: Uuid) -> Self {
        self.entity_type = Some(entity_type);
        self.entity_id = Some(entity_id);
        self
    }

    pub fn before<T: Serialize>(mut self, value: &T) -> Self {
        self.before = serde_json::to_value(value).ok();
        self
    }

    pub fn after<T: Serialize>(mut self, value: &T) -> Self {
        self.after = serde_json::to_value(value).ok();
        self
    }

    /// Lampirkan konteks HTTP (IP, user agent, request id) pada entri audit.
    #[allow(dead_code)]
    pub fn request(mut self, ip: Option<String>, ua: Option<String>, request_id: Option<String>) -> Self {
        self.ip_address = ip;
        self.user_agent = ua;
        self.request_id = request_id;
        self
    }

    /// Tulis entri audit.
    ///
    /// Kegagalan audit di-log tapi TIDAK menggagalkan operasi utama: menolak
    /// koreksi absensi hanya karena tabel audit sedang bermasalah akan
    /// membuat sistem tidak bisa dipakai di lapangan.
    pub async fn write(self, pool: &PgPool) {
        let result = sqlx::query(
            r#"
            INSERT INTO audit_logs (
                actor_type, actor_id, actor_label, school_id, action,
                entity_type, entity_id, before, after, ip_address, user_agent, request_id
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
            "#,
        )
        .bind(self.actor_kind.as_str())
        .bind(self.actor_id)
        .bind(self.actor_label)
        .bind(self.school_id)
        .bind(self.action)
        .bind(self.entity_type)
        .bind(self.entity_id)
        .bind(self.before)
        .bind(self.after)
        .bind(self.ip_address)
        .bind(self.user_agent)
        .bind(self.request_id)
        .execute(pool)
        .await;

        if let Err(e) = result {
            tracing::error!(error = %e, action = self.action, "gagal menulis audit log");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn user() -> AuthUser {
        AuthUser {
            id: Uuid::new_v4(),
            username: "budi".into(),
            name: "Budi Guru".into(),
            identity: Some("1275010101900001".into()),
            school_id: Some(Uuid::new_v4()),
            extra_schools: vec![],
            students: vec![],
            roles: vec!["guru".into()],
            permissions: HashSet::new(),
        }
    }

    #[test]
    fn entri_user_membawa_identitas() {
        let u = user();
        let e = AuditEntry::by_user(&u, "attendance.override");
        assert!(matches!(e.actor_kind, ActorKind::User));
        assert_eq!(e.actor_id, Some(u.id));
        // NIK ikut dicatat: username bisa diubah, NIK tidak.
        assert_eq!(
            e.actor_label.as_deref(),
            Some("Budi Guru (budi · 1275010101900001)")
        );
        assert_eq!(e.school_id, u.school_id);
    }

    #[test]
    fn akun_tanpa_identitas_tetap_tercatat() {
        let mut u = user();
        u.identity = None;
        let e = AuditEntry::by_user(&u, "student.update");
        assert_eq!(e.actor_label.as_deref(), Some("Budi Guru (budi)"));
    }

    #[test]
    fn builder_merangkai_entitas_dan_perubahan() {
        let u = user();
        let id = Uuid::new_v4();
        let e = AuditEntry::by_user(&u, "student.update")
            .entity("student", id)
            .before(&serde_json::json!({"status": "aktif"}))
            .after(&serde_json::json!({"status": "pindah"}));

        assert_eq!(e.entity_type, Some("student"));
        assert_eq!(e.entity_id, Some(id));
        assert_eq!(e.before.unwrap()["status"], "aktif");
        assert_eq!(e.after.unwrap()["status"], "pindah");
    }

    #[test]
    fn entri_sistem_tanpa_actor_id() {
        let e = AuditEntry::by_system("rollup.daily");
        assert!(e.actor_id.is_none());
        assert!(matches!(e.actor_kind, ActorKind::System));
    }
}
