use server::core::service::MailCoreService;

#[tokio::test]
async fn stores_and_fetches_inbox_message_subject() {
    let ctx = server::db::TestDatabase::new().await;
    let service = MailCoreService::new(ctx.repositories.clone());

    service
        .provision_mailbox("example.com", "alice", "password123")
        .await
        .unwrap();

    let mailbox = service
        .authenticate_mailbox("alice@example.com", "password123")
        .await
        .unwrap();
    service
        .receive_inbound_message(
            "alice@example.com",
            &["alice@example.com".to_string()],
            "Subject: hello\r\n\r\nbody",
        )
        .await
        .unwrap();

    let selection = service.select_folder(mailbox.id, "INBOX").await.unwrap();
    let subject = service
        .fetch_message_subject(mailbox.id, "INBOX", 1)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(selection.exists, 1);
    assert_eq!(selection.uid_next, 2);
    assert_eq!(subject, "hello");
}
