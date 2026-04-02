use std::path::PathBuf;
use tempfile::TempDir;
use crate::app::runtime::startup::bootstrap_with_progress;
use crate::config::AppConfig;
use crate::workspace::WorkspaceManager;

#[tokio::test]
async fn startup_model_override_propagates_to_workspace_session() {
    // BUG #6 regression test: Verify CLI model override is used for workspace session open
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let config = AppConfig {
        storage: crate::config::StorageConfig {
            db_path: db_path.to_string_lossy().to_string(),
        },
        defaults: crate::config::DefaultsConfig {
            model: "model-A".to_string(),
            workspace: "Test Workspace".to_string(),
        },
        ..AppConfig::default()
    };

    let config_dir = temp_dir.path().join("config");
    std::fs::create_dir_all(&config_dir).unwrap();
    let config_path = config_dir.join("config.toml");
    config.save_to_path(&config_path).unwrap();

    let url_override = None;
    let model_override = Some("model-C".to_string());

    let result = bootstrap_with_progress(url_override, model_override, |_| {}).await;

    assert!(result.is_ok());
    let bootstrap = result.unwrap();
    let app = &bootstrap.app;

    assert_eq!(app.model, "model-C");

    let preset = bootstrap
        .manager
        .storage
        .get_preset(bootstrap.session.preset_id)
        .unwrap()
        .expect("preset should exist");
    assert_eq!(preset.model, "model-C");
}
