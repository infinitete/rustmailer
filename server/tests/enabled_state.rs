use server::core::service::{UpdateDomainInput, UpdateMailboxInput};
use server::error::AppError;

#[tokio::test]
async fn disabled_mailbox_cannot_authenticate_or_receive_mail() {
    let db = server::db::TestDatabase::new().await;
    let core = server::core::service::MailCoreService::new(db.repositories.clone());
    let mailbox = core
        .provision_mailbox("example.com", "alice", "password123")
        .await
        .unwrap();

    core.update_mailbox(
        mailbox.id,
        UpdateMailboxInput {
            enabled: Some(false),
            password: None,
        },
    )
    .await
    .unwrap();

    let auth_error = core
        .authenticate_mailbox("alice@example.com", "password123")
        .await
        .unwrap_err();
    assert!(matches!(
        auth_error,
        AppError::MailboxDisabled { email } if email == "alice@example.com"
    ));

    let delivery_error = core
        .receive_inbound_message(
            "sender@example.com",
            &["alice@example.com".to_string()],
            "Subject: hello\r\n\r\nbody",
        )
        .await
        .unwrap_err();
    assert!(matches!(
        delivery_error,
        AppError::MailboxDisabled { email } if email == "alice@example.com"
    ));
}

#[tokio::test]
async fn disabled_domain_blocks_authentication_provisioning_and_delivery() {
    let db = server::db::TestDatabase::new().await;
    let core = server::core::service::MailCoreService::new(db.repositories.clone());
    let domain = core.create_domain("example.com").await.unwrap();
    core.provision_mailbox("example.com", "alice", "password123")
        .await
        .unwrap();

    core.update_domain(
        domain.id,
        UpdateDomainInput {
            name: None,
            enabled: Some(false),
        },
    )
    .await
    .unwrap();

    let auth_error = core
        .authenticate_mailbox("alice@example.com", "password123")
        .await
        .unwrap_err();
    assert!(matches!(
        auth_error,
        AppError::DomainDisabled { domain } if domain == "example.com"
    ));

    let provision_error = core
        .provision_mailbox("example.com", "bob", "password123")
        .await
        .unwrap_err();
    assert!(matches!(
        provision_error,
        AppError::DomainDisabled { domain } if domain == "example.com"
    ));

    let delivery_error = core
        .receive_inbound_message(
            "sender@example.com",
            &["alice@example.com".to_string()],
            "Subject: hello\r\n\r\nbody",
        )
        .await
        .unwrap_err();
    assert!(matches!(
        delivery_error,
        AppError::DomainDisabled { domain } if domain == "example.com"
    ));
}
