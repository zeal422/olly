pub const DEFAULT_EXCLUDE_GLOBS: &[&str] = &[
    ".env*",
    "*.pem",
    "*.key",
    "id_rsa*",
    ".git/**",
    "node_modules/**",
    "target/**",
    "*.png",
    "*.jpg",
    "*.zip",
    "*.exe",
];

pub fn is_sensitive_or_excluded(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_lowercase();
    let file_name = normalized.rsplit('/').next().unwrap_or_default();

    file_name.starts_with(".env")
        || file_name.starts_with("id_rsa")
        || file_name.ends_with(".pem")
        || file_name.ends_with(".key")
        || file_name.ends_with(".png")
        || file_name.ends_with(".jpg")
        || file_name.ends_with(".zip")
        || file_name.ends_with(".exe")
        || normalized.contains("/.git/")
        || normalized.contains("/node_modules/")
        || normalized.contains("/target/")
}

pub fn within_context_budget(
    file_size_kb: usize,
    total_size_kb: usize,
    max_file_size_kb: usize,
    max_total_size_kb: usize,
) -> bool {
    file_size_kb <= max_file_size_kb && total_size_kb <= max_total_size_kb
}
