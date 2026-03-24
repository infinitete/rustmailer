#[tokio::test]
async fn imap_fetch_returns_subject_for_inbox_message() {
    let harness = server::imap::test_support::TestHarness::spawn_with_seed_message().await;
    let transcript = harness
        .run([
            "a1 LOGIN alice@example.com password123",
            "a2 SELECT INBOX",
            "a3 FETCH 1 BODY[HEADER.FIELDS (SUBJECT)]",
        ])
        .await;

    assert!(
        transcript
            .iter()
            .any(|line| line.contains("Subject: hello"))
    );
}

#[tokio::test]
async fn imap_logout_closes_session_cleanly() {
    let harness = server::imap::test_support::TestHarness::spawn_with_seed_message().await;
    let transcript = harness.run(["a1 LOGOUT"]).await;

    assert!(transcript.iter().any(|line| line == "* BYE Logging out"));
    assert!(
        transcript
            .iter()
            .any(|line| line == "a1 OK LOGOUT completed")
    );
}

#[tokio::test]
async fn imap_store_search_and_expunge_messages() {
    let harness = server::imap::test_support::TestHarness::spawn_with_seed_messages().await;
    let transcript = harness
        .run([
            "a1 LOGIN alice@example.com password123",
            "a2 SELECT INBOX",
            "a3 SEARCH ALL",
            "a4 STORE 1 +FLAGS (\\Seen \\Deleted)",
            "a5 SEARCH DELETED",
            "a6 EXPUNGE",
            "a7 SEARCH ALL",
        ])
        .await;

    assert!(transcript.iter().any(|line| line == "* SEARCH 1 2"));
    assert!(
        transcript
            .iter()
            .any(|line| line == "* 1 FETCH (FLAGS (\\Deleted \\Seen))")
    );
    assert!(
        transcript
            .iter()
            .any(|line| line == "a4 OK STORE completed")
    );
    assert!(
        transcript
            .iter()
            .any(|line| line == "a5 OK SEARCH completed")
    );
    assert!(transcript.iter().any(|line| line == "* 1 EXPUNGE"));
    assert!(
        transcript
            .iter()
            .any(|line| line == "a6 OK EXPUNGE completed")
    );

    let final_searches = transcript
        .iter()
        .filter(|line| line.starts_with("* SEARCH"))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        final_searches.last().map(String::as_str),
        Some("* SEARCH 1")
    );
}

#[tokio::test]
async fn imap_uid_commands_use_stable_uids() {
    let harness = server::imap::test_support::TestHarness::spawn_with_seed_messages().await;
    let transcript = harness
        .run([
            "a1 LOGIN alice@example.com password123",
            "a2 SELECT INBOX",
            "a3 UID SEARCH ALL",
            "a4 UID FETCH 2 BODY[HEADER.FIELDS (SUBJECT)]",
            "a5 UID STORE 2 +FLAGS (\\Seen)",
        ])
        .await;

    assert!(transcript.iter().any(|line| line == "* SEARCH 1 2"));
    assert!(
        transcript
            .iter()
            .any(|line| line.contains("* 2 FETCH (UID 2 BODY[HEADER.FIELDS (SUBJECT)]"))
    );
    assert!(
        transcript
            .iter()
            .any(|line| line.contains("Subject: second"))
    );
    assert!(
        transcript
            .iter()
            .any(|line| line == "* 2 FETCH (UID 2 FLAGS (\\Seen))")
    );
    assert!(
        transcript
            .iter()
            .any(|line| line == "a5 OK STORE completed")
    );
}
