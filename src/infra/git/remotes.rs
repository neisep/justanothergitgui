use git2::Repository;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::shared::github::GithubAuthSession;

pub enum RemoteAuth<'a> {
    GitHub(&'a GithubAuthSession),
    System,
}

pub fn push_with_git2(
    repo_path: &Path,
    branch_name: &str,
    auth: RemoteAuth<'_>,
) -> Result<String, String> {
    let repo =
        Repository::open(repo_path).map_err(|error| format!("Open repo error: {}", error))?;
    let mut remote = repo
        .find_remote("origin")
        .map_err(|error| format!("Remote error: {}", error))?;
    let mut push_options = git2::PushOptions::new();
    let push_rejected: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let push_rejected_cb = Arc::clone(&push_rejected);
    let mut callbacks = remote_callbacks(&repo, auth)?;
    callbacks.push_update_reference(move |_refname, status| {
        if let Some(message) = status {
            *push_rejected_cb.lock().unwrap_or_else(|e| e.into_inner()) = Some(message.to_string());
        }
        Ok(())
    });
    push_options.remote_callbacks(callbacks);
    let refspec = format!("refs/heads/{0}:refs/heads/{0}", branch_name);
    remote
        .push(&[&refspec], Some(&mut push_options))
        .map_err(|error| format!("Push error: {}", error))?;

    if let Some(reason) = push_rejected
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
    {
        return Err(format!("Push rejected by remote: {}", reason));
    }

    sync_remote_tracking_branch(&repo, branch_name)?;

    if let Err(error) = repair_branch_upstream(&repo, branch_name) {
        return Ok(format!(
            "Push successful (warning: upstream configuration error: {})",
            error
        ));
    }

    Ok("Push successful".into())
}

pub fn push_tag_with_git2(
    repo_path: &Path,
    tag_name: &str,
    auth: RemoteAuth<'_>,
) -> Result<(), String> {
    let repo =
        Repository::open(repo_path).map_err(|error| format!("Open repo error: {}", error))?;
    let mut remote = repo
        .find_remote("origin")
        .map_err(|error| format!("Remote error: {}", error))?;
    let mut push_options = git2::PushOptions::new();
    let push_rejected: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let push_rejected_cb = Arc::clone(&push_rejected);
    let mut callbacks = remote_callbacks(&repo, auth)?;
    callbacks.push_update_reference(move |_refname, status| {
        if let Some(message) = status {
            *push_rejected_cb.lock().unwrap_or_else(|e| e.into_inner()) = Some(message.to_string());
        }
        Ok(())
    });
    push_options.remote_callbacks(callbacks);
    let refspec = format!("refs/tags/{0}:refs/tags/{0}", tag_name);
    remote
        .push(&[&refspec], Some(&mut push_options))
        .map_err(|error| format!("Tag push error: {}", error))?;

    if let Some(reason) = push_rejected
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
    {
        return Err(format!("Tag push rejected by remote: {}", reason));
    }

    Ok(())
}

pub fn pull_with_git2(
    repo_path: &Path,
    branch_name: &str,
    auth: RemoteAuth<'_>,
) -> Result<String, String> {
    let repo =
        Repository::open(repo_path).map_err(|error| format!("Open repo error: {}", error))?;
    let mut remote = repo
        .find_remote("origin")
        .map_err(|error| format!("Remote error: {}", error))?;

    let mut fetch_options = git2::FetchOptions::new();
    fetch_options.remote_callbacks(remote_callbacks(&repo, auth)?);
    fetch_options.prune(git2::FetchPrune::On);
    let refspecs: [&str; 0] = [];
    remote
        .fetch(&refspecs, Some(&mut fetch_options), None)
        .map_err(|error| format!("Pull fetch error: {}", error))?;

    let fetch_ref_name = format!("refs/remotes/origin/{}", branch_name);
    let fetch_ref = repo
        .find_reference(&fetch_ref_name)
        .map_err(|error| format!("Pull reference error: {}", error))?;
    let fetch_commit = repo
        .reference_to_annotated_commit(&fetch_ref)
        .map_err(|error| format!("Pull analysis error: {}", error))?;

    let (analysis, _) = repo
        .merge_analysis(&[&fetch_commit])
        .map_err(|error| format!("Pull analysis error: {}", error))?;

    if analysis.is_up_to_date() {
        return Ok("Already up to date".into());
    }

    if analysis.is_fast_forward() {
        fast_forward_branch(&repo, branch_name, &fetch_commit)?;
        return Ok("Pull successful".into());
    }

    if analysis.is_normal() {
        return merge_fetched_branch(&repo, branch_name, &fetch_commit);
    }

    Err("Pull requires manual reconciliation.".into())
}

pub fn reset_to_remote(
    repo_path: &Path,
    auth: RemoteAuth<'_>,
    clean_untracked: bool,
) -> Result<String, String> {
    let branch_name = crate::infra::git::repository::current_branch_name(repo_path)?
        .ok_or_else(|| "Reset requires a checked-out branch.".to_string())?;

    let repo =
        Repository::open(repo_path).map_err(|error| format!("Open repo error: {}", error))?;
    let mut remote = repo
        .find_remote("origin")
        .map_err(|error| format!("Remote error: {}", error))?;

    let mut fetch_options = git2::FetchOptions::new();
    fetch_options.remote_callbacks(remote_callbacks(&repo, auth)?);
    fetch_options.prune(git2::FetchPrune::On);
    remote
        .fetch(&[&branch_name], Some(&mut fetch_options), None)
        .map_err(|error| format!("Fetch error: {}", error))?;

    let fetch_ref_name = format!("refs/remotes/origin/{}", branch_name);
    let fetch_ref = repo
        .find_reference(&fetch_ref_name)
        .map_err(|error| format!("Remote branch {} not found: {}", branch_name, error))?;
    let target_oid = fetch_ref
        .target()
        .ok_or_else(|| format!("Remote branch {} has no target commit", branch_name))?;
    let target_commit = repo
        .find_commit(target_oid)
        .map_err(|error| format!("Find commit error: {}", error))?;

    repo.reset(target_commit.as_object(), git2::ResetType::Hard, None)
        .map_err(|error| format!("Reset error: {}", error))?;

    let mut cleaned = super::worktree::CleanUntrackedResult::default();
    if clean_untracked {
        cleaned = super::worktree::clean_untracked_files(&repo)
            .map_err(|error| format!("Clean untracked error: {}", error))?;
    }

    let mut message = format!("Reset to origin/{}", branch_name);
    if cleaned.removed_count > 0 {
        message.push_str(&format!(
            ", removed {} untracked entry(ies)",
            cleaned.removed_count
        ));
    }
    if !cleaned.failures.is_empty() {
        let failed_paths = cleaned
            .failures
            .iter()
            .map(|failure| format!("{} ({})", failure.path, failure.error))
            .collect::<Vec<_>>()
            .join(", ");
        message.push_str(&format!(
            ", failed to remove {} untracked entry(ies): {}",
            cleaned.failures.len(),
            failed_paths
        ));
    }
    Ok(message)
}

pub fn rollback_tag(repo: &Repository, tag_name: &str) -> Result<(), String> {
    let refname = format!("refs/tags/{}", tag_name);
    let mut reference = repo
        .find_reference(&refname)
        .map_err(|error| format!("Tag rollback error: {}", error))?;
    reference
        .delete()
        .map_err(|error| format!("Tag rollback error: {}", error))
}

pub(crate) fn upstream_target_oid(
    repo: &Repository,
    branch_name: &str,
) -> Result<Option<git2::Oid>, git2::Error> {
    let branch = repo.find_branch(branch_name, git2::BranchType::Local)?;
    let Ok(upstream) = branch.upstream() else {
        return Ok(None);
    };

    let local_ref_name = format!("refs/heads/{}", branch_name);
    if upstream.get().name() == Some(local_ref_name.as_str()) {
        return Ok(None);
    }

    Ok(upstream.get().target())
}

pub(crate) fn repair_branch_upstream(repo: &Repository, branch_name: &str) -> Result<(), String> {
    let remote_ref_name = format!("refs/remotes/origin/{}", branch_name);
    if repo.find_reference(&remote_ref_name).is_err() {
        return Ok(());
    }

    let mut branch = repo
        .find_branch(branch_name, git2::BranchType::Local)
        .map_err(|error| error.to_string())?;
    let needs_repair = match branch.upstream() {
        Ok(upstream) => {
            upstream.get().name() == Some(format!("refs/heads/{}", branch_name).as_str())
        }
        Err(_) => true,
    };

    if needs_repair {
        branch
            .set_upstream(Some(&format!("origin/{}", branch_name)))
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

pub(crate) fn github_remote_callbacks(auth: &GithubAuthSession) -> git2::RemoteCallbacks<'static> {
    let token = auth.access_token.clone();
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(move |_url, username_from_url, _allowed_types| {
        git2::Cred::userpass_plaintext(username_from_url.unwrap_or("x-access-token"), &token)
    });
    callbacks
}

pub(crate) fn standard_remote_callbacks_from_config(
    config: git2::Config,
) -> git2::RemoteCallbacks<'static> {
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(move |url, username_from_url, allowed_types| {
        if allowed_types.contains(git2::CredentialType::SSH_KEY)
            && let Some(username) = username_from_url
            && let Ok(cred) = git2::Cred::ssh_key_from_agent(username)
        {
            return Ok(cred);
        }

        if (allowed_types.contains(git2::CredentialType::USER_PASS_PLAINTEXT)
            || allowed_types.contains(git2::CredentialType::USERNAME))
            && let Ok(cred) = git2::Cred::credential_helper(&config, url, username_from_url)
        {
            return Ok(cred);
        }

        if allowed_types.contains(git2::CredentialType::USERNAME)
            && let Some(username) = username_from_url
        {
            return git2::Cred::username(username);
        }

        git2::Cred::default()
    });
    callbacks
}

fn sync_remote_tracking_branch(repo: &Repository, branch_name: &str) -> Result<(), String> {
    let branch_ref_name = format!("refs/heads/{}", branch_name);
    let local_oid = repo
        .refname_to_id(&branch_ref_name)
        .map_err(|error| format!("Push tracking update error: {}", error))?;
    let remote_ref_name = format!("refs/remotes/origin/{}", branch_name);
    repo.reference(
        &remote_ref_name,
        local_oid,
        true,
        "Update remote-tracking ref after push",
    )
    .map_err(|error| format!("Push tracking update error: {}", error))?;
    Ok(())
}

fn remote_callbacks(
    repo: &Repository,
    auth: RemoteAuth<'_>,
) -> Result<git2::RemoteCallbacks<'static>, String> {
    match auth {
        RemoteAuth::GitHub(auth) => Ok(github_remote_callbacks(auth)),
        RemoteAuth::System => standard_remote_callbacks(repo),
    }
}

fn standard_remote_callbacks(repo: &Repository) -> Result<git2::RemoteCallbacks<'static>, String> {
    let config = repo
        .config()
        .map_err(|error| format!("Credential configuration error: {}", error))?;
    Ok(standard_remote_callbacks_from_config(config))
}

fn fast_forward_branch(
    repo: &Repository,
    branch_name: &str,
    fetch_commit: &git2::AnnotatedCommit<'_>,
) -> Result<(), String> {
    let refname = format!("refs/heads/{}", branch_name);

    let target_obj = repo
        .find_object(fetch_commit.id(), None)
        .map_err(|error| format!("Pull fast-forward error: {}", error))?;
    repo.checkout_tree(
        &target_obj,
        Some(git2::build::CheckoutBuilder::new().safe()),
    )
    .map_err(|error| format!("Pull checkout error: {}", error))?;

    let mut branch_ref = repo
        .find_reference(&refname)
        .map_err(|error| format!("Pull fast-forward error: {}", error))?;
    branch_ref
        .set_target(fetch_commit.id(), &format!("Fast-forward {}", refname))
        .map_err(|error| format!("Pull fast-forward error: {}", error))?;
    repo.set_head(&refname)
        .map_err(|error| format!("Pull head update error: {}", error))?;
    Ok(())
}

fn merge_fetched_branch(
    repo: &Repository,
    branch_name: &str,
    fetch_commit: &git2::AnnotatedCommit<'_>,
) -> Result<String, String> {
    let head_commit = repo
        .head()
        .and_then(|head| head.peel_to_commit())
        .map_err(|error| format!("Pull head error: {}", error))?;
    let remote_commit = repo
        .find_commit(fetch_commit.id())
        .map_err(|error| format!("Pull merge error: {}", error))?;

    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.safe();
    repo.merge(&[fetch_commit], None, Some(&mut checkout))
        .map_err(|error| format!("Pull merge error: {}", error))?;

    let mut index = repo
        .index()
        .map_err(|error| format!("Pull index error: {}", error))?;
    if index.has_conflicts() {
        return Ok(format!(
            "Pull completed with conflicts on {}. Resolve them and commit the merge.",
            branch_name
        ));
    }

    let tree_oid = index
        .write_tree_to(repo)
        .map_err(|error| format!("Pull tree error: {}", error))?;
    let tree = repo
        .find_tree(tree_oid)
        .map_err(|error| format!("Pull tree error: {}", error))?;
    let signature = repo
        .signature()
        .map_err(|error| format!("Pull signature error: {}", error))?;
    let remote_label = repo
        .find_remote("origin")
        .ok()
        .and_then(|remote| remote.url().map(ToOwned::to_owned))
        .unwrap_or_else(|| "origin".into());
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        &format!("Merge branch '{}' of {}", branch_name, remote_label),
        &tree,
        &[&head_commit, &remote_commit],
    )
    .map_err(|error| format!("Pull commit error: {}", error))?;
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().safe()))
        .map_err(|error| format!("Pull checkout error: {}", error))?;
    repo.cleanup_state()
        .map_err(|error| format!("Pull cleanup error: {}", error))?;

    Ok("Pull successful".into())
}
