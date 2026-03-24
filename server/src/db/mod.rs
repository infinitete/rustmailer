use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sqlx::PgPool;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use tokio::time::sleep;

use crate::db::repositories::Repositories;
use crate::error::{AppError, AppResult};

pub mod models;
pub mod repositories;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub async fn connect_pool(database_url: &str) -> AppResult<PgPool> {
    Ok(PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?)
}

pub async fn run_migrations(pool: &PgPool) -> AppResult<()> {
    MIGRATOR.run(pool).await?;
    Ok(())
}

pub struct TestDatabase {
    pub pool: PgPool,
    pub repositories: Repositories,
    pub database_url: String,
    _container: TestPostgresContainer,
}

impl TestDatabase {
    pub async fn new() -> Self {
        Self::try_new().await.unwrap()
    }

    async fn try_new() -> AppResult<Self> {
        let container = TestPostgresContainer::start()?;
        let pool = wait_for_database(&container.database_url).await?;
        run_migrations(&pool).await?;
        let database_url = container.database_url.clone();

        Ok(Self {
            repositories: Repositories::new(pool.clone()),
            pool,
            database_url,
            _container: container,
        })
    }
}

struct TestPostgresContainer {
    id: String,
    database_url: String,
}

impl TestPostgresContainer {
    fn start() -> AppResult<Self> {
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let name = format!("rustmailer-test-{}-{unique_suffix}", std::process::id());
        let image = "postgres:16-alpine";

        let container_id = run_docker_command(&[
            "run",
            "--rm",
            "-d",
            "--name",
            &name,
            "-e",
            "POSTGRES_USER=postgres",
            "-e",
            "POSTGRES_PASSWORD=postgres",
            "-e",
            "POSTGRES_DB=rustmailer_test",
            "-P",
            image,
        ])?;
        let host_port = docker_host_port(&container_id)?;

        Ok(Self {
            id: container_id,
            database_url: format!(
                "postgres://postgres:postgres@127.0.0.1:{host_port}/rustmailer_test"
            ),
        })
    }
}

impl Drop for TestPostgresContainer {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.id])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

async fn wait_for_database(database_url: &str) -> AppResult<PgPool> {
    let mut last_error = None;

    for _ in 0..40 {
        match connect_pool(database_url).await {
            Ok(pool) => return Ok(pool),
            Err(error) => {
                last_error = Some(error);
                sleep(Duration::from_millis(250)).await;
            }
        }
    }

    Err(last_error.unwrap_or(AppError::CommandFailed {
        program: "docker",
        message: "postgres container did not become ready".to_string(),
    }))
}

fn docker_host_port(container_id: &str) -> AppResult<u16> {
    let output = run_docker_command(&["port", container_id, "5432/tcp"])?;
    let port = output
        .rsplit_once(':')
        .and_then(|(_, port)| port.parse::<u16>().ok())
        .ok_or_else(|| AppError::InvalidDockerPortOutput { value: output })?;

    Ok(port)
}

fn run_docker_command(args: &[&str]) -> AppResult<String> {
    let output = Command::new("docker").args(args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(AppError::CommandFailed {
            program: "docker",
            message: if stderr.is_empty() {
                format!("command {:?} exited with status {}", args, output.status)
            } else {
                stderr
            },
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
