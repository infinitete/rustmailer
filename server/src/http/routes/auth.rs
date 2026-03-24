use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::http::state::AppState;

#[derive(Debug, Deserialize)]
struct LoginRequest {
    token: String,
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    authenticated: bool,
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/admin/login", post(login))
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> AppResult<Json<LoginResponse>> {
    let provided = headers
        .get("x-admin-token")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .unwrap_or(&payload.token);

    if provided != state.admin_token() {
        return Err(AppError::Unauthorized);
    }

    Ok(Json(LoginResponse {
        authenticated: true,
    }))
}
