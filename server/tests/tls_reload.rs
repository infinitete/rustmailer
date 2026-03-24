use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rcgen::generate_simple_self_signed;
use server::tls;
use tokio::time::{Duration, sleep};

#[tokio::test]
async fn reloads_certificate_bundle_when_files_change() {
    let cert_dir = write_reloadable_certificate_dir();
    let manager = tls::TlsManager::load_from_dir(&cert_dir).await.unwrap();

    let before = manager.current_serial().await;
    tls::test_support::swap_certificate_fixture(&cert_dir)
        .await
        .unwrap();
    manager.reload_if_changed().await.unwrap();
    let after = manager.current_serial().await;

    assert_ne!(before, after);
}

#[tokio::test]
async fn reload_updates_last_reloaded_timestamp_and_metadata() {
    let cert_dir = write_reloadable_certificate_dir();
    let manager = tls::TlsManager::load_from_dir(&cert_dir).await.unwrap();

    assert!(manager.last_reloaded_at().await.is_none());

    tls::test_support::swap_certificate_fixture(&cert_dir)
        .await
        .unwrap();
    manager.reload_if_changed().await.unwrap();

    assert_eq!(
        manager.subject_names().await,
        vec!["mail.local".to_string()]
    );
    assert!(manager.last_reloaded_at().await.is_some());
}

#[tokio::test]
async fn background_reload_task_picks_up_certificate_changes() {
    let cert_dir = write_reloadable_certificate_dir();
    let manager = Arc::new(tls::TlsManager::load_from_dir(&cert_dir).await.unwrap());
    let before = manager.current_serial().await;
    let handle = server::app::spawn_tls_reload_task(manager.clone(), Duration::from_millis(25));

    tls::test_support::swap_certificate_fixture(&cert_dir)
        .await
        .unwrap();

    let mut after = before;
    for _ in 0..20 {
        sleep(Duration::from_millis(25)).await;
        after = manager.current_serial().await;
        if after > before {
            break;
        }
    }
    handle.abort();

    assert!(after > before, "expected reload task to bump serial");
    assert_eq!(
        manager.subject_names().await,
        vec!["mail.local".to_string()]
    );
    assert!(manager.last_reloaded_at().await.is_some());
}

fn write_reloadable_certificate_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rustmailer-reloadable-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();

    write_certificate_pair(&dir, "base", "localhost");
    write_certificate_pair(&dir, "next", "mail.local");
    fs::copy(dir.join("fullchain.base.pem"), dir.join("fullchain.pem")).unwrap();
    fs::copy(dir.join("privkey.base.pem"), dir.join("privkey.pem")).unwrap();

    dir
}

fn write_certificate_pair(dir: &Path, suffix: &str, server_name: &str) {
    let cert = generate_simple_self_signed(vec![server_name.to_string()]).unwrap();
    fs::write(dir.join(format!("fullchain.{suffix}.pem")), cert.cert.pem()).unwrap();
    fs::write(
        dir.join(format!("privkey.{suffix}.pem")),
        cert.signing_key.serialize_pem(),
    )
    .unwrap();
}
