use git2::{Repository, Status, StatusOptions};
use keyring::{Entry, Error as KeyringError};
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::state::{
    CommitEntry, ConflictChoice, ConflictData, ConflictPart, CreateBranchPreview, DiscardPreview,
    FileEntry, PullRequestPrompt, StaleBranch,
};

const GITHUB_AUTH_KEYRING_SERVICE: &str = "justanothergitgui";
const GITHUB_AUTH_KEYRING_USER: &str = "github-auth-session";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GithubRepoVisibility {
    Public,
    Private,
}

#[derive(Clone, Debug)]
pub struct CreateGithubRepoRequest {
    pub folder_path: PathBuf,
    pub repo_name: String,
    pub commit_message: String,
    pub visibility: GithubRepoVisibility,
    pub auth: GithubAuthSession,
}

#[derive(Clone, Debug)]
pub struct CreateGithubRepoSuccess {
    pub folder_path: PathBuf,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct PushSuccess {
    pub message: String,
    pub pull_request_prompt: Option<PullRequestPrompt>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GithubAuthSession {
    pub access_token: String,
    pub login: String,
}

#[derive(Clone, Debug)]
pub struct GithubAuthPrompt {
    pub user_code: String,
    pub verification_uri: String,
    pub browser_url: String,
}

#[derive(Deserialize)]
struct GithubUser {
    login: String,
}

#[derive(Deserialize)]
struct GithubRepo {
    clone_url: String,
    html_url: String,
    default_branch: String,
}

#[derive(Deserialize)]
struct GithubPullRequest {
    number: u64,
    html_url: String,
}

#[derive(Deserialize)]
struct GithubTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct GithubDeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    verification_uri_complete: Option<String>,
    expires_in: u64,
    interval: Option<u64>,
}

#[derive(Serialize)]
struct GithubCreateRepoBody<'a> {
    name: &'a str,
    private: bool,
}

pub fn open_repo(path: &Path) -> Result<Repository, git2::Error> {
    let repo = Repository::discover(path)?;
    if repo.is_bare() {
        return Err(git2::Error::from_str("Bare repositories are not supported"));
    }
    Ok(repo)
}

pub fn get_current_branch(repo: &Repository) -> Result<String, git2::Error> {
    let head = repo.head()?;
    Ok(head.shorthand().unwrap_or("HEAD").to_string())
}

pub fn get_branches(repo: &Repository) -> Result<Vec<String>, git2::Error> {
    let mut names = Vec::new();
    for branch in repo.branches(Some(git2::BranchType::Local))? {
        let (branch, _) = branch?;
        if let Some(name) = branch.name()? {
            names.push(name.to_string());
        }
    }
    Ok(names)
}

pub fn get_outgoing_commit_count(repo: &Repository) -> Result<usize, git2::Error> {
    let head = match repo.head() {
        Ok(head) if head.is_branch() => head,
        Ok(_) | Err(_) => return Ok(0),
    };

    let Some(local_oid) = head.target() else {
        return Ok(0);
    };

    let branch_name = head.shorthand().unwrap_or_default();
    if branch_name.is_empty() {
        return Ok(0);
    }

    let _ = repair_branch_upstream(repo, branch_name);

    if let Some(upstream_oid) = upstream_target_oid(repo, branch_name)? {
        let (ahead, _) = repo.graph_ahead_behind(local_oid, upstream_oid)?;
        return Ok(ahead);
    }

    let remote_ref = format!("refs/remotes/origin/{}", branch_name);
    if let Ok(reference) = repo.find_reference(&remote_ref)
        && let Some(remote_oid) = reference.target()
    {
        let (ahead, _) = repo.graph_ahead_behind(local_oid, remote_oid)?;
        return Ok(ahead);
    }

    let mut walk = repo.revwalk()?;
    walk.push(local_oid)?;
    if let Ok(references) = repo.references_glob("refs/remotes/*") {
        for reference in references {
            let reference = reference?;
            if let Some(remote_oid) = reference.target() {
                let _ = walk.hide(remote_oid);
            }
        }
    }

    Ok(walk.count())
}

pub fn can_create_tag_on_branch(branch_name: &str) -> bool {
    matches!(branch_name.trim(), "main" | "master")
}

pub fn has_origin_remote(repo: &Repository) -> bool {
    repo.find_remote("origin").is_ok()
}

pub fn has_github_origin(repo: &Repository) -> bool {
    repo.find_remote("origin")
        .ok()
        .and_then(|remote| remote.url().and_then(parse_github_remote_slug))
        .is_some()
}

pub fn has_github_https_origin(repo: &Repository) -> bool {
    repo.find_remote("origin")
        .ok()
        .is_some_and(|remote| remote.url().is_some_and(is_github_https_url))
}

pub fn load_github_auth_session() -> Result<Option<GithubAuthSession>, String> {
    let entry = github_auth_keyring_entry()?;
    match entry.get_password() {
        Ok(payload) => serde_json::from_str(&payload)
            .map(Some)
            .map_err(|e| format!("Saved GitHub sign-in is invalid: {}", e)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(e) => Err(format!("Could not load saved GitHub sign-in: {}", e)),
    }
}

pub fn save_github_auth_session(session: &GithubAuthSession) -> Result<(), String> {
    let entry = github_auth_keyring_entry()?;
    let payload = serde_json::to_string(session)
        .map_err(|e| format!("Could not serialize GitHub sign-in: {}", e))?;
    entry
        .set_password(&payload)
        .map_err(|e| format!("Could not save GitHub sign-in to system keychain: {}", e))
}

pub fn get_file_statuses(
    repo: &Repository,
) -> Result<(Vec<FileEntry>, Vec<FileEntry>), git2::Error> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);

    let statuses = repo.statuses(Some(&mut opts))?;
    let mut unstaged = Vec::new();
    let mut staged = Vec::new();

    for entry in statuses.iter() {
        let path = entry.path().unwrap_or("").to_string();
        let status = entry.status();

        // Conflicted files
        if status.contains(Status::CONFLICTED) {
            unstaged.push(FileEntry {
                path,
                display_status: "conflicted".to_string(),
                is_conflicted: true,
            });
            continue;
        }

        if status.intersects(
            Status::INDEX_NEW
                | Status::INDEX_MODIFIED
                | Status::INDEX_DELETED
                | Status::INDEX_RENAMED,
        ) {
            staged.push(FileEntry {
                path: path.clone(),
                display_status: status_label_staged(status).to_string(),
                is_conflicted: false,
            });
        }

        if status.intersects(Status::WT_NEW | Status::WT_MODIFIED | Status::WT_DELETED) {
            unstaged.push(FileEntry {
                path: path.clone(),
                display_status: status_label_unstaged(status).to_string(),
                is_conflicted: false,
            });
        }
    }

    Ok((unstaged, staged))
}

fn status_label_staged(s: Status) -> &'static str {
    if s.contains(Status::INDEX_NEW) {
        "new"
    } else if s.contains(Status::INDEX_MODIFIED) {
        "modified"
    } else if s.contains(Status::INDEX_DELETED) {
        "deleted"
    } else if s.contains(Status::INDEX_RENAMED) {
        "renamed"
    } else {
        "changed"
    }
}

fn status_label_unstaged(s: Status) -> &'static str {
    if s.contains(Status::WT_NEW) {
        "untracked"
    } else if s.contains(Status::WT_MODIFIED) {
        "modified"
    } else if s.contains(Status::WT_DELETED) {
        "deleted"
    } else {
        "changed"
    }
}

pub fn stage_file(repo: &Repository, path: &str) -> Result<(), git2::Error> {
    let mut index = repo.index()?;
    let full_path = repo_workdir(repo)?.join(path);

    if full_path.exists() {
        index.add_path(Path::new(path))?;
    } else {
        index.remove_path(Path::new(path))?;
    }

    index.write()?;
    Ok(())
}

pub fn unstage_file(repo: &Repository, path: &str) -> Result<(), git2::Error> {
    let mut index = repo.index()?;
    let p = Path::new(path);

    match repo.head() {
        Ok(head_ref) => {
            let commit = head_ref.peel_to_commit()?;
            let tree = commit.tree()?;
            match tree.get_path(p) {
                Ok(entry) => {
                    index.add(&git2::IndexEntry {
                        ctime: git2::IndexTime::new(0, 0),
                        mtime: git2::IndexTime::new(0, 0),
                        dev: 0,
                        ino: 0,
                        mode: entry.filemode() as u32,
                        uid: 0,
                        gid: 0,
                        file_size: 0,
                        id: entry.id(),
                        flags: 0,
                        flags_extended: 0,
                        path: path.as_bytes().to_vec(),
                    })?;
                }
                Err(_) => {
                    index.remove_path(p)?;
                }
            }
        }
        Err(_) => {
            index.remove_path(p)?;
        }
    }

    index.write()?;
    Ok(())
}

pub fn stage_all(repo: &Repository) -> Result<(), git2::Error> {
    let mut index = repo.index()?;
    index.add_all(["*"], git2::IndexAddOption::DEFAULT, None)?;
    index.update_all(["*"], None)?;
    index.write()?;
    Ok(())
}

pub fn unstage_all(repo: &Repository) -> Result<(), git2::Error> {
    let (_, staged) = get_file_statuses(repo)?;
    for file in staged {
        unstage_file(repo, &file.path)?;
    }
    Ok(())
}

pub fn create_commit(repo: &Repository, message: &str) -> Result<git2::Oid, git2::Error> {
    let mut index = repo.index()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;
    let sig = repo.signature()?;
    let mut parents = Vec::new();

    if let Ok(head) = repo.head() {
        parents.push(head.peel_to_commit()?);
    }

    if repo.state() == git2::RepositoryState::Merge
        && let Ok(merge_head) = repo.find_reference("MERGE_HEAD")
        && let Some(merge_oid) = merge_head.target()
    {
        parents.push(repo.find_commit(merge_oid)?);
    }

    let parent_refs: Vec<&git2::Commit<'_>> = parents.iter().collect();
    let oid = repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)?;

    if repo.state() == git2::RepositoryState::Merge {
        repo.cleanup_state()?;
    }

    Ok(oid)
}

pub fn get_file_diff(repo: &Repository, path: &str, staged: bool) -> Result<String, git2::Error> {
    let mut opts = git2::DiffOptions::new();
    opts.pathspec(path);

    let diff = if staged {
        let head_tree = repo.head().ok().and_then(|head| head.peel_to_tree().ok());
        repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))?
    } else {
        repo.diff_index_to_workdir(None, Some(&mut opts))?
    };

    let mut result = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        let origin = line.origin();
        if origin == '+' || origin == '-' || origin == ' ' {
            result.push(origin);
        }
        if let Ok(content) = std::str::from_utf8(line.content()) {
            result.push_str(content);
        }
        true
    })?;

    Ok(result)
}

pub fn push(repo_path: &Path, auth: Option<&GithubAuthSession>) -> Result<PushSuccess, String> {
    let branch_name = current_branch_name(repo_path)?;
    let base_message =
        if let Some(message) = try_push_with_auth(repo_path, branch_name.as_deref(), auth)? {
            message
        } else {
            let branch_name = branch_name
                .as_deref()
                .ok_or_else(|| "Push requires a checked-out branch.".to_string())?;
            push_with_git2(repo_path, branch_name, RemoteAuth::System)?
        };

    let mut message = base_message;
    let pull_request_prompt = match branch_name.as_deref() {
        Some(branch) => match detect_pull_request_prompt(repo_path, branch, auth) {
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
    if is_github_https_origin(repo_path) {
        let branch_name = current_branch_name(repo_path)?
            .ok_or_else(|| "GitHub pull requires a checked-out branch.".to_string())?;
        let auth = auth.ok_or_else(|| {
            "GitHub pull requires the app's GitHub sign-in. Use 'Sign in to GitHub...' and try again."
                .to_string()
        })?;

        return pull_with_git2(repo_path, &branch_name, RemoteAuth::GitHub(auth));
    }

    let branch_name = current_branch_name(repo_path)?
        .ok_or_else(|| "Pull requires a checked-out branch.".to_string())?;
    pull_with_git2(repo_path, &branch_name, RemoteAuth::System)
}

pub fn preview_discard_damage(repo: &Repository) -> DiscardPreview {
    let mut dirty_files = 0usize;
    let mut untracked_files = 0usize;

    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    if let Ok(statuses) = repo.statuses(Some(&mut opts)) {
        for entry in statuses.iter() {
            let status = entry.status();
            if status == Status::WT_NEW {
                untracked_files += 1;
            } else if status.intersects(
                Status::WT_MODIFIED
                    | Status::WT_DELETED
                    | Status::WT_TYPECHANGE
                    | Status::WT_RENAMED
                    | Status::INDEX_NEW
                    | Status::INDEX_MODIFIED
                    | Status::INDEX_DELETED
                    | Status::INDEX_RENAMED
                    | Status::INDEX_TYPECHANGE
                    | Status::CONFLICTED,
            ) {
                dirty_files += 1;
            }
        }
    }

    let local_only_commits = get_outgoing_commit_count(repo).unwrap_or(0);

    DiscardPreview {
        dirty_files,
        untracked_files,
        local_only_commits,
    }
}

pub fn preview_create_branch(repo: &Repository, branch_name: &str) -> CreateBranchPreview {
    let mut dirty_files = 0usize;
    let mut untracked_files = 0usize;
    let mut staged_files = 0usize;

    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    if let Ok(statuses) = repo.statuses(Some(&mut opts)) {
        for entry in statuses.iter() {
            let status = entry.status();
            if status.intersects(
                Status::INDEX_NEW
                    | Status::INDEX_MODIFIED
                    | Status::INDEX_DELETED
                    | Status::INDEX_RENAMED
                    | Status::INDEX_TYPECHANGE,
            ) {
                staged_files += 1;
            }
            if status.contains(Status::WT_NEW) {
                untracked_files += 1;
            } else if status.intersects(
                Status::WT_MODIFIED
                    | Status::WT_DELETED
                    | Status::WT_TYPECHANGE
                    | Status::WT_RENAMED
                    | Status::CONFLICTED,
            ) {
                dirty_files += 1;
            }
        }
    }

    CreateBranchPreview {
        branch_name: branch_name.to_string(),
        dirty_files,
        untracked_files,
        staged_files,
    }
}

pub fn discard_and_reset_to_remote(
    repo_path: &Path,
    auth: Option<&GithubAuthSession>,
    clean_untracked: bool,
) -> Result<String, String> {
    let branch_name = current_branch_name(repo_path)?
        .ok_or_else(|| "Reset requires a checked-out branch.".to_string())?;

    let remote_auth = if is_github_https_origin(repo_path) {
        let auth = auth.ok_or_else(|| {
            "Reset requires the app's GitHub sign-in. Use 'Sign in to GitHub...' and try again."
                .to_string()
        })?;
        RemoteAuth::GitHub(auth)
    } else {
        RemoteAuth::System
    };

    let repo = Repository::open(repo_path).map_err(|e| format!("Open repo error: {}", e))?;
    let mut remote = repo
        .find_remote("origin")
        .map_err(|e| format!("Remote error: {}", e))?;

    let mut fetch_options = git2::FetchOptions::new();
    fetch_options.remote_callbacks(remote_callbacks(&repo, remote_auth)?);
    fetch_options.prune(git2::FetchPrune::On);
    remote
        .fetch(&[&branch_name], Some(&mut fetch_options), None)
        .map_err(|e| format!("Fetch error: {}", e))?;

    let fetch_ref_name = format!("refs/remotes/origin/{}", branch_name);
    let fetch_ref = repo
        .find_reference(&fetch_ref_name)
        .map_err(|e| format!("Remote branch {} not found: {}", branch_name, e))?;
    let target_oid = fetch_ref
        .target()
        .ok_or_else(|| format!("Remote branch {} has no target commit", branch_name))?;
    let target_commit = repo
        .find_commit(target_oid)
        .map_err(|e| format!("Find commit error: {}", e))?;

    repo.reset(target_commit.as_object(), git2::ResetType::Hard, None)
        .map_err(|e| format!("Reset error: {}", e))?;

    let mut cleaned = 0usize;
    if clean_untracked {
        cleaned =
            clean_untracked_files(&repo).map_err(|e| format!("Clean untracked error: {}", e))?;
    }

    let mut message = format!("Reset to origin/{}", branch_name);
    if cleaned > 0 {
        message.push_str(&format!(", removed {} untracked entry(ies)", cleaned));
    }
    Ok(message)
}

fn clean_untracked_files(repo: &Repository) -> Result<usize, git2::Error> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| git2::Error::from_str("Repository has no workdir"))?
        .to_path_buf();

    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(false);
    let statuses = repo.statuses(Some(&mut opts))?;

    let mut count = 0usize;
    for entry in statuses.iter() {
        if entry.status() != Status::WT_NEW {
            continue;
        }
        let Some(path) = entry.path() else { continue };
        let full_path = workdir.join(path);
        let removed = if full_path.is_dir() {
            std::fs::remove_dir_all(&full_path).is_ok()
        } else if full_path.exists() {
            std::fs::remove_file(&full_path).is_ok()
        } else {
            false
        };
        if removed {
            count += 1;
        }
    }

    Ok(count)
}

pub fn create_tag(
    repo_path: &Path,
    tag_name: &str,
    auth: Option<&GithubAuthSession>,
) -> Result<String, String> {
    let tag_name = tag_name.trim();
    if tag_name.is_empty() {
        return Err("Tag name cannot be empty.".into());
    }

    let branch_name = current_branch_name(repo_path)?
        .ok_or_else(|| "Tag creation requires a checked-out branch.".to_string())?;
    if !can_create_tag_on_branch(&branch_name) {
        return Err("Tags can only be created from the main or master branch.".into());
    }

    let refname = format!("refs/tags/{}", tag_name);
    if !git2::Reference::is_valid_name(&refname) {
        return Err("Invalid tag name.".into());
    }

    let repo = Repository::open(repo_path).map_err(|e| format!("Open repo error: {}", e))?;
    if repo.find_reference(&refname).is_ok() {
        return Err("Tag already exists.".into());
    }

    let target = repo
        .head()
        .and_then(|head| head.peel(git2::ObjectType::Commit))
        .map_err(|_| "Cannot create a tag without a current commit.".to_string())?;
    repo.tag_lightweight(tag_name, &target, false)
        .map_err(|e| format!("Create tag error: {}", e))?;

    if !has_origin_remote(&repo) {
        return Ok(format!("Created local tag {}", tag_name));
    }

    match push_tag(repo_path, tag_name, auth) {
        Ok(()) => Ok(format!("Created and pushed tag {}", tag_name)),
        Err(error) => match rollback_tag(&repo, tag_name) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(format!(
                "{} Local tag rollback also failed: {}",
                error, rollback_error
            )),
        },
    }
}

pub fn github_auth_login<F>(client_id: &str, on_prompt: F) -> Result<GithubAuthSession, String>
where
    F: FnOnce(GithubAuthPrompt),
{
    let client = github_http_client()?;
    let device_response = client
        .post("https://github.com/login/device/code")
        .header(ACCEPT, "application/json")
        .header(USER_AGENT, "justanothergitgui")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "client_id={}&scope={}",
            urlencoding::encode(client_id),
            urlencoding::encode("repo")
        ))
        .send()
        .map_err(|e| format!("GitHub device sign-in failed: {}", e))?;

    if !device_response.status().is_success() {
        return Err(format!(
            "GitHub device sign-in failed with status {}",
            device_response.status()
        ));
    }

    let device: GithubDeviceCodeResponse = device_response
        .json()
        .map_err(|e| format!("Invalid GitHub device sign-in response: {}", e))?;

    let open_url = device
        .verification_uri_complete
        .as_deref()
        .unwrap_or(&device.verification_uri);
    on_prompt(GithubAuthPrompt {
        user_code: device.user_code.clone(),
        verification_uri: device.verification_uri.clone(),
        browser_url: open_url.to_string(),
    });
    let _ = webbrowser::open(open_url);

    let mut poll_interval = device.interval.unwrap_or(5).max(1);
    let mut remaining_seconds = device.expires_in;

    while remaining_seconds > 0 {
        std::thread::sleep(Duration::from_secs(poll_interval));
        remaining_seconds = remaining_seconds.saturating_sub(poll_interval);

        let token_response = client
            .post("https://github.com/login/oauth/access_token")
            .header(ACCEPT, "application/json")
            .header(USER_AGENT, "justanothergitgui")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!(
                "client_id={}&device_code={}&grant_type={}",
                urlencoding::encode(client_id),
                urlencoding::encode(&device.device_code),
                urlencoding::encode("urn:ietf:params:oauth:grant-type:device_code")
            ))
            .send()
            .map_err(|e| format!("GitHub token exchange failed: {}", e))?;

        if !token_response.status().is_success() {
            return Err(format!(
                "GitHub token exchange failed with status {}",
                token_response.status()
            ));
        }

        let token_body: GithubTokenResponse = token_response
            .json()
            .map_err(|e| format!("Invalid GitHub token response: {}", e))?;

        if let Some(access_token) = token_body.access_token {
            return fetch_github_user(&client, &access_token);
        }

        match token_body.error.as_deref() {
            Some("authorization_pending") => continue,
            Some("slow_down") => {
                poll_interval += 5;
            }
            Some("access_denied") => return Err("GitHub sign-in was cancelled.".into()),
            Some("expired_token") => {
                return Err("GitHub sign-in timed out before authorization completed.".into());
            }
            _ => {
                let message = token_body
                    .error_description
                    .or(token_body.error)
                    .unwrap_or_else(|| "GitHub did not return an access token".into());
                return Err(normalize_github_oauth_error(message));
            }
        }
    }

    Err("GitHub sign-in timed out before authorization completed.".into())
}

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
        .map_err(|e| format!("Invalid folder: {}", e))?;
    if !folder_path.is_dir() {
        return Err("Selected path is not a folder".into());
    }

    let repo = open_or_init_repo(&folder_path)?;
    if repo.find_remote("origin").is_ok() {
        return Err("Remote 'origin' already exists for this repository".into());
    }

    let has_changes = repo_has_changes(&repo)?;
    let has_head = repo.head().ok().and_then(|head| head.target()).is_some();
    if has_changes || !has_head {
        stage_all(&repo).map_err(|e| format!("Stage all error: {}", e))?;
        create_commit(&repo, commit_message).map_err(|e| format!("Commit error: {}", e))?;
    }

    let client = github_http_client()?;
    let (owner, repo_name_only) = parse_target_repo_name(repo_name, &request.auth.login)?;
    let create_url = if owner == request.auth.login {
        "https://api.github.com/user/repos".to_string()
    } else {
        format!("https://api.github.com/orgs/{}/repos", owner)
    };
    let github_repo: GithubRepo = client
        .post(create_url)
        .header(
            AUTHORIZATION,
            format!("Bearer {}", request.auth.access_token),
        )
        .header(ACCEPT, "application/vnd.github+json")
        .header(USER_AGENT, "justanothergitgui")
        .json(&GithubCreateRepoBody {
            name: &repo_name_only,
            private: request.visibility.is_private(),
        })
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|e| format!("GitHub repository creation failed: {}", e))?
        .json()
        .map_err(|e| format!("Invalid GitHub repository response: {}", e))?;

    repo.remote("origin", &github_repo.clone_url)
        .map_err(|e| format!("Remote add error: {}", e))?;
    let push_result = push(&folder_path, Some(&request.auth))?;
    let message = format!(
        "Created GitHub repository {}. {}",
        repo_name, push_result.message
    );

    Ok(CreateGithubRepoSuccess {
        folder_path,
        message,
    })
}

pub fn open_pull_request(url: &str) -> Result<String, String> {
    webbrowser::open(url).map_err(|e| format!("Could not open pull request: {}", e))?;
    Ok("Opened pull request in browser".into())
}

pub fn create_pull_request(url: &str) -> Result<String, String> {
    webbrowser::open(url)
        .map_err(|e| format!("Could not open pull request creation page: {}", e))?;
    Ok("Opened pull request creation in browser".into())
}

pub fn switch_branch(repo: &Repository, branch_name: &str) -> Result<(), git2::Error> {
    let refname = format!("refs/heads/{}", branch_name);
    let obj = repo.revparse_single(&refname)?;

    repo.checkout_tree(&obj, Some(git2::build::CheckoutBuilder::new().safe()))?;
    repo.set_head(&refname)?;

    Ok(())
}

pub fn validate_new_branch_name(repo: &Repository, name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let refname = format!("refs/heads/{}", name);
    if !git2::Reference::is_valid_name(&refname) {
        return Some(
            "Invalid name. Avoid spaces, '..', '~', '^', ':', '?', '*', '[', '\\', \
             and leading/trailing '/' or '.'."
                .into(),
        );
    }
    if repo.find_branch(name, git2::BranchType::Local).is_ok() {
        return Some(format!("A branch named '{}' already exists.", name));
    }
    None
}

pub fn create_branch(repo: &Repository, branch_name: &str) -> Result<(), git2::Error> {
    let branch_name = branch_name.trim();
    if branch_name.is_empty() {
        return Err(git2::Error::from_str("Branch name cannot be empty"));
    }

    let refname = format!("refs/heads/{}", branch_name);
    if !git2::Reference::is_valid_name(&refname) {
        return Err(git2::Error::from_str("Invalid branch name"));
    }

    if repo
        .find_branch(branch_name, git2::BranchType::Local)
        .is_ok()
    {
        return Err(git2::Error::from_str("Branch already exists"));
    }

    let head = repo
        .head()
        .map_err(|_| git2::Error::from_str("Cannot create a branch without a current commit"))?;
    let commit = head
        .peel_to_commit()
        .map_err(|_| git2::Error::from_str("Cannot create a branch without a current commit"))?;

    repo.branch(branch_name, &commit, false)?;
    switch_branch(repo, branch_name)
}

pub fn list_stale_branches(repo: &Repository) -> Result<Vec<StaleBranch>, git2::Error> {
    let head_oid = repo.head().ok().and_then(|head| head.target());
    let mut stale = Vec::new();

    for entry in repo.branches(Some(git2::BranchType::Local))? {
        let (branch, _) = entry?;
        if branch.is_head() {
            continue;
        }
        let Some(name) = branch.name()?.map(str::to_string) else {
            continue;
        };
        if matches!(name.as_str(), "main" | "master") {
            continue;
        }

        let refname = format!("refs/heads/{}", name);
        let Ok(upstream_buf) = repo.branch_upstream_name(&refname) else {
            continue;
        };
        let Some(upstream_name) = upstream_buf.as_str() else {
            continue;
        };
        if repo.find_reference(upstream_name).is_ok() {
            continue;
        }

        let merged_into_head = match (head_oid, branch.get().target()) {
            (Some(head), Some(branch_oid)) => {
                head == branch_oid || repo.graph_descendant_of(head, branch_oid).unwrap_or(false)
            }
            _ => false,
        };

        stale.push(StaleBranch {
            name,
            merged_into_head,
            selected: merged_into_head,
        });
    }

    Ok(stale)
}

pub fn delete_local_branch(repo: &Repository, name: &str) -> Result<(), git2::Error> {
    let mut branch = repo.find_branch(name, git2::BranchType::Local)?;
    if branch.is_head() {
        return Err(git2::Error::from_str(
            "Cannot delete the currently checked-out branch",
        ));
    }
    branch.delete()
}

fn open_or_init_repo(folder_path: &Path) -> Result<Repository, String> {
    if let Ok(repo) = Repository::open(folder_path) {
        return Ok(repo);
    }

    if let Ok(repo) = Repository::discover(folder_path) {
        let existing_root = repo_root_path(&repo)
            .canonicalize()
            .unwrap_or_else(|_| repo_root_path(&repo));
        let selected_root = folder_path
            .canonicalize()
            .unwrap_or_else(|_| folder_path.to_path_buf());
        if existing_root != selected_root {
            return Err(format!(
                "Selected folder is inside an existing repository: {}",
                existing_root.display()
            ));
        }
    }

    let mut options = git2::RepositoryInitOptions::new();
    options.initial_head("main");
    Repository::init_opts(folder_path, &options).map_err(|e| e.to_string())
}

fn repo_has_changes(repo: &Repository) -> Result<bool, String> {
    let (unstaged, staged) = get_file_statuses(repo).map_err(|e| e.to_string())?;
    Ok(!unstaged.is_empty() || !staged.is_empty())
}

fn current_branch_name(repo_path: &Path) -> Result<Option<String>, String> {
    let repo = Repository::open(repo_path).map_err(|e| format!("Open repo error: {}", e))?;
    let head = match repo.head() {
        Ok(head) => head,
        Err(_) => return Ok(None),
    };

    if !head.is_branch() {
        return Ok(None);
    }

    Ok(head.shorthand().map(ToOwned::to_owned))
}

fn github_http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Could not create GitHub HTTP client: {}", e))
}

fn github_auth_keyring_entry() -> Result<Entry, String> {
    Entry::new(GITHUB_AUTH_KEYRING_SERVICE, GITHUB_AUTH_KEYRING_USER)
        .map_err(|e| format!("Could not access system keychain: {}", e))
}

fn normalize_github_oauth_error(message: String) -> String {
    if message.contains("incorrect_client_credentials")
        || message.contains("client_id and/or client_secret passed are incorrect")
        || message.contains("client_id is invalid")
        || message.contains("device flow is disabled")
    {
        return "GitHub OAuth configuration error: the configured client ID is not valid for a GitHub OAuth App device flow. Use a GitHub OAuth App client ID and make sure Device Flow is enabled for that app.".into();
    }

    message
}

fn fetch_github_user(client: &Client, access_token: &str) -> Result<GithubAuthSession, String> {
    let user: GithubUser = client
        .get("https://api.github.com/user")
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .header(ACCEPT, "application/vnd.github+json")
        .header(USER_AGENT, "justanothergitgui")
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|e| format!("GitHub user lookup failed: {}", e))?
        .json()
        .map_err(|e| format!("Invalid GitHub user response: {}", e))?;

    Ok(GithubAuthSession {
        access_token: access_token.to_string(),
        login: user.login,
    })
}

fn try_push_with_auth(
    repo_path: &Path,
    branch_name: Option<&str>,
    auth: Option<&GithubAuthSession>,
) -> Result<Option<String>, String> {
    if !is_github_https_origin(repo_path) {
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

    push_with_git2(repo_path, branch_name, RemoteAuth::GitHub(auth))?;
    Ok(Some("Push successful".into()))
}

fn push_with_git2(
    repo_path: &Path,
    branch_name: &str,
    auth: RemoteAuth<'_>,
) -> Result<String, String> {
    let repo = Repository::open(repo_path).map_err(|e| format!("Open repo error: {}", e))?;
    let mut remote = repo
        .find_remote("origin")
        .map_err(|e| format!("Remote error: {}", e))?;
    let mut push_options = git2::PushOptions::new();
    let push_rejected: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let push_rejected_cb = Arc::clone(&push_rejected);
    let mut callbacks = remote_callbacks(&repo, auth)?;
    callbacks.push_update_reference(move |_refname, status| {
        if let Some(msg) = status {
            *push_rejected_cb.lock().unwrap() = Some(msg.to_string());
        }
        Ok(())
    });
    push_options.remote_callbacks(callbacks);
    let refspec = format!("refs/heads/{0}:refs/heads/{0}", branch_name);
    remote
        .push(&[&refspec], Some(&mut push_options))
        .map_err(|e| format!("Push error: {}", e))?;

    if let Some(reason) = push_rejected.lock().unwrap().take() {
        return Err(format!("Push rejected by remote: {}", reason));
    }

    sync_remote_tracking_branch(&repo, branch_name)?;

    if let Err(error) = repair_branch_upstream(&repo, branch_name) {
        return Err(format!("Upstream configuration error: {}", error));
    }

    Ok("Push successful".into())
}

fn sync_remote_tracking_branch(repo: &Repository, branch_name: &str) -> Result<(), String> {
    let branch_ref_name = format!("refs/heads/{}", branch_name);
    let local_oid = repo
        .refname_to_id(&branch_ref_name)
        .map_err(|e| format!("Push tracking update error: {}", e))?;
    let remote_ref_name = format!("refs/remotes/origin/{}", branch_name);
    repo.reference(
        &remote_ref_name,
        local_oid,
        true,
        "Update remote-tracking ref after push",
    )
    .map_err(|e| format!("Push tracking update error: {}", e))?;
    Ok(())
}

fn upstream_target_oid(
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

fn repair_branch_upstream(repo: &Repository, branch_name: &str) -> Result<(), String> {
    let remote_ref_name = format!("refs/remotes/origin/{}", branch_name);
    if repo.find_reference(&remote_ref_name).is_err() {
        return Ok(());
    }

    let mut branch = repo
        .find_branch(branch_name, git2::BranchType::Local)
        .map_err(|e| e.to_string())?;
    let needs_repair = match branch.upstream() {
        Ok(upstream) => {
            upstream.get().name() == Some(format!("refs/heads/{}", branch_name).as_str())
        }
        Err(_) => true,
    };

    if needs_repair {
        branch
            .set_upstream(Some(&format!("origin/{}", branch_name)))
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn push_tag(
    repo_path: &Path,
    tag_name: &str,
    auth: Option<&GithubAuthSession>,
) -> Result<(), String> {
    if is_github_https_origin(repo_path) {
        let auth = auth.ok_or_else(|| {
            "GitHub tag creation requires the app's GitHub sign-in. Use 'Sign in to GitHub...' and try again."
                .to_string()
        })?;
        return push_tag_with_git2(repo_path, tag_name, RemoteAuth::GitHub(auth));
    }

    push_tag_with_git2(repo_path, tag_name, RemoteAuth::System)
}

fn push_tag_with_git2(
    repo_path: &Path,
    tag_name: &str,
    auth: RemoteAuth<'_>,
) -> Result<(), String> {
    let repo = Repository::open(repo_path).map_err(|e| format!("Open repo error: {}", e))?;
    let mut remote = repo
        .find_remote("origin")
        .map_err(|e| format!("Remote error: {}", e))?;
    let mut push_options = git2::PushOptions::new();
    let push_rejected: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let push_rejected_cb = Arc::clone(&push_rejected);
    let mut callbacks = remote_callbacks(&repo, auth)?;
    callbacks.push_update_reference(move |_refname, status| {
        if let Some(msg) = status {
            *push_rejected_cb.lock().unwrap() = Some(msg.to_string());
        }
        Ok(())
    });
    push_options.remote_callbacks(callbacks);
    let refspec = format!("refs/tags/{0}:refs/tags/{0}", tag_name);
    remote
        .push(&[&refspec], Some(&mut push_options))
        .map_err(|e| format!("Tag push error: {}", e))?;

    if let Some(reason) = push_rejected.lock().unwrap().take() {
        return Err(format!("Tag push rejected by remote: {}", reason));
    }

    Ok(())
}

fn pull_with_git2(
    repo_path: &Path,
    branch_name: &str,
    auth: RemoteAuth<'_>,
) -> Result<String, String> {
    let repo = Repository::open(repo_path).map_err(|e| format!("Open repo error: {}", e))?;
    let mut remote = repo
        .find_remote("origin")
        .map_err(|e| format!("Remote error: {}", e))?;

    let mut fetch_options = git2::FetchOptions::new();
    fetch_options.remote_callbacks(remote_callbacks(&repo, auth)?);
    fetch_options.prune(git2::FetchPrune::On);
    let refspecs: [&str; 0] = [];
    remote
        .fetch(&refspecs, Some(&mut fetch_options), None)
        .map_err(|e| format!("Pull fetch error: {}", e))?;

    let fetch_ref_name = format!("refs/remotes/origin/{}", branch_name);
    let fetch_ref = repo
        .find_reference(&fetch_ref_name)
        .map_err(|e| format!("Pull reference error: {}", e))?;
    let fetch_commit = repo
        .reference_to_annotated_commit(&fetch_ref)
        .map_err(|e| format!("Pull analysis error: {}", e))?;

    let (analysis, _) = repo
        .merge_analysis(&[&fetch_commit])
        .map_err(|e| format!("Pull analysis error: {}", e))?;

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

enum RemoteAuth<'a> {
    GitHub(&'a GithubAuthSession),
    System,
}

fn parse_target_repo_name(
    repo_name: &str,
    fallback_owner: &str,
) -> Result<(String, String), String> {
    if let Some((owner, name)) = repo_name.split_once('/') {
        let owner = owner.trim();
        let name = name.trim();
        if owner.is_empty() || name.is_empty() {
            return Err("Repository name must look like owner/name or name".into());
        }
        Ok((owner.to_string(), name.to_string()))
    } else {
        Ok((fallback_owner.to_string(), repo_name.trim().to_string()))
    }
}

fn github_repo_slug(repo_path: &Path) -> Option<(String, String)> {
    let repo = Repository::open(repo_path).ok()?;
    let remote = repo.find_remote("origin").ok()?;
    let url = remote.url()?;
    parse_github_remote_slug(url)
}

fn parse_github_remote_slug(remote_url: &str) -> Option<(String, String)> {
    if let Some(rest) = remote_url.strip_prefix("https://github.com/") {
        return parse_github_slug(rest);
    }
    if let Some(rest) = remote_url.strip_prefix("http://github.com/") {
        return parse_github_slug(rest);
    }
    if let Some(rest) = remote_url.strip_prefix("git@github.com:") {
        return parse_github_slug(rest);
    }
    if let Some(rest) = remote_url.strip_prefix("ssh://git@github.com/") {
        return parse_github_slug(rest);
    }
    None
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

fn github_remote_callbacks(auth: &GithubAuthSession) -> git2::RemoteCallbacks<'static> {
    let token = auth.access_token.clone();
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(move |_url, username_from_url, _allowed_types| {
        git2::Cred::userpass_plaintext(username_from_url.unwrap_or("x-access-token"), &token)
    });
    callbacks
}

fn standard_remote_callbacks(repo: &Repository) -> Result<git2::RemoteCallbacks<'static>, String> {
    let config = repo
        .config()
        .map_err(|e| format!("Credential configuration error: {}", e))?;
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
    Ok(callbacks)
}

fn parse_github_slug(slug: &str) -> Option<(String, String)> {
    let slug = slug.trim_end_matches(".git");
    let mut parts = slug.splitn(2, '/');
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim();
    if owner.is_empty() || repo.is_empty() {
        None
    } else {
        Some((owner.to_string(), repo.to_string()))
    }
}

fn rollback_tag(repo: &Repository, tag_name: &str) -> Result<(), String> {
    let refname = format!("refs/tags/{}", tag_name);
    let mut reference = repo
        .find_reference(&refname)
        .map_err(|e| format!("Tag rollback error: {}", e))?;
    reference
        .delete()
        .map_err(|e| format!("Tag rollback error: {}", e))
}

fn is_github_https_origin(repo_path: &Path) -> bool {
    let Ok(repo) = Repository::open(repo_path) else {
        return false;
    };
    let Ok(remote) = repo.find_remote("origin") else {
        return false;
    };
    remote.url().is_some_and(is_github_https_url)
}

fn is_github_https_url(url: &str) -> bool {
    url.starts_with("https://github.com/")
}

fn detect_pull_request_prompt(
    repo_path: &Path,
    branch: &str,
    auth: Option<&GithubAuthSession>,
) -> Result<Option<PullRequestPrompt>, String> {
    let Some(auth) = auth else {
        return Ok(None);
    };
    let Some((owner, repo)) = github_repo_slug(repo_path) else {
        return Ok(None);
    };

    let client = github_http_client()?;
    let pulls: Vec<GithubPullRequest> = client
        .get(format!(
            "https://api.github.com/repos/{owner}/{repo}/pulls?state=open&head={}%3A{}",
            urlencoding::encode(&owner),
            urlencoding::encode(branch)
        ))
        .header(AUTHORIZATION, format!("Bearer {}", auth.access_token))
        .header(ACCEPT, "application/vnd.github+json")
        .header(USER_AGENT, "justanothergitgui")
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|e| format!("GitHub PR lookup failed: {}", e))?
        .json()
        .map_err(|e| format!("Invalid GitHub PR response: {}", e))?;

    if let Some(pr) = pulls.into_iter().next() {
        return Ok(Some(PullRequestPrompt::Open {
            branch: branch.to_string(),
            number: pr.number,
            url: pr.html_url,
        }));
    }

    let repo_info: GithubRepo = client
        .get(format!("https://api.github.com/repos/{owner}/{repo}"))
        .header(AUTHORIZATION, format!("Bearer {}", auth.access_token))
        .header(ACCEPT, "application/vnd.github+json")
        .header(USER_AGENT, "justanothergitgui")
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|e| format!("GitHub repository lookup failed: {}", e))?
        .json()
        .map_err(|e| format!("Invalid GitHub repository response: {}", e))?;

    Ok(Some(PullRequestPrompt::Create {
        branch: branch.to_string(),
        url: format!(
            "{}/compare/{}...{}?expand=1",
            repo_info.html_url,
            urlencoding::encode(&repo_info.default_branch),
            urlencoding::encode(branch)
        ),
    }))
}

fn repo_root_path(repo: &Repository) -> PathBuf {
    repo.workdir()
        .map(|path| path.to_path_buf())
        .unwrap_or_else(|| repo.path().parent().unwrap_or(repo.path()).to_path_buf())
}

fn fast_forward_branch(
    repo: &Repository,
    branch_name: &str,
    fetch_commit: &git2::AnnotatedCommit<'_>,
) -> Result<(), String> {
    let refname = format!("refs/heads/{}", branch_name);

    let target_obj = repo
        .find_object(fetch_commit.id(), None)
        .map_err(|e| format!("Pull fast-forward error: {}", e))?;
    repo.checkout_tree(
        &target_obj,
        Some(git2::build::CheckoutBuilder::new().safe()),
    )
    .map_err(|e| format!("Pull checkout error: {}", e))?;

    let mut branch_ref = repo
        .find_reference(&refname)
        .map_err(|e| format!("Pull fast-forward error: {}", e))?;
    branch_ref
        .set_target(fetch_commit.id(), &format!("Fast-forward {}", refname))
        .map_err(|e| format!("Pull fast-forward error: {}", e))?;
    repo.set_head(&refname)
        .map_err(|e| format!("Pull head update error: {}", e))?;
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
        .map_err(|e| format!("Pull head error: {}", e))?;
    let remote_commit = repo
        .find_commit(fetch_commit.id())
        .map_err(|e| format!("Pull merge error: {}", e))?;

    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.safe();
    repo.merge(&[fetch_commit], None, Some(&mut checkout))
        .map_err(|e| format!("Pull merge error: {}", e))?;

    let mut index = repo
        .index()
        .map_err(|e| format!("Pull index error: {}", e))?;
    if index.has_conflicts() {
        return Ok(format!(
            "Pull completed with conflicts on {}. Resolve them and commit the merge.",
            branch_name
        ));
    }

    let tree_oid = index
        .write_tree_to(repo)
        .map_err(|e| format!("Pull tree error: {}", e))?;
    let tree = repo
        .find_tree(tree_oid)
        .map_err(|e| format!("Pull tree error: {}", e))?;
    let signature = repo
        .signature()
        .map_err(|e| format!("Pull signature error: {}", e))?;
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
    .map_err(|e| format!("Pull commit error: {}", e))?;
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().safe()))
        .map_err(|e| format!("Pull checkout error: {}", e))?;
    repo.cleanup_state()
        .map_err(|e| format!("Pull cleanup error: {}", e))?;

    Ok("Pull successful".into())
}

fn repo_workdir(repo: &Repository) -> Result<&Path, git2::Error> {
    repo.workdir()
        .ok_or_else(|| git2::Error::from_str("Bare repositories are not supported"))
}

#[cfg(test)]
mod tests {
    use super::{is_github_https_url, parse_github_remote_slug};
    use git2::Repository;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestRepoDir {
        path: PathBuf,
    }

    impl TestRepoDir {
        fn init_with_origin(origin_url: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "justanothergitgui-git-ops-test-{}-{}",
                std::process::id(),
                unique
            ));
            std::fs::create_dir_all(&path).expect("create temp repo dir");
            let repo = Repository::init(&path).expect("init temp repo");
            repo.remote("origin", origin_url)
                .expect("add origin remote");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestRepoDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn treats_https_github_remotes_as_app_auth_candidates() {
        assert!(is_github_https_url(
            "https://github.com/octocat/hello-world.git"
        ));
    }

    #[test]
    fn keeps_github_ssh_remotes_on_system_auth_path() {
        assert_eq!(
            parse_github_remote_slug("git@github.com:octocat/hello-world.git"),
            Some(("octocat".into(), "hello-world".into()))
        );
        assert_eq!(
            parse_github_remote_slug("ssh://git@github.com/octocat/hello-world.git"),
            Some(("octocat".into(), "hello-world".into()))
        );
        assert!(!is_github_https_url(
            "git@github.com:octocat/hello-world.git"
        ));
        assert!(!is_github_https_url(
            "ssh://git@github.com/octocat/hello-world.git"
        ));
    }

    #[test]
    fn github_https_pushes_require_app_auth() {
        let repo_dir = TestRepoDir::init_with_origin("https://github.com/octocat/hello-world.git");
        let error = super::try_push_with_auth(repo_dir.path(), Some("main"), None)
            .expect_err("https GitHub remotes should require app auth");
        assert!(error.contains("GitHub push requires the app's GitHub sign-in"));
    }

    #[test]
    fn github_ssh_pushes_fall_back_to_system_auth() {
        let repo_dir = TestRepoDir::init_with_origin("git@github.com:octocat/hello-world.git");
        assert_eq!(
            super::try_push_with_auth(repo_dir.path(), Some("main"), None)
                .expect("ssh GitHub remotes should stay on system auth"),
            None
        );
    }
}

impl GithubRepoVisibility {
    fn is_private(self) -> bool {
        matches!(self, GithubRepoVisibility::Private)
    }
}

// --- Commit history ---

pub fn get_commit_history(
    repo: &Repository,
    limit: usize,
) -> Result<Vec<CommitEntry>, git2::Error> {
    let mut branch_map: HashMap<String, Vec<String>> = HashMap::new();
    if let Ok(branches) = repo.branches(Some(git2::BranchType::Local)) {
        for branch in branches {
            if let Ok((branch, _)) = branch {
                if let (Ok(Some(name)), Some(target)) = (branch.name(), branch.get().target()) {
                    branch_map
                        .entry(target.to_string())
                        .or_default()
                        .push(name.to_string());
                }
            }
        }
    }

    if let Ok(tag_names) = repo.tag_names(None) {
        for name in tag_names.iter().flatten() {
            let refname = format!("refs/tags/{}", name);
            if let Ok(reference) = repo.find_reference(&refname)
                && let Ok(target) = reference.peel_to_commit()
            {
                branch_map
                    .entry(target.id().to_string())
                    .or_default()
                    .push(name.to_string());
            }
        }
    }

    let repo_dir = repo
        .workdir()
        .or_else(|| repo.path().parent())
        .ok_or_else(|| git2::Error::from_str("Cannot determine repository path"))?;
    let output = Command::new("git")
        .args([
            "log",
            "--graph",
            "--topo-order",
            "-n",
            &limit.to_string(),
            "--format=format:%x1f%H%x1f%h%x1f%P%x1f%an%x1f%at%x1f%s",
        ])
        .current_dir(repo_dir)
        .output()
        .map_err(|e| git2::Error::from_str(&e.to_string()))?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let mut history = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let parts: Vec<&str> = line.splitn(7, '\x1f').collect();
        if parts.len() != 7 {
            continue;
        }

        let oid = parts[1];
        let short_oid = parts[2];
        let parents = parts[3];
        let author = parts[4];
        let timestamp = parts[5].parse::<i64>().unwrap_or_default();
        let message = parts[6];

        history.push(CommitEntry {
            short_oid: short_oid.to_string(),
            message: message.to_string(),
            author: author.to_string(),
            time: format_relative_time(now, timestamp),
            is_merge: parents.split_whitespace().count() > 1,
            branch_labels: branch_map.remove(oid).unwrap_or_default(),
        });
    }

    Ok(history)
}

fn format_relative_time(now: i64, then: i64) -> String {
    let diff = now - then;
    if diff < 0 {
        return "in the future".into();
    }
    if diff < 60 {
        return "just now".into();
    }
    if diff < 3600 {
        return format!("{}m ago", diff / 60);
    }
    if diff < 86400 {
        return format!("{}h ago", diff / 3600);
    }
    if diff < 2592000 {
        return format!("{}d ago", diff / 86400);
    }
    format!("{}mo ago", diff / 2592000)
}

// --- Conflict resolution ---

pub fn read_conflict_file(repo: &Repository, path: &str) -> Result<ConflictData, String> {
    let full_path = repo_workdir(repo).map_err(|e| e.to_string())?.join(path);
    let content = std::fs::read_to_string(&full_path).map_err(|e| e.to_string())?;
    let sections = parse_conflict_markers(&content);
    Ok(ConflictData {
        path: path.to_string(),
        sections,
    })
}

fn parse_conflict_markers(content: &str) -> Vec<ConflictPart> {
    let mut sections = Vec::new();
    let mut common = String::new();
    let mut ours = String::new();
    let mut theirs = String::new();
    let mut in_ours = false;
    let mut in_theirs = false;

    for line in content.lines() {
        if line.starts_with("<<<<<<<") {
            if !common.is_empty() {
                sections.push(ConflictPart::Common(std::mem::take(&mut common)));
            }
            in_ours = true;
        } else if line.starts_with("=======") && in_ours {
            in_ours = false;
            in_theirs = true;
        } else if line.starts_with(">>>>>>>") && in_theirs {
            in_theirs = false;
            sections.push(ConflictPart::Conflict {
                ours: std::mem::take(&mut ours),
                theirs: std::mem::take(&mut theirs),
                resolution: ConflictChoice::default(),
            });
        } else if in_ours {
            if !ours.is_empty() {
                ours.push('\n');
            }
            ours.push_str(line);
        } else if in_theirs {
            if !theirs.is_empty() {
                theirs.push('\n');
            }
            theirs.push_str(line);
        } else {
            if !common.is_empty() {
                common.push('\n');
            }
            common.push_str(line);
        }
    }

    if !common.is_empty() {
        sections.push(ConflictPart::Common(common));
    }

    sections
}

pub fn write_resolved_file(repo: &Repository, data: &ConflictData) -> Result<(), String> {
    let full_path = repo_workdir(repo)
        .map_err(|e| e.to_string())?
        .join(&data.path);
    let mut content = String::new();

    for (i, section) in data.sections.iter().enumerate() {
        if i > 0 {
            content.push('\n');
        }
        match section {
            ConflictPart::Common(text) => {
                content.push_str(text);
            }
            ConflictPart::Conflict {
                ours,
                theirs,
                resolution,
            } => match resolution {
                ConflictChoice::Ours => content.push_str(ours),
                ConflictChoice::Theirs => content.push_str(theirs),
                ConflictChoice::Both => {
                    content.push_str(ours);
                    content.push('\n');
                    content.push_str(theirs);
                }
                ConflictChoice::Unresolved => {
                    return Err("Not all conflicts resolved".into());
                }
            },
        }
    }

    content.push('\n');
    std::fs::write(&full_path, &content).map_err(|e| e.to_string())?;
    stage_file(repo, &data.path).map_err(|e| e.to_string())?;

    Ok(())
}
