#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainEvent {
    MailboxProvisioned {
        mailbox_id: i64,
        email: String,
    },
    MessageStored {
        sender: String,
        recipient_count: u32,
    },
}
