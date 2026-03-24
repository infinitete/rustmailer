use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::num::ParseIntError;
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("missing required environment variable `{key}`")]
    MissingRequiredEnv { key: &'static str },
    #[error("environment variable `{key}` must not be empty")]
    EmptyRequiredEnv { key: &'static str },
    #[error("invalid APP_PORT value `{value}`")]
    InvalidPort {
        value: String,
        #[source]
        source: ParseIntError,
    },
    #[error("invalid socket address `{value}`")]
    InvalidSocketAddress {
        value: String,
        #[source]
        source: std::net::AddrParseError,
    },
    #[error("i/o error")]
    Io(#[from] std::io::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
    }
}
