use axum::Router;
use axum::http::HeaderMap;

use crate::error::{AppError, AppResult};
use crate::http::state::AppState;

pub mod routes;
pub mod state;

pub fn router() -> Router<AppState> {
    routes::auth::routes()
        .merge(routes::audit::routes())
        .merge(routes::domains::routes())
        .merge(routes::mailboxes::routes())
        .merge(routes::system::routes())
}

pub fn require_admin_token(state: &AppState, headers: &HeaderMap) -> AppResult<()> {
    let provided = headers
        .get("x-admin-token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();

    if provided != state.admin_token() {
        return Err(AppError::Unauthorized);
    }

    Ok(())
}
