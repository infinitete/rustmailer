use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

#[tokio::test]
async fn smtp_listener_accepts_message_over_tcp() {
    let db = server::db::TestDatabase::new().await;
    let core = server::core::service::MailCoreService::new(db.repositories.clone());
    core.provision_mailbox("example.com", "alice", "password123")
        .await
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(server::smtp::serve(listener, core.clone()));

    let stream = TcpStream::connect(addr).await.unwrap();
    let (reader_half, mut writer_half) = stream.into_split();
    let mut reader = BufReader::new(reader_half);

    writer_half.write_all(b"EHLO localhost\r\n").await.unwrap();
    writer_half
        .write_all(b"AUTH PLAIN AGFsaWNlQGV4YW1wbGUuY29tAHBhc3N3b3JkMTIz\r\n")
        .await
        .unwrap();
    writer_half
        .write_all(b"MAIL FROM:<alice@example.com>\r\n")
        .await
        .unwrap();
    writer_half
        .write_all(b"RCPT TO:<alice@example.com>\r\n")
        .await
        .unwrap();
    writer_half.write_all(b"DATA\r\n").await.unwrap();
    writer_half
        .write_all(b"Subject: hello\r\n\r\nbody\r\n.\r\nQUIT\r\n")
        .await
        .unwrap();

    let transcript = read_lines_until(&mut reader, |line| line.contains("221 Bye")).await;
    server.abort();

    assert!(
        transcript
            .iter()
            .any(|line| line.contains("250 Message accepted"))
    );
}

async fn read_lines_until<F>(
    reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
    predicate: F,
) -> Vec<String>
where
    F: Fn(&str) -> bool,
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
