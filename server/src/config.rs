use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use serde::Deserialize;

use crate::error::AppError;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub smtp_port: u16,
    pub submission_port: u16,
    pub imap_port: u16,
    pub imaps_port: u16,
    pub database_url: String,
    pub admin_token: String,
    pub tls_cert_dir: Option<PathBuf>,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, AppError> {
        Ok(Self {
            host: env::var("APP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: optional_port("APP_PORT")?.unwrap_or(3000),
            smtp_port: optional_port("SMTP_PORT")?.unwrap_or(2525),
            submission_port: optional_port("SUBMISSION_PORT")?.unwrap_or(587),
            imap_port: optional_port("IMAP_PORT")?.unwrap_or(1143),
            imaps_port: optional_port("IMAPS_PORT")?.unwrap_or(993),
            database_url: required_env("DATABASE_URL")?,
            admin_token: required_env("ADMIN_TOKEN")?,
            tls_cert_dir: env::var("TLS_CERT_DIR").ok().map(PathBuf::from),
        })
    }

    pub fn for_tests() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 0,
            smtp_port: 0,
            submission_port: 0,
            imap_port: 0,
            imaps_port: 0,
            database_url: "postgres://localhost/rustmailer_test".to_string(),
            admin_token: "test-admin-token".to_string(),
            tls_cert_dir: None,
        }
    }

    pub fn http_socket_addr(&self) -> Result<SocketAddr, AppError> {
        parse_socket_addr(&self.host, self.port)
    }

    pub fn smtp_socket_addr(&self) -> Result<SocketAddr, AppError> {
        parse_socket_addr(&self.host, self.smtp_port)
    }

    pub fn submission_socket_addr(&self) -> Result<SocketAddr, AppError> {
        parse_socket_addr(&self.host, self.submission_port)
    }

    pub fn imap_socket_addr(&self) -> Result<SocketAddr, AppError> {
        parse_socket_addr(&self.host, self.imap_port)
    }

    pub fn imaps_socket_addr(&self) -> Result<SocketAddr, AppError> {
        parse_socket_addr(&self.host, self.imaps_port)
    }
}

fn optional_port(key: &'static str) -> Result<Option<u16>, AppError> {
    env::var(key)
        .ok()
        .map(|value| {
            value
                .parse()
                .map_err(|source| AppError::InvalidPort { value, source })
        })
        .transpose()
}

fn parse_socket_addr(host: &str, port: u16) -> Result<SocketAddr, AppError> {
    format!("{host}:{port}")
        .parse()
        .map_err(|source| AppError::InvalidSocketAddress {
            value: format!("{host}:{port}"),
            source,
        })
}

fn required_env(key: &'static str) -> Result<String, AppError> {
    let value = env::var(key).map_err(|_| AppError::MissingRequiredEnv { key })?;
    if value.trim().is_empty() {
        return Err(AppError::EmptyRequiredEnv { key });
    }

    Ok(value)
}
