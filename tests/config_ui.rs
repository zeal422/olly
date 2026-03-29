use olly::config::AppConfig;

#[test]
fn syntax_highlighting_defaults_to_enabled() {
    let config = AppConfig::default();
    assert!(config.ui.syntax_highlighting);
}

#[test]
fn missing_ui_section_uses_default_highlighting_setting() {
    let raw = r#"
        [ollama]
        url = "http://localhost:11434"
        timeout_secs = 300

        [defaults]
        model = "llama3.2:latest"
        workspace = "Default Workspace"

        [storage]
        db_path = "C:/tmp/olly.db"
    "#;

    let parsed: AppConfig = toml::from_str(raw).expect("parse config");
    assert!(parsed.ui.syntax_highlighting);
}
