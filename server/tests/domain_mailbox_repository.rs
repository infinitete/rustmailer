use server::core::entities::{DomainName, MailboxAddress};

#[tokio::test]
async fn creates_domain_and_mailbox_records() {
    let ctx = server::db::TestDatabase::new().await;
    let domain = ctx
        .repositories
        .domains
        .create(DomainName::new("example.com").unwrap())
        .await
        .unwrap();
    let mailbox = ctx
        .repositories
        .mailboxes
        .create(MailboxAddress::new("alice@example.com").unwrap(), domain.id)
        .await
        .unwrap();

    assert_eq!(mailbox.local_part, "alice");
    assert_eq!(mailbox.domain_id, domain.id);
}
