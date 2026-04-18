use git2::{Repository, Status, StatusOptions};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::shared::git::{CommitEntry, CreateBranchPreview, DiscardPreview, StaleBranch};

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

    let _ = super::remotes::repair_branch_upstream(repo, branch_name);

    if let Some(upstream_oid) = super::remotes::upstream_target_oid(repo, branch_name)? {
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

pub fn suggest_next_tag(repo: &Repository) -> String {
    let Ok(tag_names) = repo.tag_names(None) else {
        return "v1.0.0.0".to_string();
    };

    let mut best: Option<([u32; 4], bool)> = None;
    for name in tag_names.iter().flatten() {
        if let Some((version, has_v_prefix)) = parse_semver_tag(name) {
            match best {
                Some((current, _)) if current >= version => {}
                _ => best = Some((version, has_v_prefix)),
            }
        }
    }

    match best {
        Some(([major, minor, patch, build], has_v_prefix)) => {
            let prefix = if has_v_prefix { "v" } else { "" };
            format!("{}{}.{}.{}.{}", prefix, major, minor, patch, build + 1)
        }
        None => "v1.0.0.0".to_string(),
    }
}

pub fn has_origin_remote(repo: &Repository) -> bool {
    repo.find_remote("origin").is_ok()
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
            "Invalid name. Avoid spaces, '..', '~', '^', ':', '?', '*', '[', '\\', and leading/trailing '/' or '.'."
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
        .map_err(|error| git2::Error::from_str(&error.to_string()))?;

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

pub(crate) fn open_or_init_repo(folder_path: &Path) -> Result<Repository, String> {
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
    Repository::init_opts(folder_path, &options).map_err(|error| error.to_string())
}

pub(crate) fn repo_has_changes(repo: &Repository) -> Result<bool, String> {
    let (unstaged, staged) =
        super::worktree::get_file_statuses(repo).map_err(|error| error.to_string())?;
    Ok(!unstaged.is_empty() || !staged.is_empty())
}

pub(crate) fn current_branch_name(repo_path: &Path) -> Result<Option<String>, String> {
    let repo =
        Repository::open(repo_path).map_err(|error| format!("Open repo error: {}", error))?;
    let head = match repo.head() {
        Ok(head) => head,
        Err(_) => return Ok(None),
    };

    if !head.is_branch() {
        return Ok(None);
    }

    Ok(head.shorthand().map(ToOwned::to_owned))
}

pub(crate) fn parse_semver_tag(name: &str) -> Option<([u32; 4], bool)> {
    let (rest, has_v_prefix) = match name.strip_prefix('v') {
        Some(rest) => (rest, true),
        None => (name, false),
    };
    let mut parts = rest.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next()?.parse::<u32>().ok()?;
    let patch = parts.next()?.parse::<u32>().ok()?;
    let build = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(([major, minor, patch, build], has_v_prefix))
}

pub(crate) fn repo_root_path(repo: &Repository) -> PathBuf {
    repo.workdir()
        .map(|path| path.to_path_buf())
        .unwrap_or_else(|| repo.path().parent().unwrap_or(repo.path()).to_path_buf())
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
