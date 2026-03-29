use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml decode error: {0}")]
    TomlDecode(#[from] toml::de::Error),
    #[error("toml encode error: {0}")]
    TomlEncode(#[from] toml::ser::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OllamaConfig {
    pub url: String,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DefaultsConfig {
    pub model: String,
    pub workspace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    pub db_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub syntax_highlighting: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeybindingsConfig {
    pub send: String,
    pub newline: String,
    pub quit: String,
    pub help: String,
    pub dismiss: String,
    pub switch_focus: String,
    pub autocomplete: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    pub ui: UiConfig,
    pub ollama: OllamaConfig,
    pub defaults: DefaultsConfig,
    pub storage: StorageConfig,
    pub keybindings: KeybindingsConfig,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            url: String::from("http://localhost:11434"),
            timeout_secs: 300,
        }
    }
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            model: String::from("llama3.2:latest"),
            workspace: String::from("Default Workspace"),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        let data_dir = default_data_dir();
        Self {
            db_path: data_dir
                .join("workspaces.db")
                .to_string_lossy()
                .into_owned(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            syntax_highlighting: true,
        }
    }
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            send: String::from("Ctrl+Enter"),
            newline: String::from("Enter"),
            quit: String::from("Ctrl+Q"),
            help: String::from("?"),
            dismiss: String::from("Esc"),
            switch_focus: String::from("Tab"),
            autocomplete: String::from("Tab"),
        }
    }
}

impl AppConfig {
    pub fn load_or_create() -> Result<Self, ConfigError> {
        let path = config_file_path();
        if path.exists() {
            return Self::load_from_path(&path);
        }

        let default = Self::default();
        default.save_to_path(&path)?;
        Ok(default)
    }

    pub fn load_from_path(path: &Path) -> Result<Self, ConfigError> {
        let raw = fs::read_to_string(path)?;
        Ok(toml::from_str(&raw)?)
    }

    pub fn save_to_path(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(data_parent) = Path::new(&self.storage.db_path).parent() {
            fs::create_dir_all(data_parent)?;
        }
        let raw = toml::to_string_pretty(self)?;
        fs::write(path, raw)?;
        Ok(())
    }
}

pub fn config_file_path() -> PathBuf {
    default_config_dir().join("config.toml")
}

pub fn default_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("olly")
}

pub fn default_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("olly")
}
