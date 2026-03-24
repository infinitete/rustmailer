use serde_json::Value;
use sqlx::PgPool;

use crate::core::entities::{DomainName, MailboxAddress};
use crate::db::models::{
    AuditLogModel, DomainModel, FolderModel, FolderStateModel, MailboxModel, MessageSummaryModel,
};
use crate::error::AppResult;

#[derive(Clone)]
pub struct Repositories {
    pool: PgPool,
    pub audit_logs: AuditLogRepository,
    pub domains: DomainRepository,
    pub folders: FolderRepository,
    pub mailboxes: MailboxRepository,
}

impl Repositories {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool: pool.clone(),
            audit_logs: AuditLogRepository::new(pool.clone()),
            domains: DomainRepository::new(pool.clone()),
            folders: FolderRepository::new(pool.clone()),
            mailboxes: MailboxRepository::new(pool),
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[derive(Clone)]
pub struct DomainRepository {
    pool: PgPool,
}

impl DomainRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, domain: DomainName) -> AppResult<DomainModel> {
        let record = sqlx::query_as::<_, DomainModel>(
            r#"
            INSERT INTO domains (name)
            VALUES ($1)
            RETURNING id, name, enabled
            "#,
        )
        .bind(domain.as_str())
        .fetch_one(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn find_by_name(&self, domain: &DomainName) -> AppResult<Option<DomainModel>> {
        let record = sqlx::query_as::<_, DomainModel>(
            r#"
            SELECT id, name, enabled
            FROM domains
            WHERE name = $1
            "#,
        )
        .bind(domain.as_str())
        .fetch_optional(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn find_by_id(&self, id: i64) -> AppResult<Option<DomainModel>> {
        let record = sqlx::query_as::<_, DomainModel>(
            r#"
            SELECT id, name, enabled
            FROM domains
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn list(&self) -> AppResult<Vec<DomainModel>> {
        let records = sqlx::query_as::<_, DomainModel>(
            r#"
            SELECT id, name, enabled
            FROM domains
            ORDER BY id DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    pub async fn delete(&self, id: i64) -> AppResult<()> {
        sqlx::query(
            r#"
            DELETE FROM domains
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update(
        &self,
        id: i64,
        name: Option<&str>,
        enabled: Option<bool>,
    ) -> AppResult<Option<DomainModel>> {
        let record = sqlx::query_as::<_, DomainModel>(
            r#"
            UPDATE domains
            SET
                name = COALESCE($2, name),
                enabled = COALESCE($3, enabled)
            WHERE id = $1
            RETURNING id, name, enabled
            "#,
        )
        .bind(id)
        .bind(name)
        .bind(enabled)
        .fetch_optional(&self.pool)
        .await?;

        Ok(record)
    }
}

#[derive(Clone)]
pub struct MailboxRepository {
    pool: PgPool,
}

impl MailboxRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, mailbox: MailboxAddress, domain_id: i64) -> AppResult<MailboxModel> {
        let record = sqlx::query_as::<_, MailboxModel>(
            r#"
            INSERT INTO mailboxes (domain_id, local_part, email, password_hash)
            VALUES ($1, $2, $3, $4)
            RETURNING id, domain_id, local_part, email, password_hash, enabled
            "#,
        )
        .bind(domain_id)
        .bind(mailbox.local_part())
        .bind(mailbox.as_str())
        .bind("pending")
        .fetch_one(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn find_by_email(&self, email: &str) -> AppResult<Option<MailboxModel>> {
        let record = sqlx::query_as::<_, MailboxModel>(
            r#"
            SELECT id, domain_id, local_part, email, password_hash, enabled
            FROM mailboxes
            WHERE email = $1
            "#,
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn find_by_id(&self, id: i64) -> AppResult<Option<MailboxModel>> {
        let record = sqlx::query_as::<_, MailboxModel>(
            r#"
            SELECT id, domain_id, local_part, email, password_hash, enabled
            FROM mailboxes
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn list(&self) -> AppResult<Vec<MailboxModel>> {
        let records = sqlx::query_as::<_, MailboxModel>(
            r#"
            SELECT id, domain_id, local_part, email, password_hash, enabled
            FROM mailboxes
            ORDER BY id DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    pub async fn delete(&self, id: i64) -> AppResult<()> {
        sqlx::query(
            r#"
            DELETE FROM mailboxes
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update(
        &self,
        id: i64,
        enabled: Option<bool>,
        password_hash: Option<&str>,
    ) -> AppResult<Option<MailboxModel>> {
        let record = sqlx::query_as::<_, MailboxModel>(
            r#"
            UPDATE mailboxes
            SET
                enabled = COALESCE($2, enabled),
                password_hash = COALESCE($3, password_hash)
            WHERE id = $1
            RETURNING id, domain_id, local_part, email, password_hash, enabled
            "#,
        )
        .bind(id)
        .bind(enabled)
        .bind(password_hash)
        .fetch_optional(&self.pool)
        .await?;

        Ok(record)
    }
}

#[derive(Clone)]
pub struct AuditLogRepository {
    pool: PgPool,
}

impl AuditLogRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, actor: &str, action: &str, details: Value) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT INTO audit_logs (actor, action, details)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(actor)
        .bind(action)
        .bind(details)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn list_recent(&self, limit: i64) -> AppResult<Vec<AuditLogModel>> {
        let records = sqlx::query_as::<_, AuditLogModel>(
            r#"
            SELECT
                id,
                actor,
                action,
                details,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS created_at
            FROM audit_logs
            ORDER BY id DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }
}

#[derive(Clone)]
pub struct FolderRepository {
    pool: PgPool,
}

impl FolderRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_for_mailbox(&self, mailbox_id: i64) -> AppResult<Vec<FolderModel>> {
        let records = sqlx::query_as::<_, FolderModel>(
            r#"
            SELECT id, mailbox_id, name
            FROM mail_folders
            WHERE mailbox_id = $1
            ORDER BY id
            "#,
        )
        .bind(mailbox_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    pub async fn find_by_name(
        &self,
        mailbox_id: i64,
        name: &str,
    ) -> AppResult<Option<FolderModel>> {
        let record = sqlx::query_as::<_, FolderModel>(
            r#"
            SELECT id, mailbox_id, name
            FROM mail_folders
            WHERE mailbox_id = $1 AND name = $2
            "#,
        )
        .bind(mailbox_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn select_state(
        &self,
        mailbox_id: i64,
        name: &str,
    ) -> AppResult<Option<FolderStateModel>> {
        let record = sqlx::query_as::<_, FolderStateModel>(
            r#"
            SELECT state.folder_id, state.uid_validity, state.uid_next
            FROM mail_folders folders
            JOIN imap_folder_state state ON state.folder_id = folders.id
            WHERE folders.mailbox_id = $1 AND folders.name = $2
            "#,
        )
        .bind(mailbox_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn fetch_message_subject(
        &self,
        mailbox_id: i64,
        name: &str,
        sequence: i64,
    ) -> AppResult<Option<MessageSummaryModel>> {
        let record = sqlx::query_as::<_, MessageSummaryModel>(
            r#"
            SELECT uids.uid, messages.subject
            FROM mail_folders folders
            JOIN message_delivery delivery ON delivery.folder_id = folders.id
            JOIN imap_message_uids uids ON uids.delivery_id = delivery.id AND uids.folder_id = folders.id
            JOIN messages ON messages.id = delivery.message_id
            WHERE folders.mailbox_id = $1 AND folders.name = $2
            ORDER BY uids.uid
            LIMIT 1 OFFSET $3
            "#,
        )
        .bind(mailbox_id)
        .bind(name)
        .bind(sequence - 1)
        .fetch_optional(&self.pool)
        .await?;

        Ok(record)
    }
}
