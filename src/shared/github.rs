use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub enum PullRequestPrompt {
    Open {
        branch: String,
        number: u64,
        url: String,
    },
    Create {
        branch: String,
        url: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GithubRepoVisibility {
    Public,
    Private,
}

impl GithubRepoVisibility {
    pub fn is_private(self) -> bool {
        matches!(self, Self::Private)
    }
}

#[derive(Clone, Debug)]
pub struct CreateGithubRepoRequest {
    pub folder_path: PathBuf,
    pub repo_name: String,
    pub commit_message: String,
    pub visibility: GithubRepoVisibility,
    pub auth: GithubAuthSession,
}

#[derive(Clone, Debug)]
pub struct CreateGithubRepoSuccess {
    pub folder_path: PathBuf,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct PushSuccess {
    pub message: String,
    pub pull_request_prompt: Option<PullRequestPrompt>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GithubAuthSession {
    pub access_token: String,
    pub login: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GithubRepoSummary {
    pub full_name: String,
    pub clone_url: String,
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Clone, Debug)]
pub struct GithubAuthPrompt {
    pub user_code: String,
    pub verification_uri: String,
    pub browser_url: String,
}

pub enum GithubAuthCheck {
    Valid,
    Revoked,
}
