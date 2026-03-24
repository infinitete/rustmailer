use serde_json::Value;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct DomainModel {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, FromRow)]
pub struct MailboxModel {
    pub id: i64,
    pub domain_id: i64,
    pub local_part: String,
    pub email: String,
    pub password_hash: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, FromRow)]
pub struct FolderModel {
    pub id: i64,
    pub mailbox_id: i64,
    pub name: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct FolderStateModel {
    pub folder_id: i64,
    pub uid_validity: i64,
    pub uid_next: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct MessageSummaryModel {
    pub uid: i64,
    pub subject: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct AuditLogModel {
    pub id: i64,
    pub actor: String,
    pub action: String,
    pub details: Value,
    pub created_at: String,
}
