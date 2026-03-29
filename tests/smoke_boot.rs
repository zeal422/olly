#[test]
fn app_boot_smoke_defaults() {
    let app = olly::app::App::new(
        String::from("http://localhost:11434"),
        String::from("llama3.2:latest"),
    );
    assert_eq!(app.input, "");
    assert_eq!(app.status, "Ready");
    assert_eq!(app.model, "llama3.2:latest");
    assert_eq!(app.url, "http://localhost:11434");
    assert_eq!(app.workspace_name, "Default Workspace");
    assert!(!app.show_help);
    assert!(!app.is_loading);
    assert!(app.syntax_highlighting);
    assert!(app.messages.is_empty());
    assert!(app.models.is_empty());
}
