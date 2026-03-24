use std::sync::Arc;

use crate::config::AppConfig;
use crate::core::service::MailCoreService;
use crate::db::TestDatabase;
use crate::error::{AppError, AppResult};
use crate::tls::TlsManager;

#[derive(Clone)]
pub struct AppState {
    config: Arc<AppConfig>,
    mail_core: Option<MailCoreService>,
    tls_manager: Option<Arc<TlsManager>>,
    _test_database: Option<Arc<TestDatabase>>,
}

impl AppState {
    pub fn new(config: AppConfig, mail_core: MailCoreService) -> Self {
        Self {
            config: Arc::new(config),
            mail_core: Some(mail_core),
            tls_manager: None,
            _test_database: None,
        }
    }

    pub fn without_mail_core(config: AppConfig) -> Self {
        Self {
            config: Arc::new(config),
            mail_core: None,
            tls_manager: None,
            _test_database: None,
        }
    }

    pub fn with_tls_manager(mut self, tls_manager: Arc<TlsManager>) -> Self {
        self.tls_manager = Some(tls_manager);
        self
    }

    pub fn with_test_database(mut self, test_database: Arc<TestDatabase>) -> Self {
        self._test_database = Some(test_database);
        self
    }

    pub fn admin_token(&self) -> &str {
        &self.config.admin_token
    }

    pub fn admin_token_configured(&self) -> bool {
        !self.config.admin_token.is_empty()
    }

    pub fn mail_core(&self) -> AppResult<&MailCoreService> {
        self.mail_core
            .as_ref()
            .ok_or(AppError::UninitializedComponent {
                component: "mail_core",
            })
    }

    pub fn tls_manager(&self) -> Option<&Arc<TlsManager>> {
        self.tls_manager.as_ref()
    }
}
