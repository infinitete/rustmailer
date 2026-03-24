use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use serde_json::{Value, json};
use sqlx::FromRow;

use crate::core::commands::{CreateDomain, ProvisionMailbox};
use crate::core::entities::{DomainName, MailboxAddress};
use crate::core::events::DomainEvent;
use crate::db::models::{AuditLogModel, DomainModel, FolderStateModel, MailboxModel};
use crate::db::repositories::Repositories;
use crate::error::{AppError, AppResult};

const DEFAULT_FOLDERS: [&str; 4] = ["INBOX", "Sent", "Drafts", "Trash"];

#[derive(Debug, Clone)]
pub struct FolderSelection {
    pub exists: i64,
    pub uid_validity: i64,
    pub uid_next: i64,
}

#[derive(Debug, Clone)]
pub struct UpdateDomainInput {
    pub name: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct UpdateMailboxInput {
    pub enabled: Option<bool>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageIdentifier {
    Sequence(i64),
    Uid(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageSearch {
    All,
    Seen,
    Unseen,
    Deleted,
    Undeleted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlagUpdateMode {
    Add,
    Remove,
    Replace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageMetadata {
    pub sequence: i64,
    pub uid: i64,
    pub subject: Option<String>,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, FromRow)]
struct FolderMessageRow {
    delivery_id: i64,
    uid: i64,
    subject: Option<String>,
    flag: Option<String>,
}

#[derive(Debug, Clone)]
struct FolderMessageRecord {
    delivery_id: i64,
    metadata: MessageMetadata,
}

#[derive(Clone)]
pub struct MailCoreService {
    repositories: Repositories,
    events: Arc<Mutex<Vec<DomainEvent>>>,
}

impl MailCoreService {
    pub fn new(repositories: Repositories) -> Self {
        Self {
            repositories,
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn create_domain(&self, domain: &str) -> AppResult<DomainModel> {
        let command = CreateDomain {
            domain: DomainName::new(domain)?,
        };

        if let Some(existing) = self
            .repositories
            .domains
            .find_by_name(&command.domain)
            .await?
        {
            return Ok(existing);
        }

        let created = self.repositories.domains.create(command.domain).await?;
        self.write_audit_log(
            "admin",
            "domain.created",
            json!({
                "id": created.id,
                "name": created.name,
                "enabled": created.enabled,
            }),
        )
        .await?;

        Ok(created)
    }

    pub async fn list_domains(&self) -> AppResult<Vec<DomainModel>> {
        self.repositories.domains.list().await
    }

    pub async fn delete_domain(&self, id: i64) -> AppResult<()> {
        let domain = self
            .repositories
            .domains
            .find_by_id(id)
            .await?
            .ok_or(AppError::DomainNotFound { id })?;
        self.repositories.domains.delete(id).await?;
        self.write_audit_log(
            "admin",
            "domain.deleted",
            json!({
                "id": domain.id,
                "name": domain.name,
            }),
        )
        .await?;
        Ok(())
    }

    pub async fn update_domain(&self, id: i64, input: UpdateDomainInput) -> AppResult<DomainModel> {
        let normalized_name = input
            .name
            .as_deref()
            .map(DomainName::new)
            .transpose()?
            .map(|name| name.as_str().to_string());

        let updated = self
            .repositories
            .domains
            .update(id, normalized_name.as_deref(), input.enabled)
            .await?
            .ok_or(AppError::DomainNotFound { id })?;

        self.write_audit_log(
            "admin",
            "domain.updated",
            json!({
                "id": updated.id,
                "name": updated.name,
                "enabled": updated.enabled,
            }),
        )
        .await?;

        Ok(updated)
    }

    pub async fn provision_mailbox(
        &self,
        domain: &str,
        local_part: &str,
        password: &str,
    ) -> AppResult<MailboxModel> {
        let command = ProvisionMailbox {
            domain: DomainName::new(domain)?,
            local_part: local_part.trim().to_ascii_lowercase(),
            password: password.to_string(),
        };
        let mailbox = MailboxAddress::new(&format!(
            "{}@{}",
            command.local_part,
            command.domain.as_str()
        ))?;
        let password_hash = hash_password(&command.password)?;
        let pool = self.repositories.pool();
        let mut tx = pool.begin().await?;

        let domain_record = match sqlx::query_as::<_, DomainModel>(
            r#"
            SELECT id, name, enabled
            FROM domains
            WHERE name = $1
            "#,
        )
        .bind(command.domain.as_str())
        .fetch_optional(&mut *tx)
        .await?
        {
            Some(record) => {
                if !record.enabled {
                    return Err(AppError::DomainDisabled {
                        domain: record.name.clone(),
                    });
                }
                record
            }
            None => {
                sqlx::query_as::<_, DomainModel>(
                    r#"
                    INSERT INTO domains (name)
                    VALUES ($1)
                    RETURNING id, name, enabled
                    "#,
                )
                .bind(command.domain.as_str())
                .fetch_one(&mut *tx)
                .await?
            }
        };

        let mailbox_record = sqlx::query_as::<_, MailboxModel>(
            r#"
            INSERT INTO mailboxes (domain_id, local_part, email, password_hash)
            VALUES ($1, $2, $3, $4)
            RETURNING id, domain_id, local_part, email, password_hash, enabled
            "#,
        )
        .bind(domain_record.id)
        .bind(mailbox.local_part())
        .bind(mailbox.as_str())
        .bind(password_hash)
        .fetch_one(&mut *tx)
        .await?;

        let uid_validity = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        for folder_name in DEFAULT_FOLDERS {
            let folder_id: i64 = sqlx::query_scalar(
                r#"
                INSERT INTO mail_folders (mailbox_id, name)
                VALUES ($1, $2)
                RETURNING id
                "#,
            )
            .bind(mailbox_record.id)
            .bind(folder_name)
            .fetch_one(&mut *tx)
            .await?;

            sqlx::query(
                r#"
                INSERT INTO imap_folder_state (folder_id, uid_validity, uid_next)
                VALUES ($1, $2, $3)
                "#,
            )
            .bind(folder_id)
            .bind(uid_validity)
            .bind(1_i64)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        self.record_event(DomainEvent::MailboxProvisioned {
            mailbox_id: mailbox_record.id,
            email: mailbox_record.email.clone(),
        });
        self.write_audit_log(
            "admin",
            "mailbox.provisioned",
            json!({
                "id": mailbox_record.id,
                "email": mailbox_record.email.clone(),
                "domain_id": mailbox_record.domain_id,
            }),
        )
        .await?;

        Ok(mailbox_record)
    }

    pub async fn list_mailboxes(&self) -> AppResult<Vec<MailboxModel>> {
        self.repositories.mailboxes.list().await
    }

    pub async fn delete_mailbox(&self, id: i64) -> AppResult<()> {
        let mailbox = self
            .repositories
            .mailboxes
            .find_by_id(id)
            .await?
            .ok_or(AppError::MailboxIdNotFound { id })?;
        self.repositories.mailboxes.delete(id).await?;
        self.write_audit_log(
            "admin",
            "mailbox.deleted",
            json!({
                "id": mailbox.id,
                "email": mailbox.email,
            }),
        )
        .await?;
        Ok(())
    }

    pub async fn update_mailbox(
        &self,
        id: i64,
        input: UpdateMailboxInput,
    ) -> AppResult<MailboxModel> {
        let password_hash = match input.password.as_deref() {
            Some(password) => Some(hash_password(password)?),
            None => None,
        };

        let updated = self
            .repositories
            .mailboxes
            .update(id, input.enabled, password_hash.as_deref())
            .await?
            .ok_or(AppError::MailboxIdNotFound { id })?;

        self.write_audit_log(
            "admin",
            "mailbox.updated",
            json!({
                "id": updated.id,
                "email": updated.email,
                "enabled": updated.enabled,
                "password_updated": password_hash.is_some(),
            }),
        )
        .await?;

        Ok(updated)
    }

    pub async fn list_audit_logs(&self, limit: i64) -> AppResult<Vec<AuditLogModel>> {
        self.repositories.audit_logs.list_recent(limit).await
    }

    pub async fn authenticate_mailbox(
        &self,
        email: &str,
        password: &str,
    ) -> AppResult<MailboxModel> {
        let mailbox = self
            .repositories
            .mailboxes
            .find_by_email(email)
            .await?
            .ok_or_else(|| AppError::MailboxNotFound {
                email: email.to_string(),
            })?;

        if !mailbox.enabled {
            return Err(AppError::MailboxDisabled {
                email: mailbox.email.clone(),
            });
        }
        self.ensure_domain_enabled(mailbox.domain_id).await?;

        let parsed_hash =
            PasswordHash::new(&mailbox.password_hash).map_err(|_| AppError::AuthError)?;
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| AppError::AuthError)?;

        Ok(mailbox)
    }

    pub async fn receive_inbound_message(
        &self,
        sender: &str,
        recipients: &[String],
        raw_message: &str,
    ) -> AppResult<()> {
        let pool = self.repositories.pool();
        let mut tx = pool.begin().await?;
        let subject = extract_subject(raw_message);
        let size_bytes = raw_message.len() as i64;

        let message_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO messages (rfc822, subject, from_addr, size_bytes)
            VALUES ($1, $2, $3, $4)
            RETURNING id
            "#,
        )
        .bind(raw_message.as_bytes())
        .bind(subject)
        .bind(sender)
        .bind(size_bytes)
        .fetch_one(&mut *tx)
        .await?;

        for recipient in recipients {
            let mailbox = sqlx::query_as::<_, MailboxModel>(
                r#"
                SELECT id, domain_id, local_part, email, password_hash, enabled
                FROM mailboxes
                WHERE email = $1
                "#,
            )
            .bind(recipient)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::MailboxNotFound {
                email: recipient.clone(),
            })?;

            if !mailbox.enabled {
                return Err(AppError::MailboxDisabled {
                    email: mailbox.email.clone(),
                });
            }
            self.ensure_domain_enabled(mailbox.domain_id).await?;

            let folder_id: i64 = sqlx::query_scalar(
                r#"
                SELECT id
                FROM mail_folders
                WHERE mailbox_id = $1 AND name = 'INBOX'
                "#,
            )
            .bind(mailbox.id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::FolderNotFound {
                name: "INBOX".to_string(),
            })?;

            let state: FolderStateModel = sqlx::query_as(
                r#"
                SELECT folder_id, uid_validity, uid_next
                FROM imap_folder_state
                WHERE folder_id = $1
                FOR UPDATE
                "#,
            )
            .bind(folder_id)
            .fetch_one(&mut *tx)
            .await?;

            let delivery_id: i64 = sqlx::query_scalar(
                r#"
                INSERT INTO message_delivery (message_id, mailbox_id, folder_id)
                VALUES ($1, $2, $3)
                RETURNING id
                "#,
            )
            .bind(message_id)
            .bind(mailbox.id)
            .bind(folder_id)
            .fetch_one(&mut *tx)
            .await?;

            sqlx::query(
                r#"
                INSERT INTO imap_message_uids (delivery_id, folder_id, uid)
                VALUES ($1, $2, $3)
                "#,
            )
            .bind(delivery_id)
            .bind(folder_id)
            .bind(state.uid_next)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                r#"
                UPDATE imap_folder_state
                SET uid_next = $2
                WHERE folder_id = $1
                "#,
            )
            .bind(folder_id)
            .bind(state.uid_next + 1)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        self.record_event(DomainEvent::MessageStored {
            sender: sender.to_string(),
            recipient_count: recipients.len() as u32,
        });

        Ok(())
    }

    pub async fn select_folder(&self, mailbox_id: i64, name: &str) -> AppResult<FolderSelection> {
        let state = self
            .repositories
            .folders
            .select_state(mailbox_id, name)
            .await?
            .ok_or_else(|| AppError::FolderNotFound {
                name: name.to_string(),
            })?;
        let exists = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM message_delivery delivery
            JOIN mail_folders folders ON folders.id = delivery.folder_id
            WHERE folders.mailbox_id = $1 AND folders.name = $2
            "#,
        )
        .bind(mailbox_id)
        .bind(name)
        .fetch_one(self.repositories.pool())
        .await?;

        Ok(FolderSelection {
            exists,
            uid_validity: state.uid_validity,
            uid_next: state.uid_next,
        })
    }

    pub async fn fetch_message_subject(
        &self,
        mailbox_id: i64,
        name: &str,
        sequence: i64,
    ) -> AppResult<Option<String>> {
        Ok(self
            .fetch_message(mailbox_id, name, MessageIdentifier::Sequence(sequence))
            .await?
            .and_then(|message| message.subject))
    }

    pub async fn fetch_message(
        &self,
        mailbox_id: i64,
        name: &str,
        identifier: MessageIdentifier,
    ) -> AppResult<Option<MessageMetadata>> {
        let messages = self.load_folder_messages(mailbox_id, name).await?;
        Ok(find_message(messages, identifier).map(|record| record.metadata))
    }

    pub async fn search_messages(
        &self,
        mailbox_id: i64,
        name: &str,
        query: MessageSearch,
        return_uids: bool,
    ) -> AppResult<Vec<i64>> {
        let messages = self.load_folder_messages(mailbox_id, name).await?;
        Ok(messages
            .into_iter()
            .filter(|message| matches_search(&message.metadata.flags, &query))
            .map(|message| {
                if return_uids {
                    message.metadata.uid
                } else {
                    message.metadata.sequence
                }
            })
            .collect())
    }

    pub async fn store_flags(
        &self,
        mailbox_id: i64,
        name: &str,
        identifier: MessageIdentifier,
        mode: FlagUpdateMode,
        flags: &[String],
    ) -> AppResult<Option<MessageMetadata>> {
        let target = {
            let messages = self.load_folder_messages(mailbox_id, name).await?;
            find_message(messages, identifier.clone())
        };
        let Some(target) = target else {
            return Ok(None);
        };

        let normalized_flags = normalize_flags(flags);
        let pool = self.repositories.pool();
        let mut tx = pool.begin().await?;

        match mode {
            FlagUpdateMode::Replace => {
                sqlx::query(
                    r#"
                    DELETE FROM message_flags
                    WHERE delivery_id = $1
                    "#,
                )
                .bind(target.delivery_id)
                .execute(&mut *tx)
                .await?;

                insert_flags(&mut tx, target.delivery_id, &normalized_flags).await?;
            }
            FlagUpdateMode::Add => {
                insert_flags(&mut tx, target.delivery_id, &normalized_flags).await?;
            }
            FlagUpdateMode::Remove => {
                for flag in normalized_flags {
                    sqlx::query(
                        r#"
                        DELETE FROM message_flags
                        WHERE delivery_id = $1 AND flag = $2
                        "#,
                    )
                    .bind(target.delivery_id)
                    .bind(flag)
                    .execute(&mut *tx)
                    .await?;
                }
            }
        }

        tx.commit().await?;
        self.fetch_message(mailbox_id, name, identifier).await
    }

    pub async fn expunge_deleted(&self, mailbox_id: i64, name: &str) -> AppResult<Vec<i64>> {
        let messages = self.load_folder_messages(mailbox_id, name).await?;
        let deleted = messages
            .into_iter()
            .filter(|message| has_flag(&message.metadata.flags, "\\Deleted"))
            .collect::<Vec<_>>();
        if deleted.is_empty() {
            return Ok(Vec::new());
        }

        let mut sequence_numbers = deleted
            .iter()
            .map(|message| message.metadata.sequence)
            .collect::<Vec<_>>();
        sequence_numbers.sort_unstable_by(|left, right| right.cmp(left));

        let pool = self.repositories.pool();
        let mut tx = pool.begin().await?;
        for message in deleted {
            sqlx::query(
                r#"
                DELETE FROM message_delivery
                WHERE id = $1
                "#,
            )
            .bind(message.delivery_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;

        Ok(sequence_numbers)
    }

    pub fn events(&self) -> Vec<DomainEvent> {
        self.events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn record_event(&self, event: DomainEvent) {
        self.events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(event);
    }

    async fn write_audit_log(&self, actor: &str, action: &str, details: Value) -> AppResult<()> {
        self.repositories
            .audit_logs
            .create(actor, action, details)
            .await
    }

    async fn ensure_domain_enabled(&self, domain_id: i64) -> AppResult<DomainModel> {
        let domain = self
            .repositories
            .domains
            .find_by_id(domain_id)
            .await?
            .ok_or(AppError::DomainNotFound { id: domain_id })?;
        if !domain.enabled {
            return Err(AppError::DomainDisabled {
                domain: domain.name.clone(),
            });
        }

        Ok(domain)
    }

    async fn load_folder_messages(
        &self,
        mailbox_id: i64,
        name: &str,
    ) -> AppResult<Vec<FolderMessageRecord>> {
        let rows = sqlx::query_as::<_, FolderMessageRow>(
            r#"
            SELECT delivery.id AS delivery_id, uids.uid, messages.subject, flags.flag
            FROM mail_folders folders
            JOIN message_delivery delivery ON delivery.folder_id = folders.id
            JOIN imap_message_uids uids
              ON uids.delivery_id = delivery.id
             AND uids.folder_id = folders.id
            JOIN messages ON messages.id = delivery.message_id
            LEFT JOIN message_flags flags ON flags.delivery_id = delivery.id
            WHERE folders.mailbox_id = $1 AND folders.name = $2
            ORDER BY uids.uid, flags.flag NULLS LAST
            "#,
        )
        .bind(mailbox_id)
        .bind(name)
        .fetch_all(self.repositories.pool())
        .await?;

        let mut messages: Vec<FolderMessageRecord> = Vec::new();
        for row in rows {
            let subject = row.subject.clone();
            match messages.last_mut() {
                Some(existing)
                    if existing.delivery_id == row.delivery_id
                        && existing.metadata.uid == row.uid
                        && existing.metadata.subject == subject =>
                {
                    if let Some(flag) = row.flag {
                        existing.metadata.flags.push(canonical_flag(&flag));
                    }
                }
                _ => {
                    let mut flags = Vec::new();
                    if let Some(flag) = row.flag {
                        flags.push(canonical_flag(&flag));
                    }
                    messages.push(FolderMessageRecord {
                        delivery_id: row.delivery_id,
                        metadata: MessageMetadata {
                            sequence: messages.len() as i64 + 1,
                            uid: row.uid,
                            subject,
                            flags,
                        },
                    });
                }
            }
        }

        for message in &mut messages {
            message.metadata.flags = normalize_flags(&message.metadata.flags);
        }

        Ok(messages)
    }
}

fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|error| AppError::CommandFailed {
            program: "argon2",
            message: error.to_string(),
        })?
        .to_string();

    Ok(hash)
}

fn extract_subject(raw_message: &str) -> Option<String> {
    raw_message.lines().find_map(|line| {
        line.strip_prefix("Subject:")
            .map(|value| value.trim().to_string())
    })
}

async fn insert_flags(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    delivery_id: i64,
    flags: &[String],
) -> AppResult<()> {
    for flag in flags {
        sqlx::query(
            r#"
            INSERT INTO message_flags (delivery_id, flag)
            VALUES ($1, $2)
            ON CONFLICT (delivery_id, flag) DO NOTHING
            "#,
        )
        .bind(delivery_id)
        .bind(flag)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

fn find_message(
    messages: Vec<FolderMessageRecord>,
    identifier: MessageIdentifier,
) -> Option<FolderMessageRecord> {
    messages.into_iter().find(|message| match identifier {
        MessageIdentifier::Sequence(sequence) => message.metadata.sequence == sequence,
        MessageIdentifier::Uid(uid) => message.metadata.uid == uid,
    })
}

fn matches_search(flags: &[String], query: &MessageSearch) -> bool {
    match query {
        MessageSearch::All => true,
        MessageSearch::Seen => has_flag(flags, "\\Seen"),
        MessageSearch::Unseen => !has_flag(flags, "\\Seen"),
        MessageSearch::Deleted => has_flag(flags, "\\Deleted"),
        MessageSearch::Undeleted => !has_flag(flags, "\\Deleted"),
    }
}

fn has_flag(flags: &[String], expected: &str) -> bool {
    flags.iter().any(|flag| flag.eq_ignore_ascii_case(expected))
}

fn normalize_flags(flags: &[String]) -> Vec<String> {
    let mut normalized = flags
        .iter()
        .map(|flag| canonical_flag(flag))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    normalized.sort();
    normalized
}

fn canonical_flag(flag: &str) -> String {
    match flag.trim().to_ascii_lowercase().as_str() {
        "\\seen" => "\\Seen".to_string(),
        "\\answered" => "\\Answered".to_string(),
        "\\flagged" => "\\Flagged".to_string(),
        "\\deleted" => "\\Deleted".to_string(),
        "\\draft" => "\\Draft".to_string(),
        other => other.to_string(),
    }
}
