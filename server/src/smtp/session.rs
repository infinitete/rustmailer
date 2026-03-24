use crate::core::service::MailCoreService;
use crate::smtp::parser::{SmtpCommand, parse_command};

enum AuthLoginState {
    None,
    AwaitingUsername,
    AwaitingPassword { username: String },
}

pub struct SmtpSession {
    core: MailCoreService,
    mail_from: Option<String>,
    rcpt_to: Vec<String>,
    in_data: bool,
    data_lines: Vec<String>,
    starttls_available: bool,
    authentication_required: bool,
    authenticated_mailbox: Option<String>,
    auth_login_state: AuthLoginState,
}

impl SmtpSession {
    pub fn new(core: MailCoreService) -> Self {
        Self::with_options(core, false, false)
    }

    pub fn with_starttls(core: MailCoreService, starttls_available: bool) -> Self {
        Self::with_options(core, starttls_available, false)
    }

    pub fn for_submission(core: MailCoreService, starttls_available: bool) -> Self {
        Self::with_options(core, starttls_available, true)
    }

    fn with_options(
        core: MailCoreService,
        starttls_available: bool,
        authentication_required: bool,
    ) -> Self {
        Self {
            core,
            mail_from: None,
            rcpt_to: Vec::new(),
            in_data: false,
            data_lines: Vec::new(),
            starttls_available,
            authentication_required,
            authenticated_mailbox: None,
            auth_login_state: AuthLoginState::None,
        }
    }

    pub async fn handle_line(&mut self, line: &str) -> Vec<String> {
        if self.in_data {
            return self.handle_data_line(line).await;
        }

        if let Some(responses) = self.handle_auth_login_line(line).await {
            return responses;
        }

        match parse_command(line) {
            SmtpCommand::Ehlo(_domain) => {
                let mut responses = vec!["250-rustmailer".to_string()];
                if self.starttls_available {
                    responses.push("250-STARTTLS".to_string());
                }
                responses.push("250-AUTH LOGIN PLAIN".to_string());
                responses.push("250 DATA".to_string());
                responses
            }
            SmtpCommand::Helo(_domain) => vec!["250 rustmailer".to_string()],
            SmtpCommand::StartTls => {
                if self.starttls_available {
                    vec!["220 Ready to start TLS".to_string()]
                } else {
                    vec!["454 TLS not available".to_string()]
                }
            }
            SmtpCommand::AuthLogin(initial_payload) => {
                self.handle_auth_login_start(initial_payload).await
            }
            SmtpCommand::AuthPlain(payload) => self.handle_auth_plain(&payload).await,
            SmtpCommand::MailFrom(sender) => {
                if let Some(error) = self.ensure_authenticated() {
                    return vec![error];
                }
                self.mail_from = Some(sender);
                self.rcpt_to.clear();
                vec!["250 OK".to_string()]
            }
            SmtpCommand::RcptTo(recipient) => {
                if let Some(error) = self.ensure_authenticated() {
                    return vec![error];
                }
                self.rcpt_to.push(recipient);
                vec!["250 OK".to_string()]
            }
            SmtpCommand::Data => {
                if let Some(error) = self.ensure_authenticated() {
                    return vec![error];
                }
                if self.mail_from.is_none() || self.rcpt_to.is_empty() {
                    return vec!["503 Bad sequence of commands".to_string()];
                }
                self.in_data = true;
                self.data_lines.clear();
                vec!["354 End data with <CR><LF>.<CR><LF>".to_string()]
            }
            SmtpCommand::Quit => vec!["221 Bye".to_string()],
            SmtpCommand::Noop => Vec::new(),
            SmtpCommand::Unknown => vec!["500 Unrecognized command".to_string()],
        }
    }

    async fn handle_auth_login_line(&mut self, line: &str) -> Option<Vec<String>> {
        let pending = std::mem::replace(&mut self.auth_login_state, AuthLoginState::None);
        match pending {
            AuthLoginState::None => None,
            AuthLoginState::AwaitingUsername => match decode_sasl_plain_value(line) {
                Some(username) => {
                    self.auth_login_state = AuthLoginState::AwaitingPassword { username };
                    Some(vec!["334 UGFzc3dvcmQ6".to_string()])
                }
                None => Some(vec!["535 Authentication credentials invalid".to_string()]),
            },
            AuthLoginState::AwaitingPassword { username } => {
                let Some(password) = decode_sasl_plain_value(line) else {
                    return Some(vec!["535 Authentication credentials invalid".to_string()]);
                };
                let result = self.core.authenticate_mailbox(&username, &password).await;
                Some(match result {
                    Ok(_) => {
                        self.authenticated_mailbox = Some(username);
                        vec!["235 Authentication successful".to_string()]
                    }
                    Err(_) => vec!["535 Authentication credentials invalid".to_string()],
                })
            }
        }
    }

    async fn handle_data_line(&mut self, line: &str) -> Vec<String> {
        if line == "." {
            self.in_data = false;
            let sender = self.mail_from.clone().unwrap_or_default();
            let recipients = self.rcpt_to.clone();
            let raw_message = self.data_lines.join("\r\n");

            self.mail_from = None;
            self.rcpt_to.clear();
            self.data_lines.clear();

            let result = self
                .core
                .receive_inbound_message(&sender, &recipients, &raw_message)
                .await;
            return match result {
                Ok(()) => vec!["250 Message accepted".to_string()],
                Err(error) => vec![format!("550 Message rejected: {error}")],
            };
        }

        self.data_lines.push(line.to_string());
        Vec::new()
    }

    async fn handle_auth_login_start(&mut self, initial_payload: Option<String>) -> Vec<String> {
        if let Some(payload) = initial_payload {
            let Some(username) = decode_sasl_plain_value(&payload) else {
                return vec!["535 Authentication credentials invalid".to_string()];
            };
            self.auth_login_state = AuthLoginState::AwaitingPassword { username };
            return vec!["334 UGFzc3dvcmQ6".to_string()];
        }

        self.auth_login_state = AuthLoginState::AwaitingUsername;
        vec!["334 VXNlcm5hbWU6".to_string()]
    }

    async fn handle_auth_plain(&mut self, payload: &str) -> Vec<String> {
        let decoded = match decode_base64(payload) {
            Some(bytes) => bytes,
            None => return vec!["535 Authentication credentials invalid".to_string()],
        };

        let parts: Vec<&[u8]> = decoded.split(|byte| *byte == 0).collect();
        if parts.len() < 3 {
            return vec!["535 Authentication credentials invalid".to_string()];
        }
        let username = match std::str::from_utf8(parts[1]) {
            Ok(value) if !value.is_empty() => value,
            _ => return vec!["535 Authentication credentials invalid".to_string()],
        };
        let password = match std::str::from_utf8(parts[2]) {
            Ok(value) if !value.is_empty() => value,
            _ => return vec!["535 Authentication credentials invalid".to_string()],
        };

        match self.core.authenticate_mailbox(username, password).await {
            Ok(_) => {
                self.authenticated_mailbox = Some(username.to_string());
                vec!["235 Authentication successful".to_string()]
            }
            Err(_) => vec!["535 Authentication credentials invalid".to_string()],
        }
    }

    pub fn reset_after_starttls(&self) -> Self {
        Self::with_options(self.core.clone(), false, self.authentication_required)
    }

    fn ensure_authenticated(&self) -> Option<String> {
        if self.authentication_required && self.authenticated_mailbox.is_none() {
            return Some("530 Authentication required".to_string());
        }

        None
    }
}

fn decode_sasl_plain_value(input: &str) -> Option<String> {
    let decoded = decode_base64(input.trim())?;
    let value = std::str::from_utf8(&decoded).ok()?.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.to_string())
}

fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    let mut chunk = [0u8; 4];
    let mut count = 0usize;

    for byte in input.bytes().filter(|b| !b" \t\r\n".contains(b)) {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => 64,
            _ => return None,
        };
        chunk[count] = value;
        count += 1;

        if count == 4 {
            if chunk[0] == 64 || chunk[1] == 64 {
                return None;
            }

            output.push((chunk[0] << 2) | (chunk[1] >> 4));
            if chunk[2] != 64 {
                output.push((chunk[1] << 4) | (chunk[2] >> 2));
                if chunk[3] != 64 {
                    output.push((chunk[2] << 6) | chunk[3]);
                }
            }
            count = 0;
        }
    }

    if count != 0 {
        return None;
    }

    Some(output)
}
