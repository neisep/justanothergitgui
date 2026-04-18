use crate::core::sync::service as sync_service;
use crate::infra::git::{repository as git_repository, worktree as git_worktree};
use crate::infra::github::repos as github_repos;
use crate::shared::github::{CreateGithubRepoRequest, CreateGithubRepoSuccess};

pub fn create_github_repo(
    request: &CreateGithubRepoRequest,
) -> Result<CreateGithubRepoSuccess, String> {
    let repo_name = request.repo_name.trim();
    let commit_message = request.commit_message.trim();
    if repo_name.is_empty() {
        return Err("Repository name cannot be empty".into());
    }
    if commit_message.is_empty() {
        return Err("Initial commit message cannot be empty".into());
    }

    let folder_path = request
        .folder_path
        .canonicalize()
        .map_err(|error| format!("Invalid folder: {}", error))?;
    if !folder_path.is_dir() {
        return Err("Selected path is not a folder".into());
    }

    let repo = git_repository::open_or_init_repo(&folder_path)?;
    if repo.find_remote("origin").is_ok() {
        return Err("Remote 'origin' already exists for this repository".into());
    }

    let has_changes = git_repository::repo_has_changes(&repo)?;
    let has_head = repo.head().ok().and_then(|head| head.target()).is_some();
    if has_changes || !has_head {
        git_worktree::stage_all(&repo).map_err(|error| format!("Stage all error: {}", error))?;
        git_worktree::create_commit(&repo, commit_message)
            .map_err(|error| format!("Commit error: {}", error))?;
    }

    let clone_url = github_repos::create_repository(&request.auth, repo_name, request.visibility)?;
    repo.remote("origin", &clone_url)
        .map_err(|error| format!("Remote add error: {}", error))?;
    let push_result = sync_service::push(&folder_path, Some(&request.auth))?;
    let message = format!(
        "Created GitHub repository {}. {}",
        repo_name, push_result.message
    );

    Ok(CreateGithubRepoSuccess {
        folder_path,
        message,
    })
}
