use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::core::service::UpdateDomainInput;
use crate::error::AppResult;
use crate::http::require_admin_token;
use crate::http::state::AppState;

#[derive(Debug, Deserialize)]
struct CreateDomainRequest {
    domain: String,
}

#[derive(Debug, Serialize)]
struct DomainResponse {
    id: i64,
    name: String,
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct UpdateDomainRequest {
    name: Option<String>,
    enabled: Option<bool>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/domains", get(list_domains).post(create_domain))
        .route(
            "/api/admin/domains/{id}",
            delete(delete_domain).patch(update_domain),
        )
}

async fn list_domains(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<DomainResponse>>> {
    require_admin_token(&state, &headers)?;

    let domains = state.mail_core()?.list_domains().await?;

    Ok(Json(
        domains
            .into_iter()
            .map(|domain| DomainResponse {
                id: domain.id,
                name: domain.name,
                enabled: domain.enabled,
            })
            .collect(),
    ))
}

async fn create_domain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateDomainRequest>,
) -> AppResult<(axum::http::StatusCode, Json<DomainResponse>)> {
    require_admin_token(&state, &headers)?;

    let domain = state.mail_core()?.create_domain(&payload.domain).await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(DomainResponse {
            id: domain.id,
            name: domain.name,
            enabled: domain.enabled,
        }),
    ))
}

async fn delete_domain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<axum::http::StatusCode> {
    require_admin_token(&state, &headers)?;
    state.mail_core()?.delete_domain(id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn update_domain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateDomainRequest>,
) -> AppResult<Json<DomainResponse>> {
    require_admin_token(&state, &headers)?;
    let domain = state
        .mail_core()?
        .update_domain(
            id,
            UpdateDomainInput {
                name: payload.name,
                enabled: payload.enabled,
            },
        )
        .await?;

    Ok(Json(DomainResponse {
        id: domain.id,
        name: domain.name,
        enabled: domain.enabled,
    }))
}
