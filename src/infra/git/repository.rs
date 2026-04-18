use git2::{Repository, Status, StatusOptions};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::shared::git::{CommitEntry, CreateBranchPreview, DiscardPreview, StaleBranch};

pub fn open_repo(path: &Path) -> Result<Repository, git2::Error> {
    let repo = Repository::discover(path)?;
    if repo.is_bare() {
        return Err(git2::Error::from_str("Bare repositories are not supported"));
    }
    Ok(repo)
}

pub fn get_current_branch(repo: &Repository) -> Result<String, git2::Error> {
    match repo.head() {
        Ok(head) => Ok(head.shorthand().unwrap_or("HEAD").to_string()),
        Err(error)
            if matches!(
                error.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) =>
        {
            symbolic_head_branch_name(repo)
                .ok_or_else(|| git2::Error::from_str("Repository has no checked-out branch"))
        }
        Err(error) => Err(error),
    }
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
            format!(
                "{}{}.{}.{}.{}",
                prefix,
                major,
                minor,
                patch,
                build.saturating_add(1)
            )
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
    if limit == 0 {
        return Ok(Vec::new());
    }

    let mut branch_map = collect_commit_labels(repo);
    let Some(head_oid) = resolve_history_head(repo)? else {
        return Ok(Vec::new());
    };

    let walk = build_history_revwalk(repo, head_oid)?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let mut history = Vec::new();
    for oid in walk.take(limit) {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        history.push(build_commit_entry(
            repo,
            oid,
            &commit,
            now,
            branch_map.remove(&oid).unwrap_or_default(),
        )?);
    }

    Ok(history)
}

fn resolve_history_head(repo: &Repository) -> Result<Option<git2::Oid>, git2::Error> {
    match repo.head() {
        Ok(head) => Ok(head.target()),
        Err(error)
            if matches!(
                error.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn collect_commit_labels(repo: &Repository) -> HashMap<git2::Oid, Vec<String>> {
    let mut labels: HashMap<git2::Oid, Vec<String>> = HashMap::new();

    if let Ok(branches) = repo.branches(Some(git2::BranchType::Local)) {
        for branch in branches {
            if let Ok((branch, _)) = branch {
                if let (Ok(Some(name)), Some(target)) = (branch.name(), branch.get().target()) {
                    labels.entry(target).or_default().push(name.to_string());
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
                labels
                    .entry(target.id())
                    .or_default()
                    .push(name.to_string());
            }
        }
    }

    labels
}

fn build_history_revwalk(
    repo: &Repository,
    head_oid: git2::Oid,
) -> Result<git2::Revwalk<'_>, git2::Error> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
    walk.push(head_oid)?;
    Ok(walk)
}

fn build_commit_entry(
    repo: &Repository,
    oid: git2::Oid,
    commit: &git2::Commit<'_>,
    now: i64,
    branch_labels: Vec<String>,
) -> Result<CommitEntry, git2::Error> {
    let short_oid = repo
        .find_object(oid, None)?
        .short_id()?
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| oid.to_string());

    Ok(CommitEntry {
        short_oid,
        message: commit.summary().unwrap_or_default().to_string(),
        author: commit.author().name().unwrap_or_default().to_string(),
        time: format_relative_time(now, commit.time().seconds()),
        is_merge: commit.parent_count() > 1,
        branch_labels,
    })
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
        Err(error)
            if matches!(
                error.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) =>
        {
            return Ok(symbolic_head_branch_name(&repo));
        }
        Err(_) => return Ok(None),
    };

    if !head.is_branch() {
        return Ok(None);
    }

    Ok(head.shorthand().map(ToOwned::to_owned))
}

fn symbolic_head_branch_name(repo: &Repository) -> Option<String> {
    let head = repo.find_reference("HEAD").ok()?;
    let target = head.symbolic_target()?;
    target.strip_prefix("refs/heads/").map(ToOwned::to_owned)
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

#[cfg(test)]
mod tests {
    use super::{get_commit_history, get_current_branch};
    use git2::{Repository, RepositoryInitOptions, Signature, Time};
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestRepoDir {
        path: PathBuf,
    }

    impl TestRepoDir {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "justanothergitgui-repository-test-{}-{}",
                std::process::id(),
                unique
            ));
            std::fs::create_dir_all(&path).expect("create temp repo dir");
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

    fn init_repo(path: &Path) -> Repository {
        let mut options = RepositoryInitOptions::new();
        options.initial_head("main");
        Repository::init_opts(path, &options).expect("init temp repo")
    }

    fn empty_tree(repo: &Repository) -> git2::Tree<'_> {
        let tree_id = {
            let mut index = repo.index().expect("index");
            index.write_tree().expect("write tree")
        };
        repo.find_tree(tree_id).expect("find tree")
    }

    fn signature(name: &str, timestamp: i64) -> Signature<'static> {
        Signature::new(name, "tester@example.com", &Time::new(timestamp, 0)).expect("signature")
    }

    #[test]
    fn get_commit_history_uses_git2_revwalk_and_keeps_labels() {
        let repo_dir = TestRepoDir::new();
        let repo = init_repo(repo_dir.path());
        let tree = empty_tree(&repo);

        let base_sig = signature("Base Author", 1_700_000_000);
        let base_oid = repo
            .commit(Some("HEAD"), &base_sig, &base_sig, "base", &tree, &[])
            .expect("base commit");
        let base_commit = repo.find_commit(base_oid).expect("find base");

        repo.branch("feature", &base_commit, false)
            .expect("create feature branch");

        let feature_sig = signature("Feature Author", 1_700_000_100);
        let feature_oid = repo
            .commit(
                Some("refs/heads/feature"),
                &feature_sig,
                &feature_sig,
                "feature work",
                &tree,
                &[&base_commit],
            )
            .expect("feature commit");
        let feature_commit = repo.find_commit(feature_oid).expect("find feature");

        let main_sig = signature("Main Author", 1_700_000_200);
        let main_oid = repo
            .commit(
                Some("HEAD"),
                &main_sig,
                &main_sig,
                "main work",
                &tree,
                &[&base_commit],
            )
            .expect("main commit");
        let main_commit = repo.find_commit(main_oid).expect("find main");

        let merge_sig = signature("Merge Author", 1_700_000_300);
        let merge_oid = repo
            .commit(
                Some("HEAD"),
                &merge_sig,
                &merge_sig,
                "merge feature",
                &tree,
                &[&main_commit, &feature_commit],
            )
            .expect("merge commit");
        let merge_commit = repo.find_commit(merge_oid).expect("find merge");

        repo.tag_lightweight("v1.0.0.0", merge_commit.as_object(), false)
            .expect("tag merge commit");

        let history = get_commit_history(&repo, 10).expect("history");

        assert_eq!(history.len(), 4);
        assert_eq!(history[0].message, "merge feature");
        assert!(history[0].is_merge);
        assert!(history[0].branch_labels.iter().any(|label| label == "main"));
        assert!(
            history[0]
                .branch_labels
                .iter()
                .any(|label| label == "v1.0.0.0")
        );
        assert!(history.iter().any(|entry| {
            entry.message == "feature work"
                && entry.author == "Feature Author"
                && entry.branch_labels.iter().any(|label| label == "feature")
        }));
        assert!(history.iter().any(|entry| entry.message == "main work"));
        assert!(history.iter().any(|entry| entry.message == "base"));
    }

    #[test]
    fn get_commit_history_returns_empty_for_unborn_head() {
        let repo_dir = TestRepoDir::new();
        let repo = init_repo(repo_dir.path());

        let history = get_commit_history(&repo, 10).expect("history");

        assert!(history.is_empty());
    }

    #[test]
    fn get_current_branch_returns_symbolic_branch_for_unborn_head() {
        let repo_dir = TestRepoDir::new();
        let repo = init_repo(repo_dir.path());

        let branch = get_current_branch(&repo).expect("current branch");

        assert_eq!(branch, "main");
    }
}
