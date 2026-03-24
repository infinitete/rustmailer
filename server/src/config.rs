use std::env;
use std::net::SocketAddr;

use serde::Deserialize;

use crate::error::AppError;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub admin_token: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, AppError> {
        Ok(Self {
            host: env::var("APP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: env::var("APP_PORT")
                .ok()
                .map(|value| value.parse())
                .transpose()
                .map_err(|source| AppError::InvalidPort {
                    value: env::var("APP_PORT").unwrap_or_default(),
                    source,
                })?
                .unwrap_or(3000),
            database_url: required_env("DATABASE_URL")?,
            admin_token: required_env("ADMIN_TOKEN")?,
        })
    }

    pub fn for_tests() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 0,
            database_url: "postgres://localhost/rustmailer_test".to_string(),
            admin_token: "test-admin-token".to_string(),
        }
    }

    pub fn socket_addr(&self) -> Result<SocketAddr, AppError> {
        format!("{}:{}", self.host, self.port)
            .parse()
            .map_err(|source| AppError::InvalidSocketAddress {
                value: format!("{}:{}", self.host, self.port),
                source,
            })
    }
}

fn required_env(key: &'static str) -> Result<String, AppError> {
    let value = env::var(key).map_err(|_| AppError::MissingRequiredEnv { key })?;
    if value.trim().is_empty() {
        return Err(AppError::EmptyRequiredEnv { key });
    }

    Ok(value)
}
