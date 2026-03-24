pub mod parser;
pub mod session;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

use crate::core::service::{
    FlagUpdateMode, MailCoreService, MessageIdentifier, MessageMetadata, MessageSearch,
};
use crate::imap::parser::{ImapCommand, MessageLookup, SearchQuery, StoreMode, parse_line};
use crate::imap::session::{Backend, FolderSnapshot, ImapSession, MessageSnapshot};
use crate::tls::TlsManager;

#[derive(Clone)]
struct MailCoreBackend {
    core: MailCoreService,
    authenticated_mailboxes: Arc<Mutex<HashMap<String, i64>>>,
}

impl MailCoreBackend {
    fn new(core: MailCoreService) -> Self {
        Self {
            core,
            authenticated_mailboxes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn mailbox_id(&self, email: &str) -> Option<i64> {
        self.authenticated_mailboxes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(email)
            .copied()
    }
}

impl Backend for MailCoreBackend {
    async fn authenticate(&self, email: &str, password: &str) -> bool {
        match self.core.authenticate_mailbox(email, password).await {
            Ok(mailbox) => {
                self.authenticated_mailboxes
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .insert(email.to_string(), mailbox.id);
                true
            }
            Err(_) => false,
        }
    }

    async fn list_folders(&self, _email: &str) -> Vec<String> {
        vec![
            "INBOX".to_string(),
            "Sent".to_string(),
            "Drafts".to_string(),
            "Trash".to_string(),
        ]
    }

    async fn select_folder(&self, email: &str, folder: &str) -> Option<FolderSnapshot> {
        let mailbox_id = self.mailbox_id(email)?;
        let selection = self.core.select_folder(mailbox_id, folder).await.ok()?;
        Some(FolderSnapshot {
            exists: selection.exists,
            uid_validity: selection.uid_validity,
            uid_next: selection.uid_next,
        })
    }

    async fn fetch_message(
        &self,
        email: &str,
        folder: &str,
        lookup: MessageLookup,
    ) -> Option<MessageSnapshot> {
        let mailbox_id = self.mailbox_id(email)?;
        self.core
            .fetch_message(mailbox_id, folder, map_lookup(lookup))
            .await
            .ok()
            .flatten()
            .map(map_message)
    }

    async fn store_flags(
        &self,
        email: &str,
        folder: &str,
        lookup: MessageLookup,
        mode: StoreMode,
        flags: Vec<String>,
    ) -> Option<MessageSnapshot> {
        let mailbox_id = self.mailbox_id(email)?;
        self.core
            .store_flags(
                mailbox_id,
                folder,
                map_lookup(lookup),
                map_store_mode(mode),
                &flags,
            )
            .await
            .ok()
            .flatten()
            .map(map_message)
    }

    async fn search_messages(
        &self,
        email: &str,
        folder: &str,
        query: SearchQuery,
        return_uids: bool,
    ) -> Vec<i64> {
        let Some(mailbox_id) = self.mailbox_id(email) else {
            return Vec::new();
        };
        self.core
            .search_messages(mailbox_id, folder, map_search_query(query), return_uids)
            .await
            .unwrap_or_default()
    }

    async fn expunge_deleted(&self, email: &str, folder: &str) -> Vec<i64> {
        let Some(mailbox_id) = self.mailbox_id(email) else {
            return Vec::new();
        };
        self.core
            .expunge_deleted(mailbox_id, folder)
            .await
            .unwrap_or_default()
    }
}

pub async fn serve(listener: TcpListener, core: MailCoreService) -> std::io::Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let backend = MailCoreBackend::new(core.clone());
        tokio::spawn(async move {
            if let Err(error) = handle_connection_io(stream, backend).await {
                eprintln!("imap connection error: {error}");
            }
        });
    }
}

pub async fn serve_secure(
    listener: TcpListener,
    core: MailCoreService,
    tls_manager: Arc<TlsManager>,
) -> std::io::Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let backend = MailCoreBackend::new(core.clone());
        let tls_manager = tls_manager.clone();

        tokio::spawn(async move {
            let Ok(server_config) = tls_manager.server_config() else {
                eprintln!("imap tls config unavailable");
                return;
            };
            let acceptor = TlsAcceptor::from(server_config);

            match acceptor.accept(stream).await {
                Ok(tls_stream) => {
                    if let Err(error) = handle_connection_io(tls_stream, backend).await {
                        eprintln!("imaps connection error: {error}");
                    }
                }
                Err(error) => eprintln!("imaps handshake error: {error}"),
            }
        });
    }
}

async fn handle_connection_io<S>(stream: S, backend: MailCoreBackend) -> std::io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (reader_half, mut writer_half) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader_half);
    let mut session = ImapSession::new(backend);

    writer_half
        .write_all(b"* OK rustmailer IMAP ready\r\n")
        .await?;
    writer_half.flush().await?;

    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line).await?;
        if bytes == 0 {
            break;
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);
        let should_close = matches!(parse_line(trimmed), Some(ImapCommand::Logout { .. }));
        let responses = session.handle_line(trimmed).await;
        for response in responses {
            writer_half.write_all(response.as_bytes()).await?;
            writer_half.write_all(b"\r\n").await?;
        }
        writer_half.flush().await?;
        if should_close {
            break;
        }
    }

    Ok(())
}

pub mod test_support {
    use super::parser::{MessageLookup, SearchQuery, StoreMode};
    use super::session::{Backend, FolderSnapshot, ImapSession, MessageSnapshot};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    pub struct TestHarness {
        backend: MemoryBackend,
    }

    impl TestHarness {
        pub async fn spawn_with_seed_message() -> Self {
            let backend = MemoryBackend::seeded();
            Self { backend }
        }

        pub async fn spawn_with_seed_messages() -> Self {
            let backend = MemoryBackend::seeded_with_two_messages();
            Self { backend }
        }

        pub async fn run<const N: usize>(&self, commands: [&str; N]) -> Vec<String> {
            let mut session = ImapSession::new(self.backend.clone());
            let mut transcript = vec!["* OK rustmailer IMAP ready".to_string()];
            for command in commands {
                let lines = session.handle_line(command).await;
                transcript.extend(lines);
            }
            transcript
        }
    }

    #[derive(Clone)]
    struct MemoryBackend {
        state: Arc<Mutex<State>>,
    }

    #[derive(Clone)]
    struct State {
        users: HashMap<String, String>,
        folders: HashMap<String, Vec<String>>,
        messages: HashMap<(String, String), Vec<MemoryMessage>>,
    }

    #[derive(Clone)]
    struct MemoryMessage {
        uid: i64,
        subject: String,
        flags: Vec<String>,
    }

    impl MemoryBackend {
        fn seeded() -> Self {
            let mut users = HashMap::new();
            users.insert("alice@example.com".to_string(), "password123".to_string());

            let mut folders = HashMap::new();
            folders.insert(
                "alice@example.com".to_string(),
                vec![
                    "INBOX".to_string(),
                    "Sent".to_string(),
                    "Drafts".to_string(),
                    "Trash".to_string(),
                ],
            );

            let mut messages = HashMap::new();
            messages.insert(
                ("alice@example.com".to_string(), "INBOX".to_string()),
                vec![MemoryMessage {
                    uid: 1,
                    subject: "hello".to_string(),
                    flags: Vec::new(),
                }],
            );

            Self {
                state: Arc::new(Mutex::new(State {
                    users,
                    folders,
                    messages,
                })),
            }
        }

        fn seeded_with_two_messages() -> Self {
            let seeded = Self::seeded();
            let mut guard = seeded
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.messages.insert(
                ("alice@example.com".to_string(), "INBOX".to_string()),
                vec![
                    MemoryMessage {
                        uid: 1,
                        subject: "hello".to_string(),
                        flags: Vec::new(),
                    },
                    MemoryMessage {
                        uid: 2,
                        subject: "second".to_string(),
                        flags: Vec::new(),
                    },
                ],
            );
            drop(guard);
            seeded
        }
    }

    impl Backend for MemoryBackend {
        async fn authenticate(&self, email: &str, password: &str) -> bool {
            let guard = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard
                .users
                .get(email)
                .map(|stored| stored == password)
                .unwrap_or(false)
        }

        async fn list_folders(&self, email: &str) -> Vec<String> {
            let guard = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.folders.get(email).cloned().unwrap_or_default()
        }

        async fn select_folder(&self, email: &str, folder: &str) -> Option<FolderSnapshot> {
            let guard = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let messages = guard
                .messages
                .get(&(email.to_string(), folder.to_string()))
                .cloned()
                .unwrap_or_default();

            Some(FolderSnapshot {
                exists: messages.len() as i64,
                uid_validity: 1,
                uid_next: messages.len() as i64 + 1,
            })
        }

        async fn fetch_message(
            &self,
            email: &str,
            folder: &str,
            lookup: MessageLookup,
        ) -> Option<MessageSnapshot> {
            let guard = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let messages = guard
                .messages
                .get(&(email.to_string(), folder.to_string()))?;
            find_memory_message(messages, lookup)
        }

        async fn store_flags(
            &self,
            email: &str,
            folder: &str,
            lookup: MessageLookup,
            mode: StoreMode,
            flags: Vec<String>,
        ) -> Option<MessageSnapshot> {
            let mut guard = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let messages = guard
                .messages
                .get_mut(&(email.to_string(), folder.to_string()))?;
            let index = find_memory_index(messages, lookup.clone())?;
            let normalized = normalize_memory_flags(flags);

            match mode {
                StoreMode::Add => {
                    for flag in normalized {
                        if !messages[index]
                            .flags
                            .iter()
                            .any(|existing| existing.eq_ignore_ascii_case(&flag))
                        {
                            messages[index].flags.push(flag);
                        }
                    }
                }
                StoreMode::Remove => {
                    messages[index].flags.retain(|existing| {
                        !normalized
                            .iter()
                            .any(|flag| flag.eq_ignore_ascii_case(existing))
                    });
                }
                StoreMode::Replace => {
                    messages[index].flags = normalized;
                }
            }

            messages[index].flags.sort();
            drop(guard);
            self.fetch_message(email, folder, lookup).await
        }

        async fn search_messages(
            &self,
            email: &str,
            folder: &str,
            query: SearchQuery,
            return_uids: bool,
        ) -> Vec<i64> {
            let guard = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(messages) = guard.messages.get(&(email.to_string(), folder.to_string()))
            else {
                return Vec::new();
            };

            messages
                .iter()
                .enumerate()
                .filter(|(_, message)| matches_memory_search(&message.flags, &query))
                .map(|(index, message)| {
                    if return_uids {
                        message.uid
                    } else {
                        index as i64 + 1
                    }
                })
                .collect()
        }

        async fn expunge_deleted(&self, email: &str, folder: &str) -> Vec<i64> {
            let mut guard = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(messages) = guard
                .messages
                .get_mut(&(email.to_string(), folder.to_string()))
            else {
                return Vec::new();
            };

            let mut removed = messages
                .iter()
                .enumerate()
                .filter_map(|(index, message)| {
                    message
                        .flags
                        .iter()
                        .any(|flag| flag.eq_ignore_ascii_case("\\Deleted"))
                        .then_some(index as i64 + 1)
                })
                .collect::<Vec<_>>();
            if removed.is_empty() {
                return removed;
            }

            messages.retain(|message| {
                !message
                    .flags
                    .iter()
                    .any(|flag| flag.eq_ignore_ascii_case("\\Deleted"))
            });
            removed.sort_unstable_by(|left, right| right.cmp(left));
            removed
        }
    }

    fn find_memory_message(
        messages: &[MemoryMessage],
        lookup: MessageLookup,
    ) -> Option<MessageSnapshot> {
        let (index, message) =
            messages
                .iter()
                .enumerate()
                .find(|(index, message)| match lookup {
                    MessageLookup::Sequence(sequence) => *index as i64 + 1 == sequence,
                    MessageLookup::Uid(uid) => message.uid == uid,
                })?;

        Some(MessageSnapshot {
            sequence: index as i64 + 1,
            uid: message.uid,
            subject: Some(message.subject.clone()),
            flags: normalize_memory_flags(message.flags.clone()),
        })
    }

    fn find_memory_index(messages: &[MemoryMessage], lookup: MessageLookup) -> Option<usize> {
        messages
            .iter()
            .enumerate()
            .find(|(index, message)| match lookup {
                MessageLookup::Sequence(sequence) => *index as i64 + 1 == sequence,
                MessageLookup::Uid(uid) => message.uid == uid,
            })
            .map(|(index, _)| index)
    }

    fn normalize_memory_flags(flags: Vec<String>) -> Vec<String> {
        let mut flags = flags
            .into_iter()
            .map(|flag| match flag.to_ascii_lowercase().as_str() {
                "\\seen" => "\\Seen".to_string(),
                "\\deleted" => "\\Deleted".to_string(),
                other => other.to_string(),
            })
            .collect::<Vec<_>>();
        flags.sort();
        flags.dedup();
        flags
    }

    fn matches_memory_search(flags: &[String], query: &SearchQuery) -> bool {
        match query {
            SearchQuery::All => true,
            SearchQuery::Seen => flags.iter().any(|flag| flag.eq_ignore_ascii_case("\\Seen")),
            SearchQuery::Unseen => !flags.iter().any(|flag| flag.eq_ignore_ascii_case("\\Seen")),
            SearchQuery::Deleted => flags
                .iter()
                .any(|flag| flag.eq_ignore_ascii_case("\\Deleted")),
            SearchQuery::Undeleted => !flags
                .iter()
                .any(|flag| flag.eq_ignore_ascii_case("\\Deleted")),
        }
    }
}

fn map_lookup(lookup: MessageLookup) -> MessageIdentifier {
    match lookup {
        MessageLookup::Sequence(sequence) => MessageIdentifier::Sequence(sequence),
        MessageLookup::Uid(uid) => MessageIdentifier::Uid(uid),
    }
}

fn map_store_mode(mode: StoreMode) -> FlagUpdateMode {
    match mode {
        StoreMode::Add => FlagUpdateMode::Add,
        StoreMode::Remove => FlagUpdateMode::Remove,
        StoreMode::Replace => FlagUpdateMode::Replace,
    }
}

fn map_search_query(query: SearchQuery) -> MessageSearch {
    match query {
        SearchQuery::All => MessageSearch::All,
        SearchQuery::Seen => MessageSearch::Seen,
        SearchQuery::Unseen => MessageSearch::Unseen,
        SearchQuery::Deleted => MessageSearch::Deleted,
        SearchQuery::Undeleted => MessageSearch::Undeleted,
    }
}

fn map_message(message: MessageMetadata) -> MessageSnapshot {
    MessageSnapshot {
        sequence: message.sequence,
        uid: message.uid,
        subject: message.subject,
        flags: message.flags,
    }
}
