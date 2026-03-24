use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use rcgen::generate_simple_self_signed;
use std::fs;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tower::ServiceExt;

#[tokio::test]
async fn creates_domain_via_admin_api() {
    let app = server::app::build_test_app().await.unwrap();

    let response = app
        .oneshot(
            Request::post("/api/admin/domains")
                .header("content-type", "application/json")
                .header("x-admin-token", "test-admin-token")
                .body(Body::from(r#"{"domain":"example.com"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn lists_and_deletes_domains_via_admin_api() {
    let app = server::app::build_test_app().await.unwrap();

    let created = app
        .clone()
        .oneshot(
            Request::post("/api/admin/domains")
                .header("content-type", "application/json")
                .header("x-admin-token", "test-admin-token")
                .body(Body::from(r#"{"domain":"example.com"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);

    let list_response = app
        .clone()
        .oneshot(
            Request::get("/api/admin/domains")
                .header("x-admin-token", "test-admin-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);

    let list_body = list_response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let list_payload: serde_json::Value = serde_json::from_slice(&list_body).unwrap();
    let domain_id = list_payload[0]["id"].as_i64().unwrap();

    let delete_response = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/admin/domains/{domain_id}"))
                .header("x-admin-token", "test-admin-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn creates_and_lists_mailboxes_via_admin_api() {
    let app = server::app::build_test_app().await.unwrap();

    let create_mailbox = app
        .clone()
        .oneshot(
            Request::post("/api/admin/mailboxes")
                .header("content-type", "application/json")
                .header("x-admin-token", "test-admin-token")
                .body(Body::from(
                    r#"{"domain":"example.com","local_part":"alice","password":"password123"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_mailbox.status(), StatusCode::CREATED);

    let list_response = app
        .oneshot(
            Request::get("/api/admin/mailboxes")
                .header("x-admin-token", "test-admin-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);

    let list_body = list_response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let list_payload: serde_json::Value = serde_json::from_slice(&list_body).unwrap();

    assert_eq!(list_payload[0]["email"], "alice@example.com");
}

#[tokio::test]
async fn updates_domain_via_admin_api() {
    let app = server::app::build_test_app().await.unwrap();

    let created = app
        .clone()
        .oneshot(
            Request::post("/api/admin/domains")
                .header("content-type", "application/json")
                .header("x-admin-token", "test-admin-token")
                .body(Body::from(r#"{"domain":"example.com"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let body = created.into_body().collect().await.unwrap().to_bytes();
    let domain: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let domain_id = domain["id"].as_i64().unwrap();

    let updated = app
        .clone()
        .oneshot(
            Request::patch(format!("/api/admin/domains/{domain_id}"))
                .header("content-type", "application/json")
                .header("x-admin-token", "test-admin-token")
                .body(Body::from(r#"{"name":"example.net","enabled":false}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(updated.status(), StatusCode::OK);
    let body = updated.into_body().collect().await.unwrap().to_bytes();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["name"], "example.net");
    assert_eq!(payload["enabled"], false);
}

#[tokio::test]
async fn updates_mailbox_via_admin_api() {
    let app = server::app::build_test_app().await.unwrap();

    let created = app
        .clone()
        .oneshot(
            Request::post("/api/admin/mailboxes")
                .header("content-type", "application/json")
                .header("x-admin-token", "test-admin-token")
                .body(Body::from(
                    r#"{"domain":"example.com","local_part":"alice","password":"password123"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let body = created.into_body().collect().await.unwrap().to_bytes();
    let mailbox: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let mailbox_id = mailbox["id"].as_i64().unwrap();

    let updated = app
        .clone()
        .oneshot(
            Request::patch(format!("/api/admin/mailboxes/{mailbox_id}"))
                .header("content-type", "application/json")
                .header("x-admin-token", "test-admin-token")
                .body(Body::from(r#"{"enabled":false,"password":"new-password"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(updated.status(), StatusCode::OK);
    let body = updated.into_body().collect().await.unwrap().to_bytes();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["enabled"], false);
    assert_eq!(payload["email"], "alice@example.com");
}

#[tokio::test]
async fn lists_admin_audit_logs() {
    let app = server::app::build_test_app().await.unwrap();

    let created = app
        .clone()
        .oneshot(
            Request::post("/api/admin/domains")
                .header("content-type", "application/json")
                .header("x-admin-token", "test-admin-token")
                .body(Body::from(r#"{"domain":"example.com"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let body = created.into_body().collect().await.unwrap().to_bytes();
    let domain: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let domain_id = domain["id"].as_i64().unwrap();

    let mailbox_created = app
        .clone()
        .oneshot(
            Request::post("/api/admin/mailboxes")
                .header("content-type", "application/json")
                .header("x-admin-token", "test-admin-token")
                .body(Body::from(
                    r#"{"domain":"example.com","local_part":"alice","password":"password123"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mailbox_created.status(), StatusCode::CREATED);
    let body = mailbox_created
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let mailbox: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let mailbox_id = mailbox["id"].as_i64().unwrap();

    let _ = app
        .clone()
        .oneshot(
            Request::patch(format!("/api/admin/domains/{domain_id}"))
                .header("content-type", "application/json")
                .header("x-admin-token", "test-admin-token")
                .body(Body::from(r#"{"enabled":false}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    let _ = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/admin/mailboxes/{mailbox_id}"))
                .header("x-admin-token", "test-admin-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let _ = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/admin/domains/{domain_id}"))
                .header("x-admin-token", "test-admin-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let listed = app
        .oneshot(
            Request::get("/api/admin/audit-logs")
                .header("x-admin-token", "test-admin-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(listed.status(), StatusCode::OK);
    let body = listed.into_body().collect().await.unwrap().to_bytes();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let actions = payload
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["action"].as_str())
        .collect::<Vec<_>>();
    assert!(actions.contains(&"domain.created"));
    assert!(
        actions.contains(&"mailbox.provisioned"),
        "expected mailbox.provisioned in audit logs, got {actions:?}"
    );
    assert!(actions.contains(&"domain.updated"));
    assert!(actions.contains(&"mailbox.deleted"));
    assert!(actions.contains(&"domain.deleted"));
}

#[tokio::test]
async fn returns_certificate_subjects_and_expiry_from_system_api() {
    let test_database = Arc::new(server::db::TestDatabase::new().await);
    let cert_dir = write_test_certificate_dir().await;
    let config = server::config::AppConfig {
        host: "127.0.0.1".to_string(),
        port: 0,
        smtp_port: 0,
        submission_port: 0,
        imap_port: 0,
        imaps_port: 0,
        database_url: test_database.database_url.clone(),
        admin_token: "test-admin-token".to_string(),
        tls_cert_dir: Some(cert_dir.clone()),
    };

    let runtime = server::app::build_runtime_services(config).await.unwrap();
    let app = runtime.http_app;

    let response = app
        .oneshot(
            Request::get("/api/admin/system/certificates")
                .header("x-admin-token", "test-admin-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], "loaded");
    assert!(
        payload["subject_names"]
            .as_array()
            .unwrap()
            .iter()
            .any(|name| name == "localhost")
    );
    assert!(payload["expires_at"].as_str().is_some());
    assert!(payload["last_reloaded_at"].is_null());
}

async fn write_test_certificate_dir() -> std::path::PathBuf {
    let cert = generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let dir = std::env::temp_dir().join(format!(
        "rustmailer-admin-api-certs-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("fullchain.pem"), cert.cert.pem()).unwrap();
    fs::write(dir.join("privkey.pem"), cert.signing_key.serialize_pem()).unwrap();
    dir
}
