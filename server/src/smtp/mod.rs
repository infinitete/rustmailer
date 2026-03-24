pub mod parser;
pub mod session;

use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;

use crate::core::service::MailCoreService;
use crate::smtp::parser::{SmtpCommand, parse_command};
use crate::smtp::session::SmtpSession;
use crate::tls::TlsManager;

pub async fn serve(listener: TcpListener, core: MailCoreService) {
    while let Ok((stream, _peer_addr)) = listener.accept().await {
        let core_for_connection = core.clone();
        tokio::spawn(async move {
            let _ = handle_connection(stream, core_for_connection).await;
        });
    }
}

pub async fn serve_with_starttls(
    listener: TcpListener,
    core: MailCoreService,
    tls_manager: Arc<TlsManager>,
) {
    while let Ok((stream, _peer_addr)) = listener.accept().await {
        let core_for_connection = core.clone();
        let tls_for_connection = tls_manager.clone();
        tokio::spawn(async move {
            let _ =
                handle_connection_with_starttls(stream, core_for_connection, tls_for_connection)
                    .await;
        });
    }
}

async fn handle_connection(stream: TcpStream, core: MailCoreService) -> std::io::Result<()> {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut session = SmtpSession::new(core);

    write_half
        .write_all(b"220 rustmailer SMTP ready\r\n")
        .await?;

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            break;
        }

        let command_line = line.trim_end_matches(['\r', '\n']);
        let responses = session.handle_line(command_line).await;
        let should_close = responses.iter().any(|response| response.starts_with("221"));

        for response in responses {
            write_half.write_all(response.as_bytes()).await?;
            write_half.write_all(b"\r\n").await?;
        }

        if should_close {
            break;
        }
    }

    Ok(())
}

async fn handle_connection_with_starttls(
    stream: TcpStream,
    core: MailCoreService,
    tls_manager: Arc<TlsManager>,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut session = SmtpSession::for_submission(core, true);

    reader
        .get_mut()
        .write_all(b"220 rustmailer SMTP ready\r\n")
        .await?;

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            break;
        }

        let command_line = line.trim_end_matches(['\r', '\n']);
        if parse_command(command_line) == SmtpCommand::StartTls {
            let responses = session.handle_line(command_line).await;
            write_responses(reader.get_mut(), responses).await?;
            reader.get_mut().flush().await?;

            let acceptor = TlsAcceptor::from(
                tls_manager
                    .server_config()
                    .map_err(|error| std::io::Error::other(error.to_string()))?,
            );

            let stream = reader.into_inner();
            let tls_stream = acceptor
                .accept(stream)
                .await
                .map_err(|error| std::io::Error::other(error.to_string()))?;

            return handle_tls_session(tls_stream, session.reset_after_starttls()).await;
        }

        let responses = session.handle_line(command_line).await;
        let should_close = responses.iter().any(|response| response.starts_with("221"));
        write_responses(reader.get_mut(), responses).await?;
        if should_close {
            break;
        }
    }

    Ok(())
}

async fn handle_tls_session<S>(stream: S, mut session: SmtpSession) -> std::io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut reader = BufReader::new(stream);

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            break;
        }

        let command_line = line.trim_end_matches(['\r', '\n']);
        let responses = session.handle_line(command_line).await;
        let should_close = responses.iter().any(|response| response.starts_with("221"));
        write_responses(reader.get_mut(), responses).await?;
        if should_close {
            break;
        }
    }

    Ok(())
}

async fn write_responses<W>(writer: &mut W, responses: Vec<String>) -> std::io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    for response in responses {
        let line = format!("{response}\r\n");
        writer.write_all(line.as_bytes()).await?;
    }
    Ok(())
}

pub mod test_support {
    use crate::core::service::MailCoreService;
    use crate::db::TestDatabase;
    use crate::smtp::session::SmtpSession;

    pub struct SmtpHarness {
        _db: TestDatabase,
        session: SmtpSession,
    }

    impl SmtpHarness {
        pub async fn run<I, S>(&mut self, commands: I) -> Vec<String>
        where
            I: IntoIterator<Item = S>,
            S: AsRef<str>,
        {
            let mut transcript = Vec::new();
            for command in commands {
                let responses = self.session.handle_line(command.as_ref()).await;
                transcript.extend(responses);
            }
            transcript
        }
    }

    pub async fn spawn() -> SmtpHarness {
        let db = TestDatabase::new().await;
        let core = MailCoreService::new(db.repositories.clone());
        core.provision_mailbox("example.com", "alice", "password123")
            .await
            .expect("provision mailbox");

        SmtpHarness {
            _db: db,
            session: SmtpSession::new(core),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use tokio::io::AsyncWrite;

    use super::write_responses;

    struct RecordingWriter {
        writes: Vec<Vec<u8>>,
    }

    impl RecordingWriter {
        fn new() -> Self {
            Self { writes: Vec::new() }
        }
    }

    impl AsyncWrite for RecordingWriter {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            self.writes.push(buf.to_vec());
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn write_responses_writes_full_line_in_single_write() {
        let mut writer = RecordingWriter::new();
        write_responses(&mut writer, vec!["220 Ready to start TLS".to_string()])
            .await
            .expect("write response");

        assert_eq!(writer.writes.len(), 1);
        assert_eq!(writer.writes[0], b"220 Ready to start TLS\r\n");
    }
}
