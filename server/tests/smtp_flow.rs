#[tokio::test]
async fn smtp_data_command_stores_message_for_local_mailbox() {
    let mut harness = server::smtp::test_support::spawn().await;
    let transcript = harness
        .run([
            "EHLO localhost",
            "AUTH PLAIN AGFsaWNlQGV4YW1wbGUuY29tAHBhc3N3b3JkMTIz",
            "MAIL FROM:<alice@example.com>",
            "RCPT TO:<alice@example.com>",
            "DATA",
            "Subject: hello",
            "",
            "body",
            ".",
            "QUIT",
        ])
        .await;

    assert!(
        transcript
            .iter()
            .any(|line| line.contains("250 Message accepted"))
    );
}

#[tokio::test]
async fn smtp_helo_is_accepted() {
    let mut harness = server::smtp::test_support::spawn().await;
    let transcript = harness.run(["HELO localhost"]).await;

    assert!(transcript.iter().any(|line| line.starts_with("250")));
}

#[tokio::test]
async fn smtp_auth_login_two_step_flow_authenticates_mailbox() {
    let mut harness = server::smtp::test_support::spawn().await;
    let transcript = harness
        .run([
            "EHLO localhost",
            "AUTH LOGIN",
            "YWxpY2VAZXhhbXBsZS5jb20=",
            "cGFzc3dvcmQxMjM=",
            "QUIT",
        ])
        .await;

    assert!(transcript.iter().any(|line| line == "334 VXNlcm5hbWU6"));
    assert!(transcript.iter().any(|line| line == "334 UGFzc3dvcmQ6"));
    assert!(
        transcript
            .iter()
            .any(|line| line.contains("235 Authentication successful"))
    );
}
