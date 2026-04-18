use std::path::Path;

use git2::Repository;

use crate::core::ports::{GitHubPort, GitPort, GitRemoteAuth};
use crate::infra::git::{
    remotes as git_remotes, repository as git_repository, worktree as git_worktree,
};
use crate::infra::github::{pulls as github_pulls, repos as github_repos};
use crate::shared::github::{GithubAuthSession, GithubRepoVisibility, PullRequestPrompt};

#[derive(Clone, Copy, Debug, Default)]
pub struct InfraGitPort;

#[derive(Clone, Copy, Debug, Default)]
pub struct InfraGitHubPort;

impl GitPort for InfraGitPort {
    fn current_branch_name(&self, repo_path: &Path) -> Result<Option<String>, String> {
        git_repository::current_branch_name(repo_path)
    }

    fn push(
        &self,
        repo_path: &Path,
        branch_name: &str,
        auth: GitRemoteAuth<'_>,
    ) -> Result<String, String> {
        git_remotes::push_with_git2(repo_path, branch_name, map_remote_auth(auth))
    }

    fn pull(
        &self,
        repo_path: &Path,
        branch_name: &str,
        auth: GitRemoteAuth<'_>,
    ) -> Result<String, String> {
        git_remotes::pull_with_git2(repo_path, branch_name, map_remote_auth(auth))
    }

    fn reset_to_remote(
        &self,
        repo_path: &Path,
        auth: GitRemoteAuth<'_>,
        clean_untracked: bool,
    ) -> Result<String, String> {
        git_remotes::reset_to_remote(repo_path, map_remote_auth(auth), clean_untracked)
    }

    fn can_create_tag_on_branch(&self, branch_name: &str) -> bool {
        git_repository::can_create_tag_on_branch(branch_name)
    }

    fn create_tag(&self, repo_path: &Path, tag_name: &str) -> Result<(), String> {
        let repo =
            Repository::open(repo_path).map_err(|error| format!("Open repo error: {}", error))?;
        let refname = format!("refs/tags/{}", tag_name);
        if !git2::Reference::is_valid_name(&refname) {
            return Err("Invalid tag name.".into());
        }
        if repo.find_reference(&refname).is_ok() {
            return Err("Tag already exists.".into());
        }

        let target = repo
            .head()
            .and_then(|head| head.peel(git2::ObjectType::Commit))
            .map_err(|_| "Cannot create a tag without a current commit.".to_string())?;
        repo.tag_lightweight(tag_name, &target, false)
            .map_err(|error| format!("Create tag error: {}", error))?;
        Ok(())
    }

    fn push_tag(
        &self,
        repo_path: &Path,
        tag_name: &str,
        auth: GitRemoteAuth<'_>,
    ) -> Result<(), String> {
        git_remotes::push_tag_with_git2(repo_path, tag_name, map_remote_auth(auth))
    }

    fn has_origin_remote(&self, repo_path: &Path) -> Result<bool, String> {
        let repo =
            Repository::open(repo_path).map_err(|error| format!("Open repo error: {}", error))?;
        Ok(git_repository::has_origin_remote(&repo))
    }

    fn rollback_tag(&self, repo_path: &Path, tag_name: &str) -> Result<(), String> {
        let repo =
            Repository::open(repo_path).map_err(|error| format!("Open repo error: {}", error))?;
        git_remotes::rollback_tag(&repo, tag_name)
    }

    fn open_or_init_repo(&self, repo_path: &Path) -> Result<(), String> {
        git_repository::open_or_init_repo(repo_path).map(|_| ())
    }

    fn repo_has_changes(&self, repo_path: &Path) -> Result<bool, String> {
        let repo =
            Repository::open(repo_path).map_err(|error| format!("Open repo error: {}", error))?;
        git_repository::repo_has_changes(&repo)
    }

    fn head_exists(&self, repo_path: &Path) -> Result<bool, String> {
        let repo =
            Repository::open(repo_path).map_err(|error| format!("Open repo error: {}", error))?;
        Ok(repo.head().ok().and_then(|head| head.target()).is_some())
    }

    fn stage_all(&self, repo_path: &Path) -> Result<(), String> {
        let repo =
            Repository::open(repo_path).map_err(|error| format!("Open repo error: {}", error))?;
        git_worktree::stage_all(&repo).map_err(|error| format!("Stage all error: {}", error))
    }

    fn create_commit(&self, repo_path: &Path, message: &str) -> Result<(), String> {
        let repo =
            Repository::open(repo_path).map_err(|error| format!("Open repo error: {}", error))?;
        git_worktree::create_commit(&repo, message)
            .map(|_| ())
            .map_err(|error| format!("Commit error: {}", error))
    }

    fn add_remote(&self, repo_path: &Path, name: &str, url: &str) -> Result<(), String> {
        let repo =
            Repository::open(repo_path).map_err(|error| format!("Open repo error: {}", error))?;
        repo.remote(name, url)
            .map(|_| ())
            .map_err(|error| format!("Remote add error: {}", error))
    }
}

impl GitHubPort for InfraGitHubPort {
    fn is_github_https_origin(&self, repo_path: &Path) -> bool {
        github_pulls::is_github_https_origin(repo_path)
    }

    fn detect_pull_request_prompt(
        &self,
        repo_path: &Path,
        branch: &str,
        auth: Option<&GithubAuthSession>,
    ) -> Result<Option<PullRequestPrompt>, String> {
        github_pulls::detect_pull_request_prompt(repo_path, branch, auth)
    }

    fn create_repository(
        &self,
        auth: &GithubAuthSession,
        repo_name: &str,
        visibility: GithubRepoVisibility,
    ) -> Result<String, String> {
        github_repos::create_repository(auth, repo_name, visibility)
    }
}

fn map_remote_auth(auth: GitRemoteAuth<'_>) -> git_remotes::RemoteAuth<'_> {
    match auth {
        GitRemoteAuth::GitHub(session) => git_remotes::RemoteAuth::GitHub(session),
        GitRemoteAuth::System => git_remotes::RemoteAuth::System,
    }
}
