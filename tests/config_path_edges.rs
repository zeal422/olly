use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use olly::config::{AppConfig, ConfigError};

fn unique_root(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("olly-config-path-tests-{label}-{stamp}"));
    fs::create_dir_all(&root).expect("create temp root");
    root
}

#[test]
fn load_from_missing_path_returns_io_error() {
    let root = unique_root("load-missing");
    let path = root.join("missing-config.toml");

    let err = AppConfig::load_from_path(&path).expect_err("missing config should fail");
    assert!(matches!(err, ConfigError::Io(_)));
}

#[test]
fn save_to_path_returns_io_error_when_parent_segment_is_file() {
    let root = unique_root("save-parent-is-file");
    let blocker = root.join("blocker");
    fs::write(&blocker, "not a directory").expect("create blocker file");
    let config_path = blocker.join("config.toml");

    let mut config = AppConfig::default();
    config.storage.db_path = root
        .join("data")
        .join("workspaces.db")
        .to_string_lossy()
        .into_owned();

    let err = config
        .save_to_path(&config_path)
        .expect_err("save should fail when parent segment is file");
    assert!(matches!(err, ConfigError::Io(_)));
}
