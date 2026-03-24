use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::error::AppResult;
use crate::http::require_admin_token;
use crate::http::state::AppState;

#[derive(Debug, Serialize)]
struct AdminHealthResponse {
    status: &'static str,
    admin_token_configured: bool,
}

#[derive(Debug, Serialize)]
struct CertificateStatusResponse {
    status: &'static str,
    subject_names: Vec<String>,
    expires_at: Option<String>,
    last_reloaded_at: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/system/health", get(health))
        .route("/api/admin/system/certificates", get(certificates))
}

async fn health(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<AdminHealthResponse>> {
    require_admin_token(&state, &headers)?;

    Ok(Json(AdminHealthResponse {
        status: "ok",
        admin_token_configured: state.admin_token_configured(),
    }))
}

async fn certificates(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<CertificateStatusResponse>> {
    require_admin_token(&state, &headers)?;

    if let Some(tls_manager) = state.tls_manager() {
        return Ok(Json(CertificateStatusResponse {
            status: "loaded",
            subject_names: tls_manager.subject_names().await,
            expires_at: tls_manager.expires_at().await,
            last_reloaded_at: tls_manager.last_reloaded_at().await,
        }));
    }

    Ok(Json(CertificateStatusResponse {
        status: "not_configured",
        subject_names: Vec::new(),
        expires_at: None,
        last_reloaded_at: None,
    }))
}
