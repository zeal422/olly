use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone)]
pub struct TempDb {
    path: PathBuf,
    root_dir: PathBuf,
}

impl TempDb {
    pub fn new(test_name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let root_dir = std::env::temp_dir().join(format!(
            "olly-tests-{test_name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&root_dir).expect("create temp dir");
        let path = root_dir.join("workspaces.db");
        Self { path, root_dir }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root_dir);
    }
}
