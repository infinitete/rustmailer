use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::core::service::UpdateMailboxInput;
use crate::error::AppResult;
use crate::http::require_admin_token;
use crate::http::state::AppState;

#[derive(Debug, Deserialize)]
struct CreateMailboxRequest {
    domain: String,
    #[serde(alias = "localPart")]
    local_part: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct MailboxResponse {
    id: i64,
    domain_id: i64,
    local_part: String,
    email: String,
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct UpdateMailboxRequest {
    enabled: Option<bool>,
    password: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/mailboxes",
            get(list_mailboxes).post(create_mailbox),
        )
        .route(
            "/api/admin/mailboxes/{id}",
            delete(delete_mailbox).patch(update_mailbox),
        )
}

async fn list_mailboxes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<MailboxResponse>>> {
    require_admin_token(&state, &headers)?;

    let mailboxes = state.mail_core()?.list_mailboxes().await?;

    Ok(Json(
        mailboxes
            .into_iter()
            .map(|mailbox| MailboxResponse {
                id: mailbox.id,
                domain_id: mailbox.domain_id,
                local_part: mailbox.local_part,
                email: mailbox.email,
                enabled: mailbox.enabled,
            })
            .collect(),
    ))
}

async fn create_mailbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateMailboxRequest>,
) -> AppResult<(axum::http::StatusCode, Json<MailboxResponse>)> {
    require_admin_token(&state, &headers)?;

    let mailbox = state
        .mail_core()?
        .provision_mailbox(&payload.domain, &payload.local_part, &payload.password)
        .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(MailboxResponse {
            id: mailbox.id,
            domain_id: mailbox.domain_id,
            local_part: mailbox.local_part,
            email: mailbox.email,
            enabled: mailbox.enabled,
        }),
    ))
}

async fn delete_mailbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<axum::http::StatusCode> {
    require_admin_token(&state, &headers)?;
    state.mail_core()?.delete_mailbox(id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn update_mailbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateMailboxRequest>,
) -> AppResult<Json<MailboxResponse>> {
    require_admin_token(&state, &headers)?;
    let mailbox = state
        .mail_core()?
        .update_mailbox(
            id,
            UpdateMailboxInput {
                enabled: payload.enabled,
                password: payload.password,
            },
        )
        .await?;

    Ok(Json(MailboxResponse {
        id: mailbox.id,
        domain_id: mailbox.domain_id,
        local_part: mailbox.local_part,
        email: mailbox.email,
        enabled: mailbox.enabled,
    }))
}
