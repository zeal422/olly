use olly::app::context::{is_sensitive_or_excluded, within_context_budget, DEFAULT_EXCLUDE_GLOBS};
use olly::workspace::context::{has_hash_changed, hash_text};

#[test]
fn sensitive_paths_are_rejected() {
    assert!(is_sensitive_or_excluded("C:/work/.env"));
    assert!(is_sensitive_or_excluded("C:/work/.git/config"));
    assert!(is_sensitive_or_excluded("C:/work/keys/private.pem"));
    assert!(is_sensitive_or_excluded(
        "C:/work/node_modules/pkg/index.js"
    ));
}

#[test]
fn context_budget_checks_limits() {
    assert!(within_context_budget(200, 1000, 512, 2048));
    assert!(!within_context_budget(600, 1000, 512, 2048));
    assert!(!within_context_budget(200, 2500, 512, 2048));
    assert!(DEFAULT_EXCLUDE_GLOBS.contains(&".env*"));
}

#[test]
fn context_hash_change_detection() {
    let first = hash_text("fn main() { println!(\"a\"); }");
    let second = hash_text("fn main() { println!(\"b\"); }");
    let same = hash_text("fn main() { println!(\"a\"); }");

    assert!(has_hash_changed(&first, &second));
    assert!(!has_hash_changed(&first, &same));
}
