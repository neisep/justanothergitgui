use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::settings;

/// Persisted UI session: which repositories were open and which was active,
/// so the app can restore the user's tabs on the next launch. This is distinct
/// from [`crate::settings::AppSettings`] (user preferences).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SessionState {
    #[serde(default)]
    pub open_repos: Vec<PathBuf>,
    #[serde(default)]
    pub active_repo: Option<PathBuf>,
}

pub fn load_session() -> Result<SessionState, String> {
    let path = session_path();
    if !path.exists() {
        return Ok(SessionState::default());
    }

    let payload = fs::read_to_string(&path)
        .map_err(|error| format!("Could not read session file {}: {}", path.display(), error))?;

    serde_json::from_str(&payload)
        .map_err(|error| format!("Could not parse session file {}: {}", path.display(), error))
}

pub fn save_session(session: &SessionState) -> Result<(), String> {
    let path = session_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Could not create session directory {}: {}",
                parent.display(),
                error
            )
        })?;
    }

    let payload = serde_json::to_string_pretty(session)
        .map_err(|error| format!("Could not serialize session: {}", error))?;

    fs::write(&path, payload)
        .map_err(|error| format!("Could not write session file {}: {}", path.display(), error))
}

fn session_path() -> PathBuf {
    settings::config_dir().join("session.json")
}
