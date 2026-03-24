use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::BufReader;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use x509_parser::extensions::GeneralName;
use x509_parser::parse_x509_certificate;

#[derive(Debug)]
pub enum TlsError {
    Io(std::io::Error),
    LockPoisoned,
    NoPrivateKey,
    InvalidPrivateKey(std::io::Error),
    InvalidCertificate(std::io::Error),
    InvalidServerConfig(tokio_rustls::rustls::Error),
}

impl std::fmt::Display for TlsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "tls i/o error: {error}"),
            Self::LockPoisoned => write!(f, "tls state lock poisoned"),
            Self::NoPrivateKey => write!(f, "tls private key not found"),
            Self::InvalidPrivateKey(error) => write!(f, "invalid tls private key: {error}"),
            Self::InvalidCertificate(error) => write!(f, "invalid tls certificate: {error}"),
            Self::InvalidServerConfig(error) => write!(f, "invalid tls server config: {error}"),
        }
    }
}

impl std::error::Error for TlsError {}

impl From<std::io::Error> for TlsError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug, Clone)]
pub struct CertificateSnapshot {
    pub cert_pem: Vec<u8>,
    pub key_pem: Vec<u8>,
    pub serial: u64,
    pub subject_names: Vec<String>,
    pub expires_at: Option<String>,
    pub last_reloaded_at: Option<String>,
    server_config: Arc<ServerConfig>,
    fingerprint: u64,
}

#[derive(Clone)]
pub struct TlsManager {
    cert_dir: PathBuf,
    state: Arc<RwLock<CertificateSnapshot>>,
}

impl TlsManager {
    pub async fn load_from_dir(cert_dir: &Path) -> Result<Self, TlsError> {
        install_crypto_provider();
        let loaded = load_bundle(cert_dir)?;

        Ok(Self {
            cert_dir: cert_dir.to_path_buf(),
            state: Arc::new(RwLock::new(CertificateSnapshot {
                cert_pem: loaded.cert_pem,
                key_pem: loaded.key_pem,
                serial: 1,
                subject_names: loaded.subject_names,
                expires_at: loaded.expires_at,
                last_reloaded_at: None,
                server_config: loaded.server_config,
                fingerprint: loaded.fingerprint,
            })),
        })
    }

    pub async fn current_serial(&self) -> u64 {
        match self.state.read() {
            Ok(guard) => guard.serial,
            Err(_) => 0,
        }
    }

    pub async fn subject_names(&self) -> Vec<String> {
        match self.state.read() {
            Ok(guard) => guard.subject_names.clone(),
            Err(_) => Vec::new(),
        }
    }

    pub async fn expires_at(&self) -> Option<String> {
        match self.state.read() {
            Ok(guard) => guard.expires_at.clone(),
            Err(_) => None,
        }
    }

    pub async fn last_reloaded_at(&self) -> Option<String> {
        match self.state.read() {
            Ok(guard) => guard.last_reloaded_at.clone(),
            Err(_) => None,
        }
    }

    pub async fn reload_if_changed(&self) -> Result<(), TlsError> {
        let loaded = load_bundle(&self.cert_dir)?;
        let mut state = self.state.write().map_err(|_| TlsError::LockPoisoned)?;

        if loaded.fingerprint != state.fingerprint {
            state.fingerprint = loaded.fingerprint;
            state.cert_pem = loaded.cert_pem;
            state.key_pem = loaded.key_pem;
            state.subject_names = loaded.subject_names;
            state.expires_at = loaded.expires_at;
            state.server_config = loaded.server_config;
            state.serial = state.serial.saturating_add(1);
            state.last_reloaded_at = Some(reload_timestamp());
        }

        Ok(())
    }

    pub fn server_config(&self) -> Result<Arc<ServerConfig>, TlsError> {
        self.state
            .read()
            .map(|guard| guard.server_config.clone())
            .map_err(|_| TlsError::LockPoisoned)
    }
}

fn install_crypto_provider() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let _ = tokio_rustls::rustls::crypto::aws_lc_rs::default_provider().install_default();
    });
}

struct LoadedBundle {
    cert_pem: Vec<u8>,
    key_pem: Vec<u8>,
    subject_names: Vec<String>,
    expires_at: Option<String>,
    server_config: Arc<ServerConfig>,
    fingerprint: u64,
}

fn load_bundle(cert_dir: &Path) -> Result<LoadedBundle, TlsError> {
    let cert_path = cert_dir.join("fullchain.pem");
    let key_path = cert_dir.join("privkey.pem");
    let cert_pem = fs::read(&cert_path)?;
    let key_pem = fs::read(&key_path)?;
    let cert_chain = parse_certificates(&cert_pem)?;
    let (subject_names, expires_at) = parse_certificate_metadata(&cert_chain)?;
    let private_key = parse_private_key(&key_pem)?;
    let server_config = Arc::new(
        ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, private_key)
            .map_err(TlsError::InvalidServerConfig)?,
    );
    let cert_meta = fs::metadata(&cert_path)?;
    let key_meta = fs::metadata(&key_path)?;

    let fingerprint = fingerprint_for(
        &cert_pem,
        &key_pem,
        cert_meta.modified().ok(),
        key_meta.modified().ok(),
    );

    Ok(LoadedBundle {
        cert_pem,
        key_pem,
        subject_names,
        expires_at,
        server_config,
        fingerprint,
    })
}

fn parse_certificates(cert_pem: &[u8]) -> Result<Vec<CertificateDer<'static>>, TlsError> {
    let mut reader = BufReader::new(cert_pem);
    rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(TlsError::InvalidCertificate)
}

fn parse_private_key(key_pem: &[u8]) -> Result<PrivateKeyDer<'static>, TlsError> {
    let mut reader = BufReader::new(key_pem);
    rustls_pemfile::private_key(&mut reader)
        .map_err(TlsError::InvalidPrivateKey)?
        .ok_or(TlsError::NoPrivateKey)
}

fn parse_certificate_metadata(
    cert_chain: &[CertificateDer<'static>],
) -> Result<(Vec<String>, Option<String>), TlsError> {
    let Some(leaf) = cert_chain.first() else {
        return Ok((Vec::new(), None));
    };

    let (_, cert) = parse_x509_certificate(leaf.as_ref()).map_err(|error| {
        TlsError::InvalidCertificate(std::io::Error::new(
            ErrorKind::InvalidData,
            error.to_string(),
        ))
    })?;

    let mut names = Vec::new();
    if let Ok(Some(extension)) = cert.subject_alternative_name() {
        for name in &extension.value.general_names {
            if let GeneralName::DNSName(value) = name {
                names.push(value.to_string());
            }
        }
    }

    if names.is_empty() {
        for name in cert.subject().iter_common_name() {
            if let Ok(value) = name.as_str() {
                names.push(value.to_string());
            }
        }
    }

    Ok((names, Some(cert.validity().not_after.to_string())))
}

fn fingerprint_for(
    cert_pem: &[u8],
    key_pem: &[u8],
    cert_modified: Option<SystemTime>,
    key_modified: Option<SystemTime>,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    cert_pem.hash(&mut hasher);
    key_pem.hash(&mut hasher);
    cert_modified.hash(&mut hasher);
    key_modified.hash(&mut hasher);
    hasher.finish()
}

fn reload_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("unix:{}.{}", now.as_secs(), now.subsec_millis())
}

pub mod test_support {
    use std::path::Path;

    use crate::tls::TlsError;

    pub async fn swap_certificate_fixture(cert_dir: &Path) -> Result<(), TlsError> {
        let base_cert = cert_dir.join("fullchain.base.pem");
        let base_key = cert_dir.join("privkey.base.pem");
        let next_cert = cert_dir.join("fullchain.next.pem");
        let next_key = cert_dir.join("privkey.next.pem");
        let cert = cert_dir.join("fullchain.pem");
        let key = cert_dir.join("privkey.pem");
        let current_cert = std::fs::read(&cert)?;
        let next_cert_bytes = std::fs::read(&next_cert)?;

        if current_cert == next_cert_bytes {
            std::fs::copy(base_cert, cert)?;
            std::fs::copy(base_key, key)?;
        } else {
            std::fs::copy(next_cert, cert)?;
            std::fs::copy(next_key, key)?;
        }

        Ok(())
    }
}
