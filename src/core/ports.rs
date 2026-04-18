use std::path::Path;

use crate::shared::github::{GithubAuthSession, GithubRepoVisibility, PullRequestPrompt};

pub enum GitRemoteAuth<'a> {
    GitHub(&'a GithubAuthSession),
    System,
}

pub trait GitPort {
    fn current_branch_name(&self, repo_path: &Path) -> Result<Option<String>, String>;
    fn push(
        &self,
        repo_path: &Path,
        branch_name: &str,
        auth: GitRemoteAuth<'_>,
    ) -> Result<String, String>;
    fn pull(
        &self,
        repo_path: &Path,
        branch_name: &str,
        auth: GitRemoteAuth<'_>,
    ) -> Result<String, String>;
    fn reset_to_remote(
        &self,
        repo_path: &Path,
        auth: GitRemoteAuth<'_>,
        clean_untracked: bool,
    ) -> Result<String, String>;
    fn can_create_tag_on_branch(&self, branch_name: &str) -> bool;
    fn create_tag(&self, repo_path: &Path, tag_name: &str) -> Result<(), String>;
    fn push_tag(
        &self,
        repo_path: &Path,
        tag_name: &str,
        auth: GitRemoteAuth<'_>,
    ) -> Result<(), String>;
    fn has_origin_remote(&self, repo_path: &Path) -> Result<bool, String>;
    fn rollback_tag(&self, repo_path: &Path, tag_name: &str) -> Result<(), String>;
    fn open_or_init_repo(&self, repo_path: &Path) -> Result<(), String>;
    fn repo_has_changes(&self, repo_path: &Path) -> Result<bool, String>;
    fn head_exists(&self, repo_path: &Path) -> Result<bool, String>;
    fn stage_all(&self, repo_path: &Path) -> Result<(), String>;
    fn create_commit(&self, repo_path: &Path, message: &str) -> Result<(), String>;
    fn add_remote(&self, repo_path: &Path, name: &str, url: &str) -> Result<(), String>;
}

pub trait GitHubPort {
    fn is_github_https_origin(&self, repo_path: &Path) -> bool;
    fn detect_pull_request_prompt(
        &self,
        repo_path: &Path,
        branch: &str,
        auth: Option<&GithubAuthSession>,
    ) -> Result<Option<PullRequestPrompt>, String>;
    fn create_repository(
        &self,
        auth: &GithubAuthSession,
        repo_name: &str,
        visibility: GithubRepoVisibility,
    ) -> Result<String, String>;
}
