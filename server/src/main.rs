use server::app::build_app;
use server::config::AppConfig;
use server::error::AppError;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let config = AppConfig::from_env()?;
    let listener = tokio::net::TcpListener::bind(config.socket_addr()?).await?;
    let app = build_app(config).await?;

    axum::serve(listener, app).await?;
    Ok(())
}
