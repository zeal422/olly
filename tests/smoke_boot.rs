mod common;

#[test]
fn app_boot_smoke_placeholder() {
    let server = common::mock_ollama::MockOllamaServer::new(common::PLACEHOLDER_BASE_URL);
    assert!(server.base_url.starts_with("http://"));
}
