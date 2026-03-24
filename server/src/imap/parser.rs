#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImapCommand {
    Login {
        tag: String,
        email: String,
        password: String,
    },
    List {
        tag: String,
    },
    Select {
        tag: String,
        mailbox: String,
    },
    Fetch {
        tag: String,
        lookup: MessageLookup,
    },
    Logout {
        tag: String,
    },
    Store {
        tag: String,
        lookup: MessageLookup,
        mode: StoreMode,
        flags: Vec<String>,
    },
    Search {
        tag: String,
        query: SearchQuery,
        return_uids: bool,
    },
    Expunge {
        tag: String,
    },
    Unknown {
        tag: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageLookup {
    Sequence(i64),
    Uid(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchQuery {
    All,
    Seen,
    Unseen,
    Deleted,
    Undeleted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreMode {
    Add,
    Remove,
    Replace,
}

pub fn parse_line(line: &str) -> Option<ImapCommand> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut parts = trimmed.split_whitespace();
    let tag = parts.next()?.to_string();
    let verb = parts.next()?.to_ascii_uppercase();

    match verb.as_str() {
        "LOGIN" => {
            let email = parts.next()?.to_string();
            let password = parts.next()?.to_string();
            Some(ImapCommand::Login {
                tag,
                email,
                password,
            })
        }
        "LIST" => Some(ImapCommand::List { tag }),
        "SELECT" => {
            let mailbox = parts.next()?.to_string();
            Some(ImapCommand::Select { tag, mailbox })
        }
        "FETCH" => {
            let sequence = parts.next()?.parse::<i64>().ok()?;
            Some(ImapCommand::Fetch {
                tag,
                lookup: MessageLookup::Sequence(sequence),
            })
        }
        "LOGOUT" => Some(ImapCommand::Logout { tag }),
        "STORE" => parse_store(tag, &mut parts, false),
        "SEARCH" => Some(ImapCommand::Search {
            tag,
            query: parse_search_query(parts.next()),
            return_uids: false,
        }),
        "EXPUNGE" => Some(ImapCommand::Expunge { tag }),
        "UID" => parse_uid_command(tag, &mut parts),
        _ => Some(ImapCommand::Unknown { tag }),
    }
}

fn parse_store(
    tag: String,
    parts: &mut std::str::SplitWhitespace<'_>,
    use_uid: bool,
) -> Option<ImapCommand> {
    let id = parts.next()?.parse::<i64>().ok()?;
    let mode = match parts.next()?.to_ascii_uppercase().as_str() {
        "+FLAGS" => StoreMode::Add,
        "-FLAGS" => StoreMode::Remove,
        "FLAGS" => StoreMode::Replace,
        _ => return Some(ImapCommand::Unknown { tag }),
    };
    let flags = parse_flags(&parts.collect::<Vec<_>>().join(" "));
    Some(ImapCommand::Store {
        tag,
        lookup: if use_uid {
            MessageLookup::Uid(id)
        } else {
            MessageLookup::Sequence(id)
        },
        mode,
        flags,
    })
}

fn parse_uid_command(
    tag: String,
    parts: &mut std::str::SplitWhitespace<'_>,
) -> Option<ImapCommand> {
    let subcommand = parts.next()?.to_ascii_uppercase();
    match subcommand.as_str() {
        "FETCH" => {
            let uid = parts.next()?.parse::<i64>().ok()?;
            Some(ImapCommand::Fetch {
                tag,
                lookup: MessageLookup::Uid(uid),
            })
        }
        "STORE" => parse_store(tag, parts, true),
        "SEARCH" => Some(ImapCommand::Search {
            tag,
            query: parse_search_query(parts.next()),
            return_uids: true,
        }),
        _ => Some(ImapCommand::Unknown { tag }),
    }
}

fn parse_search_query(value: Option<&str>) -> SearchQuery {
    match value.unwrap_or("ALL").to_ascii_uppercase().as_str() {
        "SEEN" => SearchQuery::Seen,
        "UNSEEN" => SearchQuery::Unseen,
        "DELETED" => SearchQuery::Deleted,
        "UNDELETED" => SearchQuery::Undeleted,
        _ => SearchQuery::All,
    }
}

fn parse_flags(input: &str) -> Vec<String> {
    input
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .split_whitespace()
        .map(ToString::to_string)
        .collect()
}
