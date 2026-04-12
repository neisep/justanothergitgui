use std::env;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::commit_rules::CommitMessageRuleSet;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppSettings {
    #[serde(default)]
    pub commit_message_ruleset: CommitMessageRuleSet,
    #[serde(default)]
    pub commit_message_custom_scopes: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            commit_message_ruleset: CommitMessageRuleSet::Off,
            commit_message_custom_scopes: Vec::new(),
        }
    }
}

pub fn load_app_settings() -> Result<AppSettings, String> {
    let path = settings_path();
    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let payload = fs::read_to_string(&path)
        .map_err(|error| format!("Could not read settings file {}: {}", path.display(), error))?;

    serde_json::from_str(&payload).map_err(|error| {
        format!(
            "Could not parse settings file {}: {}",
            path.display(),
            error
        )
    })
}

pub fn save_app_settings(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Could not create settings directory {}: {}",
                parent.display(),
                error
            )
        })?;
    }

    let payload = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("Could not serialize settings: {}", error))?;

    fs::write(&path, payload).map_err(|error| {
        format!(
            "Could not write settings file {}: {}",
            path.display(),
            error
        )
    })
}

fn settings_path() -> PathBuf {
    settings_dir().join("settings.json")
}

fn settings_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("justanothergitgui");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("justanothergitgui");
        }
    }

    if let Some(config_home) = env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(config_home).join("justanothergitgui");
    }

    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("justanothergitgui");
    }

    env::temp_dir().join("justanothergitgui")
}
