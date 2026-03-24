use std::fs;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rcgen::generate_simple_self_signed;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};

#[tokio::test]
async fn imap_secure_listener_fetches_subject_over_tls() {
    install_rustls_crypto_provider();

    let db = server::db::TestDatabase::new().await;
    let core = server::core::service::MailCoreService::new(db.repositories.clone());
    core.provision_mailbox("example.com", "alice", "password123")
        .await
        .unwrap();
    core.receive_inbound_message(
        "alice@example.com",
        &["alice@example.com".to_string()],
        "Subject: hello\r\n\r\nbody",
    )
    .await
    .unwrap();

    let cert_dir = write_test_certificate_dir().await;
    let tls_manager = server::tls::TlsManager::load_from_dir(&cert_dir)
        .await
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(server::imap::serve_secure(
        listener,
        core,
        Arc::new(tls_manager),
    ));

    let stream = TcpStream::connect(addr).await.unwrap();
    let root_store = test_root_store(&cert_dir);
    let connector = TlsConnector::from(Arc::new(
        ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth(),
    ));
    let tls_stream = connector
        .connect(ServerName::try_from("localhost").unwrap(), stream)
        .await
        .unwrap();
    let mut reader = BufReader::new(tls_stream);

    let _ = read_lines_until(&mut reader, |line| {
        line.contains("* OK rustmailer IMAP ready")
    })
    .await;
    {
        let stream = reader.get_mut();
        stream
            .write_all(b"a1 LOGIN alice@example.com password123\r\n")
            .await
            .unwrap();
        stream.write_all(b"a2 SELECT INBOX\r\n").await.unwrap();
        stream
            .write_all(b"a3 FETCH 1 BODY[HEADER.FIELDS (SUBJECT)]\r\n")
            .await
            .unwrap();
    }

    let transcript =
        read_lines_until(&mut reader, |line| line.contains("a3 OK FETCH completed")).await;
    server.abort();

    assert!(
        transcript
            .iter()
            .any(|line| line.contains("Subject: hello"))
    );
}

fn install_rustls_crypto_provider() {
    let _ = tokio_rustls::rustls::crypto::aws_lc_rs::default_provider().install_default();
}

async fn write_test_certificate_dir() -> std::path::PathBuf {
    let cert = generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let dir = std::env::temp_dir().join(format!(
        "rustmailer-imaps-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("fullchain.pem"), cert.cert.pem()).unwrap();
    fs::write(dir.join("privkey.pem"), cert.signing_key.serialize_pem()).unwrap();
    dir
}

fn test_root_store(cert_dir: &std::path::Path) -> RootCertStore {
    let mut store = RootCertStore::empty();
    let cert_pem = fs::read_to_string(cert_dir.join("fullchain.pem")).unwrap();
    let certs = rustls_pemfile::certs(&mut cert_pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    for cert in certs {
        store.add(cert).unwrap();
    }
    store
}

async fn read_lines_until<F, R>(reader: &mut BufReader<R>, predicate: F) -> Vec<String>
where
    F: Fn(&str) -> bool,
    R: tokio::io::AsyncRead + Unpin,
{
    let mut lines = Vec::new();

    loop {
        let mut line = String::new();
        let bytes = timeout(Duration::from_secs(5), reader.read_line(&mut line))
            .await
            .unwrap()
            .unwrap();
        if bytes == 0 {
            break;
        }

        let trimmed = line.trim_end_matches(['\r', '\n']).to_string();
        let done = predicate(&trimmed);
        lines.push(trimmed);
        if done {
            break;
        }
    }

    lines
}
