use std::path::Path;

use git2::Repository;

use crate::infra::git::{remotes as git_remotes, repository as git_repository};
use crate::infra::github::pulls as github_pulls;
use crate::shared::github::GithubAuthSession;

pub fn create_tag(
    repo_path: &Path,
    tag_name: &str,
    auth: Option<&GithubAuthSession>,
) -> Result<String, String> {
    let tag_name = tag_name.trim();
    if tag_name.is_empty() {
        return Err("Tag name cannot be empty.".into());
    }

    let branch_name = git_repository::current_branch_name(repo_path)?
        .ok_or_else(|| "Tag creation requires a checked-out branch.".to_string())?;
    if !git_repository::can_create_tag_on_branch(&branch_name) {
        return Err("Tags can only be created from the main or master branch.".into());
    }

    let refname = format!("refs/tags/{}", tag_name);
    if !git2::Reference::is_valid_name(&refname) {
        return Err("Invalid tag name.".into());
    }

    let repo =
        Repository::open(repo_path).map_err(|error| format!("Open repo error: {}", error))?;
    if repo.find_reference(&refname).is_ok() {
        return Err("Tag already exists.".into());
    }

    let target = repo
        .head()
        .and_then(|head| head.peel(git2::ObjectType::Commit))
        .map_err(|_| "Cannot create a tag without a current commit.".to_string())?;
    repo.tag_lightweight(tag_name, &target, false)
        .map_err(|error| format!("Create tag error: {}", error))?;

    if !git_repository::has_origin_remote(&repo) {
        return Ok(format!("Created local tag {}", tag_name));
    }

    match push_tag(repo_path, tag_name, auth) {
        Ok(()) => Ok(format!("Created and pushed tag {}", tag_name)),
        Err(error) => match git_remotes::rollback_tag(&repo, tag_name) {
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
) -> Result<(), String> {
    if github_pulls::is_github_https_origin(repo_path) {
        let auth = auth.ok_or_else(|| {
            "GitHub tag creation requires the app's GitHub sign-in. Use 'Sign in to GitHub...' and try again."
                .to_string()
        })?;
        return git_remotes::push_tag_with_git2(
            repo_path,
            tag_name,
            git_remotes::RemoteAuth::GitHub(auth),
        );
    }

    git_remotes::push_tag_with_git2(repo_path, tag_name, git_remotes::RemoteAuth::System)
}
