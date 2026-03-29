#[derive(Debug, Clone)]
pub struct TempDb {
    pub path: String,
}

impl TempDb {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}
