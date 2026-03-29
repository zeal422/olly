use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use olly::config::{self, AppConfig, ConfigError};

fn unique_temp_dir(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("olly-config-tests-{label}-{stamp}"));
    fs::create_dir_all(&root).expect("create temp root");
    root
}

#[test]
fn save_to_path_creates_missing_parent_and_data_directories() {
    let root = unique_temp_dir("save-path-create");
    let config_path = root.join("config-dir").join("nested").join("config.toml");
    let db_path = root.join("data-dir").join("nested").join("workspaces.db");

    let config_parent = config_path
        .parent()
        .expect("config parent exists")
        .to_path_buf();
    let data_parent = db_path.parent().expect("data parent exists").to_path_buf();
    assert!(!config_parent.exists());
    assert!(!data_parent.exists());

    let mut config = AppConfig::default();
    config.storage.db_path = db_path.to_string_lossy().into_owned();
    config
        .save_to_path(&config_path)
        .expect("save config should succeed");

    assert!(config_parent.exists());
    assert!(data_parent.exists());
    assert!(config_path.exists());
}

#[test]
fn save_and_load_roundtrip_preserves_custom_values() {
    let root = unique_temp_dir("save-load-roundtrip");
    let config_path = root.join("config.toml");
    let db_path = root.join("data").join("db.sqlite3");

    let mut config = AppConfig::default();
    config.ui.syntax_highlighting = false;
    config.defaults.model = String::from("qwen2.5-coder:7b");
    config.defaults.workspace = String::from("My Workspace");
    config.keybindings.send = String::from("Alt+Enter");
    config.keybindings.autocomplete = String::from("Ctrl+Space");
    config.storage.db_path = db_path.to_string_lossy().into_owned();

    config
        .save_to_path(&config_path)
        .expect("save config should succeed");
    let loaded = AppConfig::load_from_path(&config_path).expect("load config should succeed");

    assert!(!loaded.ui.syntax_highlighting);
    assert_eq!(loaded.defaults.model, "qwen2.5-coder:7b");
    assert_eq!(loaded.defaults.workspace, "My Workspace");
    assert_eq!(loaded.keybindings.send, "Alt+Enter");
    assert_eq!(loaded.keybindings.autocomplete, "Ctrl+Space");
    assert_eq!(loaded.storage.db_path, db_path.to_string_lossy());
}

#[test]
fn partial_toml_loads_with_defaults_for_missing_sections() {
    let root = unique_temp_dir("partial-defaults");
    let config_path = root.join("config.toml");
    let partial = r#"
[ollama]
url = "http://remote-ollama:11434"
"#;
    fs::write(&config_path, partial).expect("write partial config");

    let loaded = AppConfig::load_from_path(&config_path).expect("load partial config");
    assert_eq!(loaded.ollama.url, "http://remote-ollama:11434");
    assert_eq!(loaded.ollama.timeout_secs, 300);
    assert_eq!(loaded.defaults.model, "llama3.2:latest");
    assert_eq!(loaded.keybindings.send, "Ctrl+Enter");
    assert!(loaded.ui.syntax_highlighting);
}

#[test]
fn invalid_toml_returns_decode_error() {
    let root = unique_temp_dir("invalid-toml");
    let config_path = root.join("broken.toml");
    fs::write(&config_path, "this = [is invalid toml").expect("write invalid toml");

    let err = AppConfig::load_from_path(&config_path).expect_err("invalid toml should fail");
    assert!(matches!(err, ConfigError::TomlDecode(_)));
}

#[test]
fn default_path_helpers_end_with_olly_segment() {
    let config_dir = config::default_config_dir();
    let data_dir = config::default_data_dir();
    assert_eq!(
        config_dir.file_name().and_then(|value| value.to_str()),
        Some("olly")
    );
    assert_eq!(
        data_dir.file_name().and_then(|value| value.to_str()),
        Some("olly")
    );
}
