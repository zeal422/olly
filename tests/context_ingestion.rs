#[test]
fn sensitive_globs_placeholder() {
    let excluded = [
        ".env*",
        "*.pem",
        "*.key",
        "id_rsa*",
        ".git/**",
        "node_modules/**",
        "target/**",
    ];
    assert!(excluded.contains(&".env*"));
    assert!(excluded.contains(&"target/**"));
}
