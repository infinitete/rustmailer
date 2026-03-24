use server::app::build_app;
use server::config::AppConfig;

#[tokio::test]
async fn builds_application_from_test_config() {
    let config = AppConfig::for_tests();
    let app = build_app(config).await;
    assert!(app.is_ok());
}
