use git2::{Repository, Status, StatusOptions};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

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

pub fn push(repo_path: &Path) -> Result<PushSuccess, String> {
    let branch_name = current_branch_name(repo_path)?;
    let output = Command::new("git")
        .args(["push"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        let msg = command_message(&output);
        Ok(PushSuccess {
            message: if msg.trim().is_empty() {
                "Push successful".into()
            } else {
                msg
            },
            pull_request_prompt: branch_name
                .as_deref()
                .and_then(|branch| detect_pull_request_prompt(repo_path, branch)),
        })
    } else if command_message(&output).contains("has no upstream branch") {
        let Some(branch_name) = branch_name else {
            return Err(command_message(&output));
        };

        let upstream_output = Command::new("git")
            .args(["push", "--set-upstream", "origin", &branch_name])
            .current_dir(repo_path)
            .output()
            .map_err(|e| e.to_string())?;

        if upstream_output.status.success() {
            let msg = command_message(&upstream_output);
            Ok(PushSuccess {
                message: if msg.trim().is_empty() {
                    format!("Push successful. Upstream set for {}", branch_name)
                } else {
                    msg
                },
                pull_request_prompt: detect_pull_request_prompt(repo_path, &branch_name),
            })
        } else {
            Err(command_message(&upstream_output))
        }
    } else {
        Err(command_message(&output))
    }
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

pub fn github_auth_status() -> Result<String, String> {
    let output = Command::new("gh")
        .args(["auth", "status", "--hostname", "github.com"])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        let message = command_message(&output);
        Ok(if message.trim().is_empty() {
            "Signed in to GitHub.".into()
        } else {
            message
        })
    } else {
        Err(command_message(&output))
    }
}

pub fn github_auth_login() -> Result<String, String> {
    let output = Command::new("gh")
        .args([
            "auth",
            "login",
            "--hostname",
            "github.com",
            "--web",
            "--git-protocol",
            "https",
        ])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        let message = command_message(&output);
        Ok(if message.trim().is_empty() {
            "GitHub authentication complete.".into()
        } else {
            message
        })
    } else {
        Err(command_message(&output))
    }
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

    github_auth_status().map_err(|msg| format!("GitHub authentication required. {}", msg))?;

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

    let output = Command::new("gh")
        .args([
            "repo",
            "create",
            repo_name,
            request.visibility.gh_flag(),
            "--source",
            ".",
            "--remote",
            "origin",
            "--push",
        ])
        .current_dir(&folder_path)
        .output()
        .map_err(|e| format!("Failed to run gh repo create: {}", e))?;

    if !output.status.success() {
        return Err(command_message(&output));
    }

    let details = command_message(&output);
    let message = if details.trim().is_empty() {
        format!("Created GitHub repository {}", repo_name)
    } else {
        format!(
            "Created GitHub repository {}. {}",
            repo_name,
            details.trim()
        )
    };

    Ok(CreateGithubRepoSuccess {
        folder_path,
        message,
    })
}

pub fn open_pull_request(repo_path: &Path, number: u64) -> Result<String, String> {
    let output = Command::new("gh")
        .args(["pr", "view", &number.to_string(), "--web"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        let message = command_message(&output);
        Ok(if message.trim().is_empty() {
            format!("Opened pull request #{}", number)
        } else {
            message
        })
    } else {
        Err(command_message(&output))
    }
}

pub fn create_pull_request(repo_path: &Path, branch: &str) -> Result<String, String> {
    let output = Command::new("gh")
        .args(["pr", "create", "--web", "--head", branch])
        .current_dir(repo_path)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        let message = command_message(&output);
        Ok(if message.trim().is_empty() {
            format!("Opened pull request creation for {}", branch)
        } else {
            message
        })
    } else {
        Err(command_message(&output))
    }
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

fn detect_pull_request_prompt(repo_path: &Path, branch: &str) -> Option<PullRequestPrompt> {
    let output = Command::new("gh")
        .args([
            "pr",
            "list",
            "--state",
            "open",
            "--head",
            branch,
            "--json",
            "number,url",
            "--template",
            "{{with index . 0}}{{.number}}\t{{.url}}{{end}}",
        ])
        .current_dir(repo_path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let details = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if details.is_empty() {
        return Some(PullRequestPrompt::Create {
            branch: branch.to_string(),
        });
    }

    let mut parts = details.splitn(2, '\t');
    let number = parts.next()?.trim().parse::<u64>().ok()?;
    let url = parts.next()?.trim().to_string();

    Some(PullRequestPrompt::Open {
        branch: branch.to_string(),
        number,
        url,
    })
}

fn repo_root_path(repo: &Repository) -> PathBuf {
    repo.workdir()
        .map(|path| path.to_path_buf())
        .unwrap_or_else(|| repo.path().parent().unwrap_or(repo.path()).to_path_buf())
}

impl GithubRepoVisibility {
    fn gh_flag(self) -> &'static str {
        match self {
            GithubRepoVisibility::Public => "--public",
            GithubRepoVisibility::Private => "--private",
        }
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
