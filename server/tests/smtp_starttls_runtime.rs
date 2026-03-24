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
async fn smtp_starttls_upgrades_plain_connection() {
    let db = server::db::TestDatabase::new().await;
    let core = server::core::service::MailCoreService::new(db.repositories.clone());
    core.provision_mailbox("example.com", "alice", "password123")
        .await
        .unwrap();

    let cert_dir = write_test_certificate_dir().await;
    let tls_manager = server::tls::TlsManager::load_from_dir(&cert_dir)
        .await
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(server::smtp::serve_with_starttls(
        listener,
        core,
        Arc::new(tls_manager),
    ));

    let stream = TcpStream::connect(addr).await.unwrap();
    let (reader_half, mut writer_half) = stream.into_split();
    let mut reader = BufReader::new(reader_half);

    let _ = read_lines_until(&mut reader, |line| line.contains("220")).await;
    writer_half.write_all(b"EHLO localhost\r\n").await.unwrap();
    let ehlo_transcript = read_lines_until(&mut reader, |line| line == "250 DATA").await;
    assert!(ehlo_transcript.iter().any(|line| line.contains("STARTTLS")));
    writer_half.write_all(b"STARTTLS\r\n").await.unwrap();
    let _ = read_lines_until(&mut reader, |line| line.contains("220 Ready to start TLS")).await;

    let root_store = test_root_store(&cert_dir);
    let connector = TlsConnector::from(Arc::new(
        ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth(),
    ));

    let stream = reader.into_inner().reunite(writer_half).unwrap();
    let mut tls_stream = connector
        .connect(ServerName::try_from("localhost").unwrap(), stream)
        .await
        .unwrap();

    tls_stream.write_all(b"EHLO localhost\r\n").await.unwrap();
    tls_stream
        .write_all(b"AUTH PLAIN AGFsaWNlQGV4YW1wbGUuY29tAHBhc3N3b3JkMTIz\r\nQUIT\r\n")
        .await
        .unwrap();

    let mut tls_reader = BufReader::new(tls_stream);
    let transcript = read_lines_until(&mut tls_reader, |line| line.contains("221 Bye")).await;
    server.abort();

    assert!(
        transcript
            .iter()
            .any(|line| line.contains("235 Authentication successful"))
    );
}

#[tokio::test]
async fn submission_listener_requires_authentication_before_mail_transaction() {
    let db = server::db::TestDatabase::new().await;
    let core = server::core::service::MailCoreService::new(db.repositories.clone());
    core.provision_mailbox("example.com", "alice", "password123")
        .await
        .unwrap();

    let cert_dir = write_test_certificate_dir().await;
    let tls_manager = server::tls::TlsManager::load_from_dir(&cert_dir)
        .await
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(server::smtp::serve_with_starttls(
        listener,
        core,
        Arc::new(tls_manager),
    ));

    let stream = TcpStream::connect(addr).await.unwrap();
    let (reader_half, mut writer_half) = stream.into_split();
    let mut reader = BufReader::new(reader_half);

    let _ = read_lines_until(&mut reader, |line| line.contains("220")).await;
    writer_half.write_all(b"EHLO localhost\r\n").await.unwrap();
    let _ = read_lines_until(&mut reader, |line| line == "250 DATA").await;
    writer_half
        .write_all(b"MAIL FROM:<alice@example.com>\r\nQUIT\r\n")
        .await
        .unwrap();

    let transcript = read_lines_until(&mut reader, |line| line.contains("221 Bye")).await;
    server.abort();

    assert!(
        transcript
            .iter()
            .any(|line| line == "530 Authentication required")
    );
}

#[tokio::test]
async fn starttls_upgrade_resets_submission_auth_state() {
    let db = server::db::TestDatabase::new().await;
    let core = server::core::service::MailCoreService::new(db.repositories.clone());
    core.provision_mailbox("example.com", "alice", "password123")
        .await
        .unwrap();

    let cert_dir = write_test_certificate_dir().await;
    let tls_manager = server::tls::TlsManager::load_from_dir(&cert_dir)
        .await
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(server::smtp::serve_with_starttls(
        listener,
        core,
        Arc::new(tls_manager),
    ));

    let stream = TcpStream::connect(addr).await.unwrap();
    let (reader_half, mut writer_half) = stream.into_split();
    let mut reader = BufReader::new(reader_half);

    let _ = read_lines_until(&mut reader, |line| line.contains("220")).await;
    writer_half.write_all(b"EHLO localhost\r\n").await.unwrap();
    let _ = read_lines_until(&mut reader, |line| line == "250 DATA").await;
    writer_half
        .write_all(b"AUTH PLAIN AGFsaWNlQGV4YW1wbGUuY29tAHBhc3N3b3JkMTIz\r\n")
        .await
        .unwrap();
    let _ = read_lines_until(&mut reader, |line| {
        line.contains("235 Authentication successful")
    })
    .await;
    writer_half.write_all(b"STARTTLS\r\n").await.unwrap();
    let _ = read_lines_until(&mut reader, |line| line.contains("220 Ready to start TLS")).await;

    let root_store = test_root_store(&cert_dir);
    let connector = TlsConnector::from(Arc::new(
        ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth(),
    ));

    let stream = reader.into_inner().reunite(writer_half).unwrap();
    let mut tls_stream = connector
        .connect(ServerName::try_from("localhost").unwrap(), stream)
        .await
        .unwrap();

    tls_stream
        .write_all(b"MAIL FROM:<alice@example.com>\r\nQUIT\r\n")
        .await
        .unwrap();

    let mut tls_reader = BufReader::new(tls_stream);
    let transcript = read_lines_until(&mut tls_reader, |line| line.contains("221 Bye")).await;
    server.abort();

    assert!(
        transcript
            .iter()
            .any(|line| line == "530 Authentication required")
    );
}

async fn write_test_certificate_dir() -> std::path::PathBuf {
    let cert = generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let dir = std::env::temp_dir().join(format!(
        "rustmailer-starttls-{}",
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
