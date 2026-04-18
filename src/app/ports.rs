use std::path::{Path, PathBuf};

use git2::Repository;

use crate::core::{publish, sync, tags};
use crate::infra::core_ports::{InfraGitHubPort, InfraGitPort};
use crate::infra::git::{clone, repository, worktree};
use crate::infra::github::{auth, pulls, repos};
use crate::infra::system::browser;
use crate::shared::conflicts::ConflictData;
use crate::shared::git::{
    CommitEntry, CreateBranchPreview, DiscardPreview, FileEntry, StaleBranch,
};
use crate::shared::github::{
    CreateGithubRepoRequest, CreateGithubRepoSuccess, GithubAuthCheck, GithubAuthPrompt,
    GithubAuthSession, GithubRepoSummary, PushSuccess,
};

pub(super) struct AppRepoRead;

impl AppRepoRead {
    pub(super) fn open(path: &Path) -> Result<Repository, git2::Error> {
        repository::open_repo(path)
    }

    pub(super) fn has_origin_remote(repo: &Repository) -> bool {
        repository::has_origin_remote(repo)
    }

    pub(super) fn has_github_origin(repo: &Repository) -> bool {
        pulls::has_github_origin(repo)
    }

    pub(super) fn has_github_https_origin(repo: &Repository) -> bool {
        pulls::has_github_https_origin(repo)
    }

    pub(super) fn outgoing_commit_count(repo: &Repository) -> Result<usize, git2::Error> {
        repository::get_outgoing_commit_count(repo)
    }

    pub(super) fn file_statuses(
        repo: &Repository,
    ) -> Result<(Vec<FileEntry>, Vec<FileEntry>), git2::Error> {
        worktree::get_file_statuses(repo)
    }

    pub(super) fn current_branch(repo: &Repository) -> Result<String, git2::Error> {
        repository::get_current_branch(repo)
    }

    pub(super) fn branches(repo: &Repository) -> Result<Vec<String>, git2::Error> {
        repository::get_branches(repo)
    }

    pub(super) fn commit_history(
        repo: &Repository,
        max_count: usize,
    ) -> Result<Vec<CommitEntry>, git2::Error> {
        repository::get_commit_history(repo, max_count)
    }

    pub(super) fn read_conflict_file(
        repo: &Repository,
        path: &str,
    ) -> Result<ConflictData, String> {
        worktree::read_conflict_file(repo, path)
    }

    pub(super) fn file_diff(
        repo: &Repository,
        path: &str,
        staged: bool,
    ) -> Result<String, git2::Error> {
        worktree::get_file_diff(repo, path, staged)
    }

    pub(super) fn repo_name_from_clone_url(url: &str) -> Option<String> {
        repos::repo_name_from_clone_url(url)
    }

    pub(super) fn validate_new_branch_name(repo: &Repository, name: &str) -> Option<String> {
        repository::validate_new_branch_name(repo, name)
    }

    pub(super) fn can_create_tag_on_branch(branch_name: &str) -> bool {
        repository::can_create_tag_on_branch(branch_name)
    }

    pub(super) fn suggest_next_tag(repo: &Repository) -> String {
        repository::suggest_next_tag(repo)
    }
}

pub(super) struct AppRepoWrite;

impl AppRepoWrite {
    pub(super) fn stage_file(repo: &Repository, path: &str) -> Result<(), git2::Error> {
        worktree::stage_file(repo, path)
    }

    pub(super) fn unstage_file(repo: &Repository, path: &str) -> Result<(), git2::Error> {
        worktree::unstage_file(repo, path)
    }

    pub(super) fn stage_all(repo: &Repository) -> Result<(), git2::Error> {
        worktree::stage_all(repo)
    }

    pub(super) fn unstage_all(repo: &Repository) -> Result<(), git2::Error> {
        worktree::unstage_all(repo)
    }

    pub(super) fn create_commit(
        repo: &Repository,
        message: &str,
    ) -> Result<git2::Oid, git2::Error> {
        worktree::create_commit(repo, message)
    }

    pub(super) fn switch_branch(repo: &Repository, branch_name: &str) -> Result<(), git2::Error> {
        repository::switch_branch(repo, branch_name)
    }

    pub(super) fn create_branch(repo: &Repository, branch_name: &str) -> Result<(), git2::Error> {
        repository::create_branch(repo, branch_name)
    }

    pub(super) fn preview_create_branch(
        repo: &Repository,
        branch_name: &str,
    ) -> CreateBranchPreview {
        repository::preview_create_branch(repo, branch_name)
    }

    pub(super) fn list_stale_branches(repo: &Repository) -> Result<Vec<StaleBranch>, git2::Error> {
        repository::list_stale_branches(repo)
    }

    pub(super) fn delete_local_branch(repo: &Repository, name: &str) -> Result<(), git2::Error> {
        repository::delete_local_branch(repo, name)
    }

    pub(super) fn preview_discard_damage(repo: &Repository) -> DiscardPreview {
        repository::preview_discard_damage(repo)
    }

    pub(super) fn write_resolved_file(
        repo: &Repository,
        data: &ConflictData,
    ) -> Result<(), String> {
        worktree::write_resolved_file(repo, data)
    }
}

pub(crate) struct AppWelcomeWorkerOps;

impl AppWelcomeWorkerOps {
    pub(crate) fn github_auth_login<F>(
        client_id: &str,
        on_prompt: F,
    ) -> Result<GithubAuthSession, String>
    where
        F: FnOnce(GithubAuthPrompt),
    {
        auth::github_auth_login(client_id, on_prompt)
    }

    pub(crate) fn create_github_repo(
        request: &CreateGithubRepoRequest,
    ) -> Result<CreateGithubRepoSuccess, String> {
        let git = InfraGitPort;
        let github = InfraGitHubPort;
        publish::service::create_github_repo(request, &git, &github)
    }

    pub(crate) fn list_github_repositories(
        auth: &GithubAuthSession,
    ) -> Result<Vec<GithubRepoSummary>, String> {
        repos::list_github_repositories(auth)
    }

    pub(crate) fn clone_repository(
        url: &str,
        dest: &Path,
        auth: Option<&GithubAuthSession>,
    ) -> Result<PathBuf, String> {
        clone::clone_repository(url, dest, auth)
    }
}

pub(crate) struct AppRepoWorkerOps;

impl AppRepoWorkerOps {
    pub(crate) fn push(
        repo_path: &Path,
        auth: Option<&GithubAuthSession>,
    ) -> Result<PushSuccess, String> {
        let git = InfraGitPort;
        let github = InfraGitHubPort;
        sync::service::push(repo_path, auth, &git, &github)
    }

    pub(crate) fn pull(
        repo_path: &Path,
        auth: Option<&GithubAuthSession>,
    ) -> Result<String, String> {
        let git = InfraGitPort;
        let github = InfraGitHubPort;
        sync::service::pull(repo_path, auth, &git, &github)
    }

    pub(crate) fn create_tag(
        repo_path: &Path,
        tag_name: &str,
        auth: Option<&GithubAuthSession>,
    ) -> Result<String, String> {
        let git = InfraGitPort;
        let github = InfraGitHubPort;
        tags::service::create_tag(repo_path, tag_name, auth, &git, &github)
    }

    pub(crate) fn open_pull_request(url: &str) -> Result<String, String> {
        browser::open_url(url, "pull request")?;
        Ok("Opened pull request in browser".into())
    }

    pub(crate) fn create_pull_request(url: &str) -> Result<String, String> {
        browser::open_url(url, "pull request creation page")?;
        Ok("Opened pull request creation in browser".into())
    }

    pub(crate) fn discard_and_reset_to_remote(
        repo_path: &Path,
        auth: Option<&GithubAuthSession>,
        clean_untracked: bool,
    ) -> Result<String, String> {
        let git = InfraGitPort;
        let github = InfraGitHubPort;
        sync::service::discard_and_reset_to_remote(repo_path, auth, clean_untracked, &git, &github)
    }

    pub(crate) fn undo_last_commit(repo_path: &Path) -> Result<String, String> {
        let git = InfraGitPort;
        sync::service::undo_last_commit(repo_path, &git)
    }
}

pub(super) struct AppGitHubAuth;

impl AppGitHubAuth {
    pub(super) fn load_session() -> Result<Option<GithubAuthSession>, String> {
        auth::load_github_auth_session()
    }

    pub(super) fn save_session(session: &GithubAuthSession) -> Result<(), String> {
        auth::save_github_auth_session(session)
    }

    pub(super) fn clear_session() -> Result<(), String> {
        auth::clear_github_auth_session()
    }

    pub(super) fn verify_session(session: &GithubAuthSession) -> Result<GithubAuthCheck, String> {
        auth::verify_github_auth_session(session)
    }
}
