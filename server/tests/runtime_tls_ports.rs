use std::fs;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rcgen::generate_simple_self_signed;
use server::app::{bind_runtime_listeners, build_runtime_services, serve_runtime};
use server::config::AppConfig;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};

#[tokio::test]
async fn runtime_binds_submission_and_imaps_ports_with_tls() {
    let db = server::db::TestDatabase::new().await;
    let cert_dir = write_test_certificate_dir().await;

    let mut config = AppConfig::for_tests();
    config.database_url = db.database_url.clone();
    config.tls_cert_dir = Some(cert_dir.clone());
    config.port = 0;
    config.smtp_port = 0;
    config.submission_port = 0;
    config.imap_port = 0;
    config.imaps_port = 0;

    let runtime = build_runtime_services(config.clone()).await.unwrap();
    runtime
        .mail_core
        .provision_mailbox("example.com", "alice", "password123")
        .await
        .unwrap();

    let listeners = bind_runtime_listeners(&config, runtime.tls_manager.is_some())
        .await
        .unwrap();
    let submission_addr = listeners.submission.as_ref().unwrap().local_addr().unwrap();
    let imaps_addr = listeners.imaps.as_ref().unwrap().local_addr().unwrap();

    let server = tokio::spawn(serve_runtime(listeners, runtime));

    let stream = TcpStream::connect(submission_addr).await.unwrap();
    let (reader_half, mut writer_half) = stream.into_split();
    let mut reader = BufReader::new(reader_half);

    let _ = read_lines_until(&mut reader, |line| line.contains("220")).await;
    writer_half.write_all(b"EHLO localhost\r\n").await.unwrap();
    let _ = read_lines_until(&mut reader, |line| line == "250 DATA").await;
    writer_half.write_all(b"STARTTLS\r\n").await.unwrap();
    let _ = read_lines_until(&mut reader, |line| line.contains("220 Ready to start TLS")).await;

    let connector = test_connector(&cert_dir);
    let stream = reader.into_inner().reunite(writer_half).unwrap();
    let mut tls_stream = connector
        .connect(ServerName::try_from("localhost").unwrap(), stream)
        .await
        .unwrap();
    tls_stream
        .write_all(b"AUTH PLAIN AGFsaWNlQGV4YW1wbGUuY29tAHBhc3N3b3JkMTIz\r\nQUIT\r\n")
        .await
        .unwrap();
    let mut tls_reader = BufReader::new(tls_stream);
    let smtp_transcript = read_lines_until(&mut tls_reader, |line| line.contains("221 Bye")).await;

    let imaps_stream = TcpStream::connect(imaps_addr).await.unwrap();
    let tls_stream = connector
        .connect(ServerName::try_from("localhost").unwrap(), imaps_stream)
        .await
        .unwrap();
    let mut imap_reader = BufReader::new(tls_stream);
    let greeting = read_lines_until(&mut imap_reader, |line| {
        line.contains("* OK rustmailer IMAP ready")
    })
    .await;

    server.abort();

    assert!(
        smtp_transcript
            .iter()
            .any(|line| line.contains("235 Authentication successful"))
    );
    assert!(
        greeting
            .iter()
            .any(|line| line.contains("* OK rustmailer IMAP ready"))
    );
}

async fn write_test_certificate_dir() -> std::path::PathBuf {
    let cert = generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let dir = std::env::temp_dir().join(format!(
        "rustmailer-runtime-tls-{}",
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

fn test_connector(cert_dir: &std::path::Path) -> TlsConnector {
    let mut store = RootCertStore::empty();
    let cert_pem = fs::read_to_string(cert_dir.join("fullchain.pem")).unwrap();
    let certs = rustls_pemfile::certs(&mut cert_pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    for cert in certs {
        store.add(cert).unwrap();
    }

    TlsConnector::from(Arc::new(
        ClientConfig::builder()
            .with_root_certificates(store)
            .with_no_client_auth(),
    ))
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
