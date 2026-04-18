use crate::core::ports::{
    GitBranchReadPort, GitHubRemoteInfoPort, GitHubRepoCreationPort, GitRemoteInfoPort,
    GitRemoteSyncPort, GitRepoBootstrapPort, GitWorktreeCommitPort,
};
use crate::core::sync::service as sync_service;
use crate::shared::github::{CreateGithubRepoRequest, CreateGithubRepoSuccess};

pub fn create_github_repo<G, H>(
    request: &CreateGithubRepoRequest,
    git: &G,
    github: &H,
) -> Result<CreateGithubRepoSuccess, String>
where
    G: GitRepoBootstrapPort
        + GitRemoteInfoPort
        + GitWorktreeCommitPort
        + GitBranchReadPort
        + GitRemoteSyncPort,
    H: GitHubRepoCreationPort + GitHubRemoteInfoPort,
{
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

    git.open_or_init_repo(&folder_path)?;
    if git.has_origin_remote(&folder_path)? {
        return Err("Remote 'origin' already exists for this repository".into());
    }

    let has_changes = git.repo_has_changes(&folder_path)?;
    let has_head = git.head_exists(&folder_path)?;
    if has_changes || !has_head {
        git.stage_all(&folder_path)?;
        git.create_commit(&folder_path, commit_message)?;
    }

    let clone_url = github.create_repository(&request.auth, repo_name, request.visibility)?;
    git.add_remote(&folder_path, "origin", &clone_url)?;
    let push_result = sync_service::push(&folder_path, Some(&request.auth), git, github)?;
    let message = format!(
        "Created GitHub repository {}. {}",
        repo_name, push_result.message
    );

    Ok(CreateGithubRepoSuccess {
        folder_path,
        message,
    })
}
