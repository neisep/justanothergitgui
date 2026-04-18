use std::path::{Path, PathBuf};

use crate::infra::git::remotes::{github_remote_callbacks, standard_remote_callbacks_from_config};
use crate::infra::github::pulls::is_github_https_url;
use crate::shared::github::GithubAuthSession;

pub fn clone_repository(
    url: &str,
    dest: &Path,
    auth: Option<&GithubAuthSession>,
) -> Result<PathBuf, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("Clone URL is required.".into());
    }

    let dest_existed = dest.exists();
    if dest_existed {
        let is_empty = std::fs::read_dir(dest)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false);
        if !is_empty {
            return Err(format!(
                "Destination already exists and is not empty: {}",
                dest.display()
            ));
        }
    } else if let Some(parent) = dest.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        return Err(format!(
            "Parent folder does not exist: {}",
            parent.display()
        ));
    }

    let use_github_auth = auth.is_some() && is_github_https_url(trimmed);
    let callbacks = if use_github_auth {
        github_remote_callbacks(auth.expect("auth is Some"))
    } else {
        let config = git2::Config::open_default()
            .map_err(|error| format!("Credential configuration error: {}", error))?;
        standard_remote_callbacks_from_config(config)
    };

    let mut fetch_options = git2::FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);

    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fetch_options);

    let repo = match builder.clone(trimmed, dest) {
        Ok(repo) => repo,
        Err(error) => {
            cleanup_failed_clone(dest, dest_existed);
            return Err(format!("Clone error: {}", error));
        }
    };
    let workdir = repo
        .workdir()
        .ok_or_else(|| "Cloned repository has no working directory.".to_string())?
        .to_path_buf();

    Ok(workdir.canonicalize().unwrap_or(workdir))
}

fn cleanup_failed_clone(dest: &Path, dest_existed: bool) {
    if dest_existed {
        if let Ok(entries) = std::fs::read_dir(dest) {
            for entry in entries.flatten() {
                let path = entry.path();
                let removed = match entry.file_type() {
                    Ok(file_type) if file_type.is_dir() => std::fs::remove_dir_all(&path),
                    _ => std::fs::remove_file(&path),
                };
                let _ = removed;
            }
        }
    } else {
        let _ = std::fs::remove_dir_all(dest);
    }
}

pub use crate::infra::github::repos::repo_name_from_clone_url;
