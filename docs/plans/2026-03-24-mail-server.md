# Mail Server Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在当前仓库中实现一个可通过 Web 管理的自研邮件服务器，首期支持管理员管理域名和邮箱账号，支持 SMTP 收信与提交、IMAP 读取，并集成 `certbot` 为 Web、SMTP、IMAP 提供统一 TLS。

**Architecture:** 后端采用单进程部署的“事件驱动核心 + 协议适配层”结构。`mail-core` 负责域名、邮箱、消息、IMAP 状态与审计；HTTP、SMTP、IMAP 仅负责协议解析与响应翻译。部署上通过 `docker-compose` 组合 `server`、`frontend`、`postgres`、`nginx` 与 `certbot`，并通过共享卷完成 ACME challenge 和证书热加载。

**Tech Stack:** Rust 2024, Tokio, Axum, SQLx with PostgreSQL, tokio-rustls, serde, React 19, Redux Toolkit, React Router, Vite, Docker Compose, Nginx, Certbot

---

### Task 1: Bootstrap backend crate and application skeleton

**Files:**
- Modify: `server/Cargo.toml`
- Modify: `server/src/main.rs`
- Create: `server/src/lib.rs`
- Create: `server/src/app.rs`
- Create: `server/src/config.rs`
- Create: `server/src/error.rs`
- Test: `server/tests/app_boot.rs`

**Step 1: Write the failing test**

```rust
use server::app::build_app;
use server::config::AppConfig;

#[tokio::test]
async fn builds_application_from_test_config() {
    let config = AppConfig::for_tests();
    let app = build_app(config).await;
    assert!(app.is_ok());
}
```

**Step 2: Run test to verify it fails**

Run: `cd server && cargo test --test app_boot -- --nocapture`
Expected: FAIL because `server::app` and `AppConfig::for_tests` do not exist yet.

**Step 3: Write minimal implementation**

- Add runtime, HTTP, serialization, error, config, async, and testing dependencies to `Cargo.toml`.
- Create a small `lib.rs` that exports `app`, `config`, and `error`.
- Implement `AppConfig` with environment-driven fields plus a `for_tests()` constructor.
- Implement `build_app(config)` returning an initialized application state.
- Update `main.rs` to load config, build the app, and start the runtime entrypoint.

**Step 4: Run test to verify it passes**

Run: `cd server && cargo test --test app_boot -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add server/Cargo.toml server/src/main.rs server/src/lib.rs server/src/app.rs server/src/config.rs server/src/error.rs server/tests/app_boot.rs
git commit -m "feat: bootstrap mail server skeleton"
```

### Task 2: Add database schema, repositories, and core entities

**Files:**
- Modify: `server/Cargo.toml`
- Create: `server/migrations/0001_initial_schema.sql`
- Create: `server/src/db/mod.rs`
- Create: `server/src/db/models.rs`
- Create: `server/src/db/repositories.rs`
- Create: `server/src/core/mod.rs`
- Create: `server/src/core/entities.rs`
- Test: `server/tests/domain_mailbox_repository.rs`

**Step 1: Write the failing test**

```rust
use server::core::entities::{DomainName, MailboxAddress};
use server::db::repositories::MailboxRepository;

#[tokio::test]
async fn creates_domain_and_mailbox_records() {
    let ctx = server::db::TestDatabase::new().await;
    let domain = ctx.repositories.domains.create(DomainName::new("example.com").unwrap()).await.unwrap();
    let mailbox = ctx.repositories.mailboxes
        .create(MailboxAddress::new("alice@example.com").unwrap(), domain.id)
        .await
        .unwrap();

    assert_eq!(mailbox.local_part, "alice");
    assert_eq!(mailbox.domain_id, domain.id);
}
```

**Step 2: Run test to verify it fails**

Run: `cd server && cargo test --test domain_mailbox_repository -- --nocapture`
Expected: FAIL because the schema, repositories, and test database helper do not exist.

**Step 3: Write minimal implementation**

- Add SQLx PostgreSQL dependencies and migration support.
- Create the initial schema for `admins`, `domains`, `mailboxes`, `mail_folders`, `messages`, `message_delivery`, `message_flags`, `imap_folder_state`, `imap_message_uids`, and `audit_logs`.
- Implement a `TestDatabase` helper for integration tests.
- Add typed entities for `DomainName` and `MailboxAddress`.
- Implement repository methods for creating and fetching domains and mailboxes.

**Step 4: Run test to verify it passes**

Run: `cd server && cargo test --test domain_mailbox_repository -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add server/Cargo.toml server/migrations/0001_initial_schema.sql server/src/db/mod.rs server/src/db/models.rs server/src/db/repositories.rs server/src/core/mod.rs server/src/core/entities.rs server/tests/domain_mailbox_repository.rs
git commit -m "feat: add mail storage schema and repositories"
```

### Task 3: Implement core commands for domain, mailbox, and folder provisioning

**Files:**
- Modify: `server/src/core/mod.rs`
- Create: `server/src/core/commands.rs`
- Create: `server/src/core/service.rs`
- Create: `server/src/core/events.rs`
- Test: `server/tests/provision_mailbox.rs`

**Step 1: Write the failing test**

```rust
use server::core::service::MailCoreService;

#[tokio::test]
async fn provisioning_mailbox_creates_default_folders() {
    let ctx = server::db::TestDatabase::new().await;
    let service = MailCoreService::new(ctx.repositories.clone());

    let mailbox = service
        .provision_mailbox("example.com", "alice", "password123")
        .await
        .unwrap();

    let folders = ctx.repositories.folders.list_for_mailbox(mailbox.id).await.unwrap();
    let names: Vec<_> = folders.into_iter().map(|folder| folder.name).collect();

    assert!(names.contains(&"INBOX".to_string()));
    assert!(names.contains(&"Sent".to_string()));
    assert!(names.contains(&"Drafts".to_string()));
    assert!(names.contains(&"Trash".to_string()));
}
```

**Step 2: Run test to verify it fails**

Run: `cd server && cargo test --test provision_mailbox -- --nocapture`
Expected: FAIL because `MailCoreService` and folder provisioning do not exist.

**Step 3: Write minimal implementation**

- Introduce command handlers for creating domains and provisioning mailboxes.
- Hash mailbox passwords using a dedicated password-hash utility.
- Create default folders and IMAP folder state inside one transaction.
- Emit a `MailboxProvisioned` domain event after successful provisioning.

**Step 4: Run test to verify it passes**

Run: `cd server && cargo test --test provision_mailbox -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add server/src/core/mod.rs server/src/core/commands.rs server/src/core/service.rs server/src/core/events.rs server/tests/provision_mailbox.rs
git commit -m "feat: add core mailbox provisioning flow"
```

### Task 4: Add admin authentication and domain/mailbox management HTTP API

**Files:**
- Create: `server/src/http/mod.rs`
- Create: `server/src/http/state.rs`
- Create: `server/src/http/routes/auth.rs`
- Create: `server/src/http/routes/domains.rs`
- Create: `server/src/http/routes/mailboxes.rs`
- Create: `server/src/http/routes/system.rs`
- Modify: `server/src/app.rs`
- Test: `server/tests/admin_api.rs`

**Step 1: Write the failing test**

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
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
```

**Step 2: Run test to verify it fails**

Run: `cd server && cargo test --test admin_api -- --nocapture`
Expected: FAIL because the HTTP routes and test app builder do not exist.

**Step 3: Write minimal implementation**

- Add an Axum router with admin endpoints for login, domains, mailboxes, health, and certificate status.
- Implement a minimal admin authentication strategy suitable for phase 1, such as seeded admin credentials plus signed session token or test header helper.
- Connect HTTP handlers to `MailCoreService`.
- Return structured JSON errors mapped from core errors.

**Step 4: Run test to verify it passes**

Run: `cd server && cargo test --test admin_api -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add server/src/http/mod.rs server/src/http/state.rs server/src/http/routes/auth.rs server/src/http/routes/domains.rs server/src/http/routes/mailboxes.rs server/src/http/routes/system.rs server/src/app.rs server/tests/admin_api.rs
git commit -m "feat: add admin management api"
```

### Task 5: Implement SMTP adapter for authenticated submission and local inbound storage

**Files:**
- Create: `server/src/smtp/mod.rs`
- Create: `server/src/smtp/session.rs`
- Create: `server/src/smtp/parser.rs`
- Modify: `server/src/core/service.rs`
- Modify: `server/src/app.rs`
- Test: `server/tests/smtp_flow.rs`

**Step 1: Write the failing test**

```rust
#[tokio::test]
async fn smtp_data_command_stores_message_for_local_mailbox() {
    let harness = server::smtp::test_support::spawn().await;
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
        ])
        .await;

    assert!(transcript.iter().any(|line| line.contains("250 Message accepted")));
}
```

**Step 2: Run test to verify it fails**

Run: `cd server && cargo test --test smtp_flow -- --nocapture`
Expected: FAIL because the SMTP adapter and test harness do not exist.

**Step 3: Write minimal implementation**

- Implement a basic line-oriented SMTP session state machine.
- Support `EHLO`, `AUTH PLAIN`, `MAIL FROM`, `RCPT TO`, `DATA`, and graceful connection close.
- Delegate recipient validation and message storage to `MailCoreService`.
- Store the raw RFC822 message and create `message_delivery` and default `INBOX` mappings.

**Step 4: Run test to verify it passes**

Run: `cd server && cargo test --test smtp_flow -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add server/src/smtp/mod.rs server/src/smtp/session.rs server/src/smtp/parser.rs server/src/core/service.rs server/src/app.rs server/tests/smtp_flow.rs
git commit -m "feat: add smtp inbound message flow"
```

### Task 6: Implement IMAP adapter for login, folder listing, and message fetch

**Files:**
- Create: `server/src/imap/mod.rs`
- Create: `server/src/imap/session.rs`
- Create: `server/src/imap/parser.rs`
- Modify: `server/src/core/service.rs`
- Modify: `server/src/app.rs`
- Test: `server/tests/imap_flow.rs`

**Step 1: Write the failing test**

```rust
#[tokio::test]
async fn imap_fetch_returns_subject_for_inbox_message() {
    let harness = server::imap::test_support::spawn_with_seed_message().await;
    let transcript = harness
        .run([
            "a1 LOGIN alice@example.com password123",
            "a2 SELECT INBOX",
            "a3 FETCH 1 BODY[HEADER.FIELDS (SUBJECT)]",
        ])
        .await;

    assert!(transcript.iter().any(|line| line.contains("Subject: hello")));
}
```

**Step 2: Run test to verify it fails**

Run: `cd server && cargo test --test imap_flow -- --nocapture`
Expected: FAIL because the IMAP adapter and seed helpers do not exist.

**Step 3: Write minimal implementation**

- Implement a minimal tagged-command IMAP parser and session state machine.
- Support `LOGIN`, `LIST`, `SELECT`, `FETCH`, `STORE`, `SEARCH`, `EXPUNGE`, and basic `UID` variants needed by the selected client flow.
- Reuse `MailCoreService` for mailbox authentication and message reads.
- Ensure folder state, UID allocation, and flags are read from the database instead of being computed on the fly.

**Step 4: Run test to verify it passes**

Run: `cd server && cargo test --test imap_flow -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add server/src/imap/mod.rs server/src/imap/session.rs server/src/imap/parser.rs server/src/core/service.rs server/src/app.rs server/tests/imap_flow.rs
git commit -m "feat: add imap read flow"
```

### Task 7: Add TLS adapter and certificate hot reload support

**Files:**
- Create: `server/src/tls.rs`
- Modify: `server/src/config.rs`
- Modify: `server/src/app.rs`
- Test: `server/tests/tls_reload.rs`

**Step 1: Write the failing test**

```rust
use std::path::PathBuf;

#[tokio::test]
async fn reloads_certificate_bundle_when_files_change() {
    let cert_dir = PathBuf::from("tests/fixtures/certs/reloadable");
    let manager = server::tls::TlsManager::load_from_dir(&cert_dir).await.unwrap();

    let before = manager.current_serial().await;
    server::tls::test_support::swap_certificate_fixture(&cert_dir).await.unwrap();
    manager.reload_if_changed().await.unwrap();
    let after = manager.current_serial().await;

    assert_ne!(before, after);
}
```

**Step 2: Run test to verify it fails**

Run: `cd server && cargo test --test tls_reload -- --nocapture`
Expected: FAIL because the TLS manager and reload support do not exist.

**Step 3: Write minimal implementation**

- Add TLS configuration fields for certificate and private key paths.
- Implement a `TlsManager` that loads `fullchain.pem` and `privkey.pem`, tracks a version or serial number, and can reload on file changes.
- Expose the current certificate status to the HTTP admin system endpoint.
- Wire the SMTP and IMAP listeners to use the shared TLS manager for `STARTTLS` and implicit TLS ports.

**Step 4: Run test to verify it passes**

Run: `cd server && cargo test --test tls_reload -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add server/src/tls.rs server/src/config.rs server/src/app.rs server/tests/tls_reload.rs
git commit -m "feat: add tls reload support"
```

### Task 8: Build the admin frontend for domains, mailboxes, status, and certificates

**Files:**
- Modify: `frontend/src/App.tsx`
- Modify: `frontend/src/App.css`
- Create: `frontend/src/features/admin/api.ts`
- Create: `frontend/src/features/admin/pages/LoginPage.tsx`
- Create: `frontend/src/features/admin/pages/DashboardPage.tsx`
- Create: `frontend/src/features/admin/pages/DomainsPage.tsx`
- Create: `frontend/src/features/admin/pages/MailboxesPage.tsx`
- Create: `frontend/src/features/admin/pages/SystemPage.tsx`
- Modify: `frontend/src/router/index.tsx`
- Modify: `frontend/src/store/index.ts`

**Step 1: Write the failing build-level check**

Run: `cd frontend && npm run build`
Expected: FAIL once route imports reference the new admin pages before they exist.

**Step 2: Add API client and pages**

- Create a small fetch wrapper for admin authentication and CRUD requests.
- Add pages for login, dashboard, domains, mailboxes, and system status.
- Update the router to point `/` to the admin dashboard and include the new sections.
- Keep state management simple and colocated unless cross-page coordination demands a Redux slice.

**Step 3: Add intentional admin UI styling**

- Replace the default Vite starter screen with a mail-ops dashboard look.
- Preserve the existing project conventions while giving the app a clear operational visual identity.

**Step 4: Run build to verify it passes**

Run: `cd frontend && npm run build`
Expected: PASS.

**Step 5: Run lint**

Run: `cd frontend && npm run lint`
Expected: PASS.

**Step 6: Commit**

```bash
git add frontend/src/App.tsx frontend/src/App.css frontend/src/features/admin/api.ts frontend/src/features/admin/pages/LoginPage.tsx frontend/src/features/admin/pages/DashboardPage.tsx frontend/src/features/admin/pages/DomainsPage.tsx frontend/src/features/admin/pages/MailboxesPage.tsx frontend/src/features/admin/pages/SystemPage.tsx frontend/src/router/index.tsx frontend/src/store/index.ts
git commit -m "feat: add admin mail operations frontend"
```

### Task 9: Add Docker Compose, Nginx, and Certbot integration

**Files:**
- Create: `docker-compose.yml`
- Create: `server/Dockerfile`
- Create: `frontend/Dockerfile`
- Create: `infra/nginx/default.conf`
- Create: `infra/certbot/renew.sh`
- Create: `.env.example`

**Step 1: Write the failing deployment verification**

Run: `docker compose config`
Expected: FAIL because the compose file and referenced infrastructure files do not exist.

**Step 2: Add container definitions**

- Define services for `postgres`, `server`, `frontend`, `nginx`, and `certbot`.
- Mount a shared ACME webroot and shared certificate directory.
- Expose `25`, `587`, `143`, `993`, `80`, and `443`.
- Add the environment variables required by the Rust config layer and frontend API target.

**Step 3: Add Nginx and Certbot automation**

- Configure Nginx to serve `/.well-known/acme-challenge/` and proxy the admin frontend/API.
- Add a renewal script that loops `certbot renew` and keeps logs visible.
- Ensure the certificate volume path matches the Rust TLS config.

**Step 4: Run deployment verification**

Run: `docker compose config`
Expected: PASS with a valid merged configuration.

**Step 5: Commit**

```bash
git add docker-compose.yml server/Dockerfile frontend/Dockerfile infra/nginx/default.conf infra/certbot/renew.sh .env.example
git commit -m "feat: add compose deployment with certbot"
```

### Task 10: Run end-to-end verification and fix integration gaps

**Files:**
- Review: `server/tests/admin_api.rs`
- Review: `server/tests/smtp_flow.rs`
- Review: `server/tests/imap_flow.rs`
- Review: `server/tests/tls_reload.rs`
- Review: `frontend/src/features/admin/pages/SystemPage.tsx`
- Review: `docker-compose.yml`

**Step 1: Run backend tests**

Run: `cd server && cargo test`
Expected: PASS.

**Step 2: Run formatting check**

Run: `cd server && cargo fmt --check`
Expected: PASS.

**Step 3: Run frontend lint**

Run: `cd frontend && npm run lint`
Expected: PASS.

**Step 4: Run frontend build**

Run: `cd frontend && npm run build`
Expected: PASS.

**Step 5: Run compose validation**

Run: `docker compose config`
Expected: PASS.

**Step 6: Fix only verified integration regressions**

Address any failures found in the commands above without expanding feature scope.

**Step 7: Commit**

```bash
git add server frontend docker-compose.yml infra .env.example
git commit -m "chore: verify mail server integration"
```
