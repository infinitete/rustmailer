use std::future::pending;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use crate::config::AppConfig;
use crate::core::service::MailCoreService;
use crate::db::repositories::Repositories;
use crate::db::{TestDatabase, connect_pool, run_migrations};
use crate::error::{AppError, AppResult};
use crate::http;
use crate::http::state::AppState;
use crate::tls::{TlsError, TlsManager};

const TLS_RELOAD_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    admin_token_configured: bool,
}

pub async fn build_app(config: AppConfig) -> AppResult<Router> {
    Ok(build_router(AppState::without_mail_core(config)))
}

pub async fn build_runtime_app(config: AppConfig) -> AppResult<Router> {
    Ok(build_runtime_services(config).await?.http_app)
}

pub struct RuntimeServices {
    pub http_app: Router,
    pub mail_core: MailCoreService,
    pub tls_manager: Option<Arc<TlsManager>>,
}

pub struct RuntimeListeners {
    pub http: TcpListener,
    pub smtp: TcpListener,
    pub imap: TcpListener,
    pub submission: Option<TcpListener>,
    pub imaps: Option<TcpListener>,
}

pub async fn build_runtime_services(config: AppConfig) -> AppResult<RuntimeServices> {
    let pool = connect_pool(&config.database_url).await?;
    run_migrations(&pool).await?;
    let repositories = Repositories::new(pool);
    let tls_manager = load_tls_manager(config.tls_cert_dir.as_deref()).await?;
    let mut state = AppState::new(config, MailCoreService::new(repositories));
    if let Some(tls_manager) = tls_manager.as_ref() {
        state = state.with_tls_manager(tls_manager.clone());
    }

    let mail_core = state.mail_core()?.clone();

    Ok(RuntimeServices {
        http_app: build_router(state),
        mail_core,
        tls_manager,
    })
}

pub async fn load_tls_manager(cert_dir: Option<&Path>) -> AppResult<Option<Arc<TlsManager>>> {
    let Some(cert_dir) = cert_dir else {
        return Ok(None);
    };

    match TlsManager::load_from_dir(cert_dir).await {
        Ok(manager) => Ok(Some(Arc::new(manager))),
        Err(TlsError::Io(io_error)) if io_error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!(
                "tls certificate files not found under `{}`; continuing without tls listeners",
                cert_dir.display()
            );
            Ok(None)
        }
        Err(error) => Err(crate::error::AppError::CommandFailed {
            program: "tls",
            message: error.to_string(),
        }),
    }
}

pub async fn bind_runtime_listeners(
    config: &AppConfig,
    tls_enabled: bool,
) -> AppResult<RuntimeListeners> {
    let http = TcpListener::bind(config.http_socket_addr()?).await?;
    let smtp = TcpListener::bind(config.smtp_socket_addr()?).await?;
    let imap = TcpListener::bind(config.imap_socket_addr()?).await?;

    let submission = if tls_enabled {
        Some(TcpListener::bind(socket_addr(&config.host, config.submission_port)?).await?)
    } else {
        None
    };

    let imaps = if tls_enabled {
        Some(TcpListener::bind(socket_addr(&config.host, config.imaps_port)?).await?)
    } else {
        None
    };

    Ok(RuntimeListeners {
        http,
        smtp,
        imap,
        submission,
        imaps,
    })
}

pub async fn serve_runtime(listeners: RuntimeListeners, runtime: RuntimeServices) -> AppResult<()> {
    let RuntimeListeners {
        http,
        smtp,
        imap,
        submission,
        imaps,
    } = listeners;
    let RuntimeServices {
        http_app,
        mail_core,
        tls_manager,
    } = runtime;

    let smtp_core = mail_core.clone();
    let imap_core = mail_core.clone();
    let submission_core = mail_core.clone();
    let imaps_core = mail_core;
    let submission_tls = tls_manager.clone();
    let imaps_tls = tls_manager;
    let _tls_reload_task = submission_tls
        .clone()
        .map(|tls_manager| spawn_tls_reload_task(tls_manager, TLS_RELOAD_INTERVAL));

    tokio::try_join!(
        axum::serve(http, http_app),
        async move {
            crate::smtp::serve(smtp, smtp_core).await;
            Ok::<(), std::io::Error>(())
        },
        crate::imap::serve(imap, imap_core),
        async move {
            match (submission, submission_tls) {
                (Some(listener), Some(tls_manager)) => {
                    crate::smtp::serve_with_starttls(listener, submission_core, tls_manager).await;
                    Ok(())
                }
                _ => pending::<std::io::Result<()>>().await,
            }
        },
        async move {
            match (imaps, imaps_tls) {
                (Some(listener), Some(tls_manager)) => {
                    crate::imap::serve_secure(listener, imaps_core, tls_manager).await
                }
                _ => pending::<std::io::Result<()>>().await,
            }
        },
    )?;

    Ok(())
}

pub fn spawn_tls_reload_task(tls_manager: Arc<TlsManager>, interval: Duration) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;
            if let Err(error) = tls_manager.reload_if_changed().await {
                eprintln!("tls reload failed: {error}");
            }
        }
    })
}

pub async fn build_test_app() -> AppResult<Router> {
    let config = AppConfig::for_tests();
    let test_database = Arc::new(TestDatabase::new().await);
    let state = AppState::new(
        config,
        MailCoreService::new(test_database.repositories.clone()),
    )
    .with_test_database(test_database);

    Ok(build_router(state))
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health_check))
        .merge(http::router())
        .with_state(state)
}

async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        admin_token_configured: state.admin_token_configured(),
    })
}

fn socket_addr(host: &str, port: u16) -> AppResult<SocketAddr> {
    format!("{host}:{port}")
        .parse()
        .map_err(|source| AppError::InvalidSocketAddress {
            value: format!("{host}:{port}"),
            source,
        })
}
