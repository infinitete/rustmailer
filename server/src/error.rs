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
    #[error("invalid domain name `{value}`")]
    InvalidDomainName { value: String },
    #[error("invalid mailbox address `{value}`")]
    InvalidMailboxAddress { value: String },
    #[error("authentication failed")]
    AuthError,
    #[error("mailbox `{email}` not found")]
    MailboxNotFound { email: String },
    #[error("mailbox `{email}` is disabled")]
    MailboxDisabled { email: String },
    #[error("mailbox id `{id}` not found")]
    MailboxIdNotFound { id: i64 },
    #[error("domain id `{id}` not found")]
    DomainNotFound { id: i64 },
    #[error("domain `{domain}` is disabled")]
    DomainDisabled { domain: String },
    #[error("folder `{name}` not found")]
    FolderNotFound { name: String },
    #[error("unauthorized")]
    Unauthorized,
    #[error("external command `{program}` failed: {message}")]
    CommandFailed {
        program: &'static str,
        message: String,
    },
    #[error("invalid docker port output `{value}`")]
    InvalidDockerPortOutput { value: String },
    #[error("component `{component}` is not initialized")]
    UninitializedComponent { component: &'static str },
    #[error("i/o error")]
    Io(#[from] std::io::Error),
    #[error("database error")]
    Sqlx(#[from] sqlx::Error),
    #[error("database migration error")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::InvalidDomainName { .. } | Self::InvalidMailboxAddress { .. } => {
                StatusCode::BAD_REQUEST
            }
            Self::AuthError | Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::MailboxDisabled { .. } | Self::DomainDisabled { .. } => StatusCode::FORBIDDEN,
            Self::MailboxNotFound { .. }
            | Self::MailboxIdNotFound { .. }
            | Self::DomainNotFound { .. }
            | Self::FolderNotFound { .. } => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, self.to_string()).into_response()
    }
}
