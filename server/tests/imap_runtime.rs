use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

#[tokio::test]
async fn imap_listener_fetches_subject_over_tcp() {
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

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(server::imap::serve(listener, core.clone()));

    let stream = TcpStream::connect(addr).await.unwrap();
    let (reader_half, mut writer_half) = stream.into_split();
    let mut reader = BufReader::new(reader_half);

    writer_half
        .write_all(b"a1 LOGIN alice@example.com password123\r\n")
        .await
        .unwrap();
    writer_half.write_all(b"a2 SELECT INBOX\r\n").await.unwrap();
    writer_half
        .write_all(b"a3 FETCH 1 BODY[HEADER.FIELDS (SUBJECT)]\r\n")
        .await
        .unwrap();

    let transcript =
        read_lines_until(&mut reader, |line| line.contains("a3 OK FETCH completed")).await;
    server.abort();

    assert!(
        transcript
            .iter()
            .any(|line| line.contains("Subject: hello"))
    );
}

#[tokio::test]
async fn imap_listener_logs_out_over_tcp() {
    let db = server::db::TestDatabase::new().await;
    let core = server::core::service::MailCoreService::new(db.repositories.clone());

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(server::imap::serve(listener, core));

    let stream = TcpStream::connect(addr).await.unwrap();
    let (reader_half, mut writer_half) = stream.into_split();
    let mut reader = BufReader::new(reader_half);

    writer_half.write_all(b"a1 LOGOUT\r\n").await.unwrap();

    let transcript =
        read_lines_until(&mut reader, |line| line.contains("a1 OK LOGOUT completed")).await;
    server.abort();

    assert!(transcript.iter().any(|line| line == "* BYE Logging out"));
    assert!(
        transcript
            .iter()
            .any(|line| line == "a1 OK LOGOUT completed")
    );
}

#[tokio::test]
async fn imap_listener_supports_store_uid_search_and_expunge() {
    let db = server::db::TestDatabase::new().await;
    let core = server::core::service::MailCoreService::new(db.repositories.clone());
    core.provision_mailbox("example.com", "alice", "password123")
        .await
        .unwrap();
    core.receive_inbound_message(
        "sender@example.com",
        &["alice@example.com".to_string()],
        "Subject: first\r\n\r\nbody",
    )
    .await
    .unwrap();
    core.receive_inbound_message(
        "sender@example.com",
        &["alice@example.com".to_string()],
        "Subject: second\r\n\r\nbody",
    )
    .await
    .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(server::imap::serve(listener, core));

    let stream = TcpStream::connect(addr).await.unwrap();
    let (reader_half, mut writer_half) = stream.into_split();
    let mut reader = BufReader::new(reader_half);

    writer_half
        .write_all(b"a1 LOGIN alice@example.com password123\r\n")
        .await
        .unwrap();
    writer_half.write_all(b"a2 SELECT INBOX\r\n").await.unwrap();
    writer_half
        .write_all(b"a3 UID SEARCH ALL\r\n")
        .await
        .unwrap();
    writer_half
        .write_all(b"a4 UID FETCH 2 BODY[HEADER.FIELDS (SUBJECT)]\r\n")
        .await
        .unwrap();
    writer_half
        .write_all(b"a5 STORE 1 +FLAGS (\\Deleted)\r\n")
        .await
        .unwrap();
    writer_half.write_all(b"a6 EXPUNGE\r\n").await.unwrap();
    writer_half.write_all(b"a7 SEARCH ALL\r\n").await.unwrap();

    let transcript =
        read_lines_until(&mut reader, |line| line.contains("a7 OK SEARCH completed")).await;
    server.abort();

    assert!(transcript.iter().any(|line| line == "* SEARCH 1 2"));
    assert!(
        transcript
            .iter()
            .any(|line| line.contains("Subject: second"))
    );
    assert!(
        transcript
            .iter()
            .any(|line| line == "* 1 FETCH (FLAGS (\\Deleted))")
    );
    assert!(transcript.iter().any(|line| line == "* 1 EXPUNGE"));
    assert!(
        transcript
            .iter()
            .any(|line| line == "a6 OK EXPUNGE completed")
    );
    let search_lines = transcript
        .iter()
        .filter(|line| line.starts_with("* SEARCH"))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(search_lines.last().map(String::as_str), Some("* SEARCH 1"));
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
