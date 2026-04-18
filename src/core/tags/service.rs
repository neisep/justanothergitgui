use std::path::Path;

use crate::core::ports::{GitHubPort, GitPort, GitRemoteAuth};
use crate::shared::github::GithubAuthSession;

pub fn create_tag(
    repo_path: &Path,
    tag_name: &str,
    auth: Option<&GithubAuthSession>,
    git: &impl GitPort,
    github: &impl GitHubPort,
) -> Result<String, String> {
    let tag_name = tag_name.trim();
    if tag_name.is_empty() {
        return Err("Tag name cannot be empty.".into());
    }

    let branch_name = git
        .current_branch_name(repo_path)?
        .ok_or_else(|| "Tag creation requires a checked-out branch.".to_string())?;
    if !git.can_create_tag_on_branch(&branch_name) {
        return Err("Tags can only be created from the main or master branch.".into());
    }

    git.create_tag(repo_path, tag_name)?;

    if !git.has_origin_remote(repo_path)? {
        return Ok(format!("Created local tag {}", tag_name));
    }

    match push_tag(repo_path, tag_name, auth, git, github) {
        Ok(()) => Ok(format!("Created and pushed tag {}", tag_name)),
        Err(error) => match git.rollback_tag(repo_path, tag_name) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(format!(
                "{} Local tag rollback also failed: {}",
                error, rollback_error
            )),
        },
    }
}

fn push_tag(
    repo_path: &Path,
    tag_name: &str,
    auth: Option<&GithubAuthSession>,
    git: &impl GitPort,
    github: &impl GitHubPort,
) -> Result<(), String> {
    if github.is_github_https_origin(repo_path) {
        let auth = auth.ok_or_else(|| {
            "GitHub tag creation requires the app's GitHub sign-in. Use 'Sign in to GitHub...' and try again."
                .to_string()
        })?;
        return git.push_tag(repo_path, tag_name, GitRemoteAuth::GitHub(auth));
    }

    git.push_tag(repo_path, tag_name, GitRemoteAuth::System)
}
