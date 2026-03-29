use std::{fs, path::Path};

use sha2::{Digest, Sha256};

use crate::workspace::{error::WorkspaceError, ContextFileMeta};

pub fn read_file(path: &Path) -> Result<String, WorkspaceError> {
    Ok(fs::read_to_string(path)?)
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub fn hash_text(content: &str) -> String {
    hash_bytes(content.as_bytes())
}

pub fn has_hash_changed(previous_hash: &str, new_hash: &str) -> bool {
    previous_hash != new_hash
}

pub fn file_meta_for_workspace(
    workspace_id: i64,
    path: &Path,
    content: &str,
) -> Result<ContextFileMeta, WorkspaceError> {
    let metadata = fs::metadata(path)?;
    Ok(ContextFileMeta {
        workspace_id,
        path: path.to_string_lossy().into_owned(),
        hash: hash_text(content),
        size_bytes: metadata.len() as usize,
    })
}
