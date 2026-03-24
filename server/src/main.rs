use server::app::{bind_runtime_listeners, build_runtime_services, serve_runtime};
use server::config::AppConfig;
use server::error::AppError;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let config = AppConfig::from_env()?;
    let runtime = build_runtime_services(config.clone()).await?;
    let listeners = bind_runtime_listeners(&config, runtime.tls_manager.is_some()).await?;
    serve_runtime(listeners, runtime).await
}
