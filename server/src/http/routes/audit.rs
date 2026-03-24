use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::error::AppResult;
use crate::http::require_admin_token;
use crate::http::state::AppState;

#[derive(Debug, Serialize)]
struct AuditLogResponse {
    id: i64,
    actor: String,
    action: String,
    details: serde_json::Value,
    created_at: String,
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/admin/audit-logs", get(list_audit_logs))
}

async fn list_audit_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<AuditLogResponse>>> {
    require_admin_token(&state, &headers)?;
    let logs = state.mail_core()?.list_audit_logs(100).await?;

    Ok(Json(
        logs.into_iter()
            .map(|entry| AuditLogResponse {
                id: entry.id,
                actor: entry.actor,
                action: entry.action,
                details: entry.details,
                created_at: entry.created_at,
            })
            .collect(),
    ))
}
