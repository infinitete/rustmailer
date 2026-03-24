#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmtpCommand {
    Ehlo(String),
    Helo(String),
    StartTls,
    AuthLogin(Option<String>),
    AuthPlain(String),
    MailFrom(String),
    RcptTo(String),
    Data,
    Quit,
    Noop,
    Unknown,
}

pub fn parse_command(line: &str) -> SmtpCommand {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return SmtpCommand::Noop;
    }

    if let Some(rest) = strip_prefix_case_insensitive(trimmed, "EHLO ") {
        return SmtpCommand::Ehlo(rest.trim().to_string());
    }

    if let Some(rest) = strip_prefix_case_insensitive(trimmed, "HELO ") {
        return SmtpCommand::Helo(rest.trim().to_string());
    }

    if trimmed.eq_ignore_ascii_case("STARTTLS") {
        return SmtpCommand::StartTls;
    }

    if trimmed.eq_ignore_ascii_case("AUTH LOGIN") {
        return SmtpCommand::AuthLogin(None);
    }

    if let Some(rest) = strip_prefix_case_insensitive(trimmed, "AUTH LOGIN ") {
        return SmtpCommand::AuthLogin(Some(rest.trim().to_string()));
    }

    if let Some(rest) = strip_prefix_case_insensitive(trimmed, "AUTH PLAIN ") {
        return SmtpCommand::AuthPlain(rest.trim().to_string());
    }

    if let Some(rest) = strip_prefix_case_insensitive(trimmed, "MAIL FROM:") {
        return SmtpCommand::MailFrom(extract_path(rest));
    }

    if let Some(rest) = strip_prefix_case_insensitive(trimmed, "RCPT TO:") {
        return SmtpCommand::RcptTo(extract_path(rest));
    }

    if trimmed.eq_ignore_ascii_case("DATA") {
        return SmtpCommand::Data;
    }

    if trimmed.eq_ignore_ascii_case("QUIT") {
        return SmtpCommand::Quit;
    }

    SmtpCommand::Unknown
}

fn strip_prefix_case_insensitive<'a>(input: &'a str, prefix: &str) -> Option<&'a str> {
    let head = input.get(..prefix.len())?;
    if head.eq_ignore_ascii_case(prefix) {
        return input.get(prefix.len()..);
    }

    None
}

fn extract_path(input: &str) -> String {
    let value = input.trim();
    value
        .strip_prefix('<')
        .and_then(|v| v.strip_suffix('>'))
        .unwrap_or(value)
        .trim()
        .to_ascii_lowercase()
}
