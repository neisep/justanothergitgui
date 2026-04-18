use std::path::Path;

use crate::infra::git::{remotes as git_remotes, repository as git_repository};
use crate::infra::github::pulls as github_pulls;
use crate::shared::github::{GithubAuthSession, PushSuccess};

pub fn push(repo_path: &Path, auth: Option<&GithubAuthSession>) -> Result<PushSuccess, String> {
    let branch_name = git_repository::current_branch_name(repo_path)?;
    let base_message =
        if let Some(message) = try_push_with_auth(repo_path, branch_name.as_deref(), auth)? {
            message
        } else {
            let branch_name = branch_name
                .as_deref()
                .ok_or_else(|| "Push requires a checked-out branch.".to_string())?;
            git_remotes::push_with_git2(repo_path, branch_name, git_remotes::RemoteAuth::System)?
        };

    let mut message = base_message;
    let pull_request_prompt = match branch_name.as_deref() {
        Some(branch) => match github_pulls::detect_pull_request_prompt(repo_path, branch, auth) {
            Ok(prompt) => prompt,
            Err(error) => {
                message.push_str(&format!(" PR check unavailable: {}", error));
                None
            }
        },
        None => None,
    };

    Ok(PushSuccess {
        message,
        pull_request_prompt,
    })
}

pub fn pull(repo_path: &Path, auth: Option<&GithubAuthSession>) -> Result<String, String> {
    if github_pulls::is_github_https_origin(repo_path) {
        let branch_name = git_repository::current_branch_name(repo_path)?
            .ok_or_else(|| "GitHub pull requires a checked-out branch.".to_string())?;
        let auth = auth.ok_or_else(|| {
            "GitHub pull requires the app's GitHub sign-in. Use 'Sign in to GitHub...' and try again."
                .to_string()
        })?;

        return git_remotes::pull_with_git2(
            repo_path,
            &branch_name,
            git_remotes::RemoteAuth::GitHub(auth),
        );
    }

    let branch_name = git_repository::current_branch_name(repo_path)?
        .ok_or_else(|| "Pull requires a checked-out branch.".to_string())?;
    git_remotes::pull_with_git2(repo_path, &branch_name, git_remotes::RemoteAuth::System)
}

pub fn discard_and_reset_to_remote(
    repo_path: &Path,
    auth: Option<&GithubAuthSession>,
    clean_untracked: bool,
) -> Result<String, String> {
    let remote_auth = if github_pulls::is_github_https_origin(repo_path) {
        let auth = auth.ok_or_else(|| {
            "Reset requires the app's GitHub sign-in. Use 'Sign in to GitHub...' and try again."
                .to_string()
        })?;
        git_remotes::RemoteAuth::GitHub(auth)
    } else {
        git_remotes::RemoteAuth::System
    };

    git_remotes::reset_to_remote(repo_path, remote_auth, clean_untracked)
}

pub(crate) fn try_push_with_auth(
    repo_path: &Path,
    branch_name: Option<&str>,
    auth: Option<&GithubAuthSession>,
) -> Result<Option<String>, String> {
    if !github_pulls::is_github_https_origin(repo_path) {
        return Ok(None);
    }

    let Some(branch_name) = branch_name else {
        return Err("GitHub push requires a checked-out branch.".into());
    };
    let Some(auth) = auth else {
        return Err(
            "GitHub push requires the app's GitHub sign-in. Use 'Sign in to GitHub...' and try again."
                .into(),
        );
    };

    git_remotes::push_with_git2(
        repo_path,
        branch_name,
        git_remotes::RemoteAuth::GitHub(auth),
    )?;
    Ok(Some("Push successful".into()))
}
