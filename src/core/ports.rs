use std::path::{Path, PathBuf};

use crate::shared::github::{GithubAuthSession, GithubRepoVisibility, PullRequestPrompt};
use crate::shared::worktrees::{LinkedWorktree, NewWorktreeRequest};

pub enum GitRemoteAuth<'a> {
    GitHub(&'a GithubAuthSession),
    System,
}

pub trait GitBranchReadPort {
    fn current_branch_name(&self, repo_path: &Path) -> Result<Option<String>, String>;
}

pub trait GitRemoteSyncPort {
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
}

pub trait GitRemoteInfoPort {
    fn has_origin_remote(&self, repo_path: &Path) -> Result<bool, String>;
}

pub trait GitTagPort {
    fn can_create_tag_on_branch(&self, branch_name: &str) -> bool;
    fn create_tag(&self, repo_path: &Path, tag_name: &str) -> Result<(), String>;
    fn push_tag(
        &self,
        repo_path: &Path,
        tag_name: &str,
        auth: GitRemoteAuth<'_>,
    ) -> Result<(), String>;
    fn rollback_tag(&self, repo_path: &Path, tag_name: &str) -> Result<(), String>;
}

pub trait GitRepoBootstrapPort {
    fn open_or_init_repo(&self, repo_path: &Path) -> Result<(), String>;
    fn add_remote(&self, repo_path: &Path, name: &str, url: &str) -> Result<(), String>;
}

pub trait GitWorktreeCommitPort {
    fn repo_has_changes(&self, repo_path: &Path) -> Result<bool, String>;
    fn head_exists(&self, repo_path: &Path) -> Result<bool, String>;
    fn stage_all(&self, repo_path: &Path) -> Result<(), String>;
    fn create_commit(&self, repo_path: &Path, message: &str) -> Result<(), String>;
}

pub trait GitUndoCommitPort {
    fn outgoing_commit_count(&self, repo_path: &Path) -> Result<usize, String>;
    fn undo_last_commit(&self, repo_path: &Path) -> Result<String, String>;
}

/// Git's *linked* worktrees — the `git worktree` set — as opposed to
/// [`GitWorktreeCommitPort`], which is about the index and working tree of a
/// single checkout.
pub trait GitLinkedWorktreePort {
    fn list_worktrees(&self, repo_path: &Path) -> Result<Vec<LinkedWorktree>, String>;
    fn add_worktree(
        &self,
        repo_path: &Path,
        request: &NewWorktreeRequest,
    ) -> Result<PathBuf, String>;
    /// `force` decides only whether uncommitted changes may be destroyed. The
    /// worktree service never passes `true`; the parameter is the seam an
    /// explicit discard-and-remove flow would use.
    fn remove_worktree(&self, repo_path: &Path, name: &str, force: bool) -> Result<(), String>;
}

#[allow(dead_code)]
pub trait GitPort:
    GitBranchReadPort
    + GitRemoteSyncPort
    + GitRemoteInfoPort
    + GitTagPort
    + GitRepoBootstrapPort
    + GitWorktreeCommitPort
    + GitUndoCommitPort
{
}

impl<T> GitPort for T where
    T: ?Sized
        + GitBranchReadPort
        + GitRemoteSyncPort
        + GitRemoteInfoPort
        + GitTagPort
        + GitRepoBootstrapPort
        + GitWorktreeCommitPort
        + GitUndoCommitPort
{
}

pub trait GitHubRemoteInfoPort {
    fn is_github_https_origin(&self, repo_path: &Path) -> bool;
    fn detect_pull_request_prompt(
        &self,
        repo_path: &Path,
        branch: &str,
        auth: Option<&GithubAuthSession>,
    ) -> Result<Option<PullRequestPrompt>, String>;
}

pub trait GitHubRepoCreationPort {
    fn create_repository(
        &self,
        auth: &GithubAuthSession,
        repo_name: &str,
        visibility: GithubRepoVisibility,
    ) -> Result<String, String>;
}

#[allow(dead_code)]
pub trait GitHubPort: GitHubRemoteInfoPort + GitHubRepoCreationPort {}

impl<T> GitHubPort for T where T: ?Sized + GitHubRemoteInfoPort + GitHubRepoCreationPort {}
