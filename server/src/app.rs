use std::sync::Arc;

use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;

use crate::config::AppConfig;
use crate::error::AppResult;

#[derive(Clone)]
struct AppState {
    config: Arc<AppConfig>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    admin_token_configured: bool,
}

pub async fn build_app(config: AppConfig) -> AppResult<Router> {
    let state = AppState {
        config: Arc::new(config),
    };

    Ok(Router::new()
        .route("/healthz", get(health_check))
        .with_state(state))
}

async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        admin_token_configured: !state.config.admin_token.is_empty(),
    })
}
