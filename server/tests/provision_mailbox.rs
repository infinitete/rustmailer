use server::core::service::MailCoreService;

#[tokio::test]
async fn provisioning_mailbox_creates_default_folders() {
    let ctx = server::db::TestDatabase::new().await;
    let service = MailCoreService::new(ctx.repositories.clone());

    let mailbox = service
        .provision_mailbox("example.com", "alice", "password123")
        .await
        .unwrap();

    let folders = ctx
        .repositories
        .folders
        .list_for_mailbox(mailbox.id)
        .await
        .unwrap();
    let names: Vec<_> = folders.into_iter().map(|folder| folder.name).collect();

    assert!(names.contains(&"INBOX".to_string()));
    assert!(names.contains(&"Sent".to_string()));
    assert!(names.contains(&"Drafts".to_string()));
    assert!(names.contains(&"Trash".to_string()));
}
