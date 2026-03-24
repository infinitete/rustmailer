use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainName(String);

impl DomainName {
    pub fn new(value: &str) -> AppResult<Self> {
        let normalized = value.trim().to_ascii_lowercase();
        if normalized.is_empty()
            || normalized.contains(char::is_whitespace)
            || !normalized.contains('.')
            || normalized.starts_with('.')
            || normalized.ends_with('.')
            || normalized.contains("..")
        {
            return Err(AppError::InvalidDomainName {
                value: value.to_string(),
            });
        }

        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailboxAddress {
    email: String,
    local_part: String,
    domain: DomainName,
}

impl MailboxAddress {
    pub fn new(value: &str) -> AppResult<Self> {
        let normalized = value.trim().to_ascii_lowercase();
        let mut parts = normalized.split('@');
        let local_part = parts.next().unwrap_or_default().to_string();
        let domain_part = parts.next().unwrap_or_default().to_string();

        if local_part.is_empty()
            || domain_part.is_empty()
            || parts.next().is_some()
            || local_part.contains(char::is_whitespace)
        {
            return Err(AppError::InvalidMailboxAddress {
                value: value.to_string(),
            });
        }

        let domain = DomainName::new(&domain_part)?;

        Ok(Self {
            email: normalized,
            local_part,
            domain,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.email
    }

    pub fn local_part(&self) -> &str {
        &self.local_part
    }

    pub fn domain(&self) -> &DomainName {
        &self.domain
    }
}
