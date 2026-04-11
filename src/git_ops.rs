use git2::{Repository, Status, StatusOptions};
use reqwest::Url;
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

use crate::state::{
    CommitEntry, ConflictChoice, ConflictData, ConflictPart, FileEntry, PullRequestPrompt,
};

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

#[derive(Clone, Debug)]
pub struct GithubAuthSession {
    pub access_token: String,
    pub login: String,
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

#[derive(Serialize)]
struct GithubCreateRepoBody<'a> {
    name: &'a str,
    private: bool,
}

pub fn open_repo(path: &Path) -> Result<Repository, git2::Error> {
    Repository::discover(path)
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

pub fn has_origin_remote(repo: &Repository) -> bool {
    repo.find_remote("origin").is_ok()
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
    let full_path = repo.workdir().unwrap().join(path);

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

    let parent = match repo.head() {
        Ok(head) => Some(head.peel_to_commit()?),
        Err(_) => None,
    };

    let parents: Vec<&git2::Commit> = parent.iter().collect();

    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
}

pub fn get_file_diff(repo: &Repository, path: &str, staged: bool) -> Result<String, git2::Error> {
    let mut opts = git2::DiffOptions::new();
    opts.pathspec(path);

    let diff = if staged {
        let head = repo.head()?.peel_to_tree()?;
        repo.diff_tree_to_index(Some(&head), None, Some(&mut opts))?
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
    let base_message = match try_push_with_auth(repo_path, branch_name.as_deref(), auth) {
        Ok(Some(message)) => message,
        Ok(None) => push_with_git_cli(repo_path, branch_name.as_deref())?,
        Err(error) => return Err(error),
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

    if pull_request_prompt.is_none() && auth.is_none() && is_github_origin(repo_path) {
        message.push_str(" Sign in to GitHub to enable PR actions.");
    }

    Ok(PushSuccess {
        message,
        pull_request_prompt,
    })
}

pub fn pull(repo_path: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .args(["pull"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        let msg = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(if msg.trim().is_empty() {
            "Pull successful".into()
        } else {
            msg
        })
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

pub fn github_auth_login(client_id: &str, redirect_uri: &str) -> Result<GithubAuthSession, String> {
    let redirect_url =
        Url::parse(redirect_uri).map_err(|e| format!("Invalid redirect URI: {}", e))?;
    let callback_host = redirect_url
        .host_str()
        .ok_or_else(|| "Redirect URI must include a host".to_string())?;
    let callback_port = redirect_url
        .port_or_known_default()
        .ok_or_else(|| "Redirect URI must include a port".to_string())?;
    let callback_path = redirect_url.path().to_string();

    let listener = TcpListener::bind((callback_host, callback_port))
        .map_err(|e| format!("Could not start OAuth callback server: {}", e))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("Could not configure OAuth callback server: {}", e))?;

    let state = generate_pkce_value(24);
    let code_verifier = generate_pkce_value(64);
    let code_challenge = pkce_challenge(&code_verifier);

    let mut authorize_url = Url::parse("https://github.com/login/oauth/authorize")
        .map_err(|e| format!("Could not build authorize URL: {}", e))?;
    authorize_url
        .query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", "repo")
        .append_pair("state", &state)
        .append_pair("code_challenge", &code_challenge)
        .append_pair("code_challenge_method", "S256");

    webbrowser::open(authorize_url.as_str())
        .map_err(|e| format!("Could not open browser for GitHub sign-in: {}", e))?;

    let (code, returned_state) = wait_for_oauth_callback(&listener, &callback_path)?;
    if returned_state != state {
        return Err("GitHub OAuth state validation failed".into());
    }

    let client = github_http_client()?;
    let token_response = client
        .post("https://github.com/login/oauth/access_token")
        .header(ACCEPT, "application/json")
        .header(USER_AGENT, "justanothergitgui")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "client_id={}&redirect_uri={}&code={}&code_verifier={}",
            urlencoding::encode(client_id),
            urlencoding::encode(redirect_uri),
            urlencoding::encode(&code),
            urlencoding::encode(&code_verifier)
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
    let access_token = token_body.access_token.ok_or_else(|| {
        token_body
            .error_description
            .or(token_body.error)
            .unwrap_or_else(|| "GitHub did not return an access token".into())
    })?;

    fetch_github_user(&client, &access_token)
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

fn command_message(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => String::new(),
        (false, true) => stdout,
        (true, false) => stderr,
        (false, false) => format!("{}\n{}", stdout, stderr),
    }
}

fn current_branch_name(repo_path: &Path) -> Result<Option<String>, String> {
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(command_message(&output));
    }

    let branch_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if branch_name.is_empty() {
        Ok(None)
    } else {
        Ok(Some(branch_name))
    }
}

fn github_http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Could not create GitHub HTTP client: {}", e))
}

fn generate_pkce_value(byte_len: usize) -> String {
    let mut bytes = vec![0_u8; byte_len];
    for byte in &mut bytes {
        *byte = rand::random::<u8>();
    }
    URL_SAFE_NO_PAD.encode(bytes)
}

fn pkce_challenge(code_verifier: &str) -> String {
    let digest = Sha256::digest(code_verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn wait_for_oauth_callback(
    listener: &TcpListener,
    expected_path: &str,
) -> Result<(String, String), String> {
    let deadline = Instant::now() + Duration::from_secs(180);
    let (mut stream, _) = loop {
        match listener.accept() {
            Ok(connection) => break connection,
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err("Timed out waiting for GitHub OAuth callback".into());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(format!("GitHub OAuth callback failed: {}", error)),
        }
    };

    let mut buffer = [0_u8; 4096];
    let bytes_read = stream
        .read(&mut buffer)
        .map_err(|e| format!("GitHub OAuth callback read failed: {}", e))?;
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let request_line = request
        .lines()
        .next()
        .ok_or_else(|| "GitHub OAuth callback request was empty".to_string())?;
    let request_target = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| "GitHub OAuth callback request was malformed".to_string())?;
    let callback_url = Url::parse(&format!("http://localhost{}", request_target))
        .map_err(|e| format!("GitHub OAuth callback URL was invalid: {}", e))?;

    let mut response_message = "GitHub sign-in complete. You can close this window.".to_string();
    let result = (|| {
        if callback_url.path() != expected_path {
            return Err(
                "GitHub OAuth callback path did not match the configured redirect URI".into(),
            );
        }

        let query: HashMap<String, String> = callback_url.query_pairs().into_owned().collect();
        if let Some(error) = query.get("error") {
            return Err(query
                .get("error_description")
                .cloned()
                .unwrap_or_else(|| format!("GitHub authorization failed: {}", error)));
        }

        let code = query.get("code").cloned().ok_or_else(|| {
            "GitHub OAuth callback did not include an authorization code".to_string()
        })?;
        let state = query
            .get("state")
            .cloned()
            .ok_or_else(|| "GitHub OAuth callback did not include a state value".to_string())?;
        Ok((code, state))
    })();

    if let Err(error) = &result {
        response_message = format!(
            "GitHub sign-in failed: {}. You can close this window.",
            error
        );
    }

    let body = response_message;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();

    result
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
    let (Some(branch_name), Some(auth)) = (branch_name, auth) else {
        return Ok(None);
    };

    if !is_github_https_origin(repo_path) {
        return Ok(None);
    }

    push_with_git2_auth(repo_path, branch_name, auth)?;
    Ok(Some("Push successful".into()))
}

fn push_with_git_cli(repo_path: &Path, branch_name: Option<&str>) -> Result<String, String> {
    let output = Command::new("git")
        .args(["push"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        let msg = command_message(&output);
        return Ok(if msg.trim().is_empty() {
            "Push successful".into()
        } else {
            msg
        });
    }

    if command_message(&output).contains("has no upstream branch") {
        let Some(branch_name) = branch_name else {
            return Err(command_message(&output));
        };
        let upstream_output = Command::new("git")
            .args(["push", "--set-upstream", "origin", branch_name])
            .current_dir(repo_path)
            .output()
            .map_err(|e| e.to_string())?;

        if upstream_output.status.success() {
            let msg = command_message(&upstream_output);
            return Ok(if msg.trim().is_empty() {
                format!("Push successful. Upstream set for {}", branch_name)
            } else {
                msg
            });
        }

        return Err(command_message(&upstream_output));
    }

    Err(command_message(&output))
}

fn push_with_git2_auth(
    repo_path: &Path,
    branch_name: &str,
    auth: &GithubAuthSession,
) -> Result<(), String> {
    let repo = Repository::open(repo_path).map_err(|e| format!("Open repo error: {}", e))?;
    let mut remote = repo
        .find_remote("origin")
        .map_err(|e| format!("Remote error: {}", e))?;
    let token = auth.access_token.clone();
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(move |_url, username_from_url, _allowed_types| {
        git2::Cred::userpass_plaintext(username_from_url.unwrap_or("x-access-token"), &token)
    });

    let mut push_options = git2::PushOptions::new();
    push_options.remote_callbacks(callbacks);
    let refspec = format!("refs/heads/{0}:refs/heads/{0}", branch_name);
    remote
        .push(&[&refspec], Some(&mut push_options))
        .map_err(|e| format!("Push error: {}", e))?;

    if let Ok(mut branch) = repo.find_branch(branch_name, git2::BranchType::Local) {
        branch
            .set_upstream(Some(branch_name))
            .map_err(|e| format!("Upstream configuration error: {}", e))?;
    }

    Ok(())
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

fn is_github_origin(repo_path: &Path) -> bool {
    github_repo_slug(repo_path).is_some()
}

fn is_github_https_origin(repo_path: &Path) -> bool {
    let Ok(repo) = Repository::open(repo_path) else {
        return false;
    };
    let Ok(remote) = repo.find_remote("origin") else {
        return false;
    };
    remote
        .url()
        .is_some_and(|url| url.starts_with("https://github.com/"))
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
    let full_path = repo.workdir().unwrap().join(path);
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
    let full_path = repo.workdir().unwrap().join(&data.path);
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
