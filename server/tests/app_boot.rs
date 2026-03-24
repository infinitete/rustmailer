use std::env;
use std::sync::{Mutex, OnceLock};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use server::app::build_app;
use server::config::AppConfig;
use tower::ServiceExt;

fn env_lock() -> &'static Mutex<()> {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    ENV_LOCK.get_or_init(|| Mutex::new(()))
}

fn lock_env() -> std::sync::MutexGuard<'static, ()> {
    env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct EnvVarGuard {
    key: &'static str,
    original: Option<String>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let original = env::var(key).ok();
        unsafe {
            env::set_var(key, value);
        }
        Self { key, original }
    }

    fn unset(key: &'static str) -> Self {
        let original = env::var(key).ok();
        unsafe {
            env::remove_var(key);
        }
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        unsafe {
            if let Some(value) = &self.original {
                env::set_var(self.key, value);
            } else {
                env::remove_var(self.key);
            }
        }
    }
}

#[tokio::test]
async fn builds_application_from_test_config() {
    let config = AppConfig::for_tests();
    let app = build_app(config).await;
    assert!(app.is_ok());
}

#[test]
fn rejects_missing_admin_token() {
    let _guard = lock_env();
    let _database_url = EnvVarGuard::set("DATABASE_URL", "postgres://localhost/rustmailer_test");
    let _admin_token = EnvVarGuard::unset("ADMIN_TOKEN");

    let error = AppConfig::from_env().unwrap_err();

    assert_eq!(
        error.to_string(),
        "missing required environment variable `ADMIN_TOKEN`"
    );
}

#[test]
fn rejects_empty_admin_token() {
    let _guard = lock_env();
    let _database_url = EnvVarGuard::set("DATABASE_URL", "postgres://localhost/rustmailer_test");
    let _admin_token = EnvVarGuard::set("ADMIN_TOKEN", "");

    let error = AppConfig::from_env().unwrap_err();

    assert_eq!(
        error.to_string(),
        "environment variable `ADMIN_TOKEN` must not be empty"
    );
}

#[test]
fn rejects_missing_database_url() {
    let _guard = lock_env();
    let _database_url = EnvVarGuard::unset("DATABASE_URL");
    let _admin_token = EnvVarGuard::set("ADMIN_TOKEN", "test-admin-token");

    let error = AppConfig::from_env().unwrap_err();

    assert_eq!(
        error.to_string(),
        "missing required environment variable `DATABASE_URL`"
    );
}

#[tokio::test]
async fn serves_health_check() {
    let app = build_app(AppConfig::for_tests()).await.unwrap();

    let response = app
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["admin_token_configured"], true);
}
