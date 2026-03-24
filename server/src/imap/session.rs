use crate::imap::parser::{ImapCommand, MessageLookup, SearchQuery, StoreMode, parse_line};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderSnapshot {
    pub exists: i64,
    pub uid_validity: i64,
    pub uid_next: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageSnapshot {
    pub sequence: i64,
    pub uid: i64,
    pub subject: Option<String>,
    pub flags: Vec<String>,
}

#[allow(async_fn_in_trait)]
pub trait Backend: Clone + Send + Sync + 'static {
    async fn authenticate(&self, email: &str, password: &str) -> bool;
    async fn list_folders(&self, email: &str) -> Vec<String>;
    async fn select_folder(&self, email: &str, folder: &str) -> Option<FolderSnapshot>;
    async fn fetch_message(
        &self,
        email: &str,
        folder: &str,
        lookup: MessageLookup,
    ) -> Option<MessageSnapshot>;
    async fn store_flags(
        &self,
        email: &str,
        folder: &str,
        lookup: MessageLookup,
        mode: StoreMode,
        flags: Vec<String>,
    ) -> Option<MessageSnapshot>;
    async fn search_messages(
        &self,
        email: &str,
        folder: &str,
        query: SearchQuery,
        return_uids: bool,
    ) -> Vec<i64>;
    async fn expunge_deleted(&self, email: &str, folder: &str) -> Vec<i64>;
}

#[derive(Clone)]
pub struct ImapSession<B: Backend> {
    backend: B,
    authenticated_user: Option<String>,
    selected_folder: Option<String>,
}

impl<B: Backend> ImapSession<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            authenticated_user: None,
            selected_folder: None,
        }
    }

    pub async fn handle_line(&mut self, line: &str) -> Vec<String> {
        let Some(command) = parse_line(line) else {
            return vec!["* BAD Invalid command".to_string()];
        };

        match command {
            ImapCommand::Login {
                tag,
                email,
                password,
            } => {
                if self.backend.authenticate(&email, &password).await {
                    self.authenticated_user = Some(email);
                    vec![format!("{tag} OK LOGIN completed")]
                } else {
                    vec![format!("{tag} NO LOGIN failed")]
                }
            }
            ImapCommand::List { tag } => {
                let Some(user) = self.authenticated_user.as_deref() else {
                    return vec![format!("{tag} NO Authenticate first")];
                };
                let folders = self.backend.list_folders(user).await;
                let mut out = Vec::with_capacity(folders.len() + 1);
                for folder in folders {
                    out.push(format!("* LIST () \"/\" \"{folder}\""));
                }
                out.push(format!("{tag} OK LIST completed"));
                out
            }
            ImapCommand::Select { tag, mailbox } => {
                let Some(user) = self.authenticated_user.as_deref() else {
                    return vec![format!("{tag} NO Authenticate first")];
                };
                match self.backend.select_folder(user, &mailbox).await {
                    Some(snapshot) => {
                        self.selected_folder = Some(mailbox);
                        vec![
                            format!("* {} EXISTS", snapshot.exists),
                            format!("* OK [UIDVALIDITY {}] UIDs valid", snapshot.uid_validity),
                            format!("* OK [UIDNEXT {}] Predicted next UID", snapshot.uid_next),
                            format!("{tag} OK [READ-WRITE] SELECT completed"),
                        ]
                    }
                    None => vec![format!("{tag} NO No such mailbox")],
                }
            }
            ImapCommand::Fetch { tag, lookup } => self.handle_fetch(&tag, lookup).await,
            ImapCommand::Logout { tag } => vec![
                "* BYE Logging out".to_string(),
                format!("{tag} OK LOGOUT completed"),
            ],
            ImapCommand::Store {
                tag,
                lookup,
                mode,
                flags,
            } => self.handle_store(&tag, lookup, mode, flags).await,
            ImapCommand::Search {
                tag,
                query,
                return_uids,
            } => self.handle_search(&tag, query, return_uids).await,
            ImapCommand::Expunge { tag } => self.handle_expunge(&tag).await,
            ImapCommand::Unknown { tag } => vec![format!("{tag} BAD Unsupported command")],
        }
    }

    async fn handle_fetch(&self, tag: &str, lookup: MessageLookup) -> Vec<String> {
        let Some((user, folder)) = self.require_authenticated_folder(tag) else {
            return missing_session_state(tag, self.authenticated_user.is_some());
        };

        match self
            .backend
            .fetch_message(user, folder, lookup.clone())
            .await
        {
            Some(message) => {
                let header = format!("Subject: {}", message.subject.unwrap_or_default());
                let prefix = match lookup {
                    MessageLookup::Sequence(_) => format!(
                        "* {} FETCH (BODY[HEADER.FIELDS (SUBJECT)] {{{}}}",
                        message.sequence,
                        header.len()
                    ),
                    MessageLookup::Uid(_) => format!(
                        "* {} FETCH (UID {} BODY[HEADER.FIELDS (SUBJECT)] {{{}}}",
                        message.sequence,
                        message.uid,
                        header.len()
                    ),
                };

                vec![
                    prefix,
                    header,
                    ")".to_string(),
                    format!("{tag} OK FETCH completed"),
                ]
            }
            None => vec![format!("{tag} OK FETCH completed")],
        }
    }

    async fn handle_store(
        &self,
        tag: &str,
        lookup: MessageLookup,
        mode: StoreMode,
        flags: Vec<String>,
    ) -> Vec<String> {
        let Some((user, folder)) = self.require_authenticated_folder(tag) else {
            return missing_session_state(tag, self.authenticated_user.is_some());
        };

        match self
            .backend
            .store_flags(user, folder, lookup.clone(), mode, flags)
            .await
        {
            Some(message) => vec![
                format_store_response(&message, matches!(lookup, MessageLookup::Uid(_))),
                format!("{tag} OK STORE completed"),
            ],
            None => vec![format!("{tag} OK STORE completed")],
        }
    }

    async fn handle_search(&self, tag: &str, query: SearchQuery, return_uids: bool) -> Vec<String> {
        let Some((user, folder)) = self.require_authenticated_folder(tag) else {
            return missing_session_state(tag, self.authenticated_user.is_some());
        };

        let matches = self
            .backend
            .search_messages(user, folder, query, return_uids)
            .await;
        let mut response = String::from("* SEARCH");
        for id in matches {
            response.push(' ');
            response.push_str(&id.to_string());
        }

        vec![response, format!("{tag} OK SEARCH completed")]
    }

    async fn handle_expunge(&self, tag: &str) -> Vec<String> {
        let Some((user, folder)) = self.require_authenticated_folder(tag) else {
            return missing_session_state(tag, self.authenticated_user.is_some());
        };

        let mut responses = self
            .backend
            .expunge_deleted(user, folder)
            .await
            .into_iter()
            .map(|sequence| format!("* {sequence} EXPUNGE"))
            .collect::<Vec<_>>();
        responses.push(format!("{tag} OK EXPUNGE completed"));
        responses
    }

    fn require_authenticated_folder(&self, tag: &str) -> Option<(&str, &str)> {
        let user = self.authenticated_user.as_deref()?;
        let folder = self.selected_folder.as_deref()?;
        if tag.is_empty() {
            return None;
        }
        Some((user, folder))
    }
}

fn missing_session_state(tag: &str, authenticated: bool) -> Vec<String> {
    if authenticated {
        vec![format!("{tag} NO Select a mailbox first")]
    } else {
        vec![format!("{tag} NO Authenticate first")]
    }
}

fn format_store_response(message: &MessageSnapshot, include_uid: bool) -> String {
    let flags = if message.flags.is_empty() {
        String::new()
    } else {
        message.flags.join(" ")
    };

    if include_uid {
        format!(
            "* {} FETCH (UID {} FLAGS ({}))",
            message.sequence, message.uid, flags
        )
    } else {
        format!("* {} FETCH (FLAGS ({}))", message.sequence, flags)
    }
}
