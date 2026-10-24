//! libgit2 operations on *linked* worktrees (`git worktree list/add/remove`).
//!
//! Separate from [`super::worktree`], which is about the index and working tree
//! of one checkout. Everything here goes through `git2`'s worktree API; nothing
//! shells out to the `git` CLI.
//!
//! # Working from a secondary worktree
//!
//! `git_worktree_list`, `git_worktree_lookup` and `git_worktree_add` all resolve
//! through the repository's *commondir*, so they behave the same whether the
//! caller opened the main working tree or a linked one. [`main_repository`]
//! makes that explicit anyway, because the main worktree itself is not in
//! `worktrees()` and has to be synthesised from the main repository's workdir.

use std::path::{Path, PathBuf};

use git2::{Repository, Status, StatusOptions, WorktreeAddOptions, WorktreeLockStatus};

use crate::shared::worktrees::{
    CreatedWorktree, LinkedWorktree, LinkedWorktreeStatus, NewWorktreeRequest,
};

/// Open the repository that owns the shared object store.
///
/// For a linked worktree, `commondir()` is the main repository's gitdir, which
/// `Repository::open` resolves back to the main checkout.
pub(crate) fn main_repository(repo: &Repository) -> Result<Repository, git2::Error> {
    if repo.is_worktree() {
        Repository::open(repo.commondir())
    } else {
        Repository::open(repo.path())
    }
}

/// Canonicalised workdir, falling back to the path as given so a directory that
/// has since disappeared still compares by value rather than failing.
fn normalized(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// How a repository is identified across every tab that shows it.
///
/// Resolves to the shared object store, not the handle it was opened through:
/// opening a linked worktree in its own tab gives a `Repository` whose workdir
/// is that worktree, yet it lists the very same worktrees. Keying anything by
/// the tab's own workdir would file one repository's data under two names.
///
/// Canonicalised on purpose, unlike the verbatim paths `session.json` stores:
/// a session only has to reopen what was open, whereas this has to recognise
/// the same repository reached through a symlink or a trailing slash.
pub fn repository_key(repo: &Repository) -> Result<PathBuf, git2::Error> {
    let main = main_repository(repo)?;
    // A bare main repository has no workdir; its gitdir still identifies it.
    Ok(match main.workdir() {
        Some(workdir) => normalized(workdir),
        None => normalized(main.path()),
    })
}

/// Every checkout sharing this repository's object store: the main working tree
/// first, then the linked worktrees in git's own order.
///
/// A worktree that cannot be inspected is reported as
/// [`LinkedWorktreeStatus::Unavailable`] rather than dropped, so the panel can
/// show that it exists and why it could not be read.
pub fn list_worktrees(repo: &Repository) -> Result<Vec<LinkedWorktree>, git2::Error> {
    let main = main_repository(repo)?;
    let current = repo.workdir().map(normalized);
    let mut worktrees = Vec::new();

    if let Some(main_workdir) = main.workdir() {
        worktrees.push(describe_main(&main, main_workdir, current.as_deref()));
    }

    for name in main.worktrees()?.iter().flatten() {
        worktrees.push(describe_linked(&main, name, current.as_deref()));
    }

    Ok(worktrees)
}

/// The main working tree, which `worktrees()` never lists.
fn describe_main(repo: &Repository, workdir: &Path, current: Option<&Path>) -> LinkedWorktree {
    let path = normalized(workdir);
    let (branch, head_short_oid) = head_of(repo);

    LinkedWorktree {
        name: directory_name(&path),
        is_current: current == Some(path.as_path()),
        path,
        branch,
        head_short_oid,
        is_main: true,
        is_locked: false,
        lock_reason: None,
        status: status_of(repo),
    }
}

/// One entry of `main.worktrees()`, opened through the worktree API.
fn describe_linked(main: &Repository, name: &str, current: Option<&Path>) -> LinkedWorktree {
    let mut worktree = LinkedWorktree {
        name: name.to_string(),
        path: PathBuf::new(),
        branch: None,
        head_short_oid: String::new(),
        is_main: false,
        is_current: false,
        is_locked: false,
        lock_reason: None,
        status: LinkedWorktreeStatus::Missing,
    };

    let handle = match main.find_worktree(name) {
        Ok(handle) => handle,
        Err(error) => {
            worktree.status = LinkedWorktreeStatus::Unavailable(error.message().to_string());
            return worktree;
        }
    };

    worktree.path = normalized(handle.path());
    worktree.is_current = current == Some(worktree.path.as_path());

    match handle.is_locked() {
        Ok(WorktreeLockStatus::Locked(reason)) => {
            worktree.is_locked = true;
            worktree.lock_reason = reason;
        }
        Ok(WorktreeLockStatus::Unlocked) => {}
        Err(error) => {
            worktree.status = LinkedWorktreeStatus::Unavailable(error.message().to_string());
            return worktree;
        }
    }

    // A worktree whose directory was removed externally is still listed by git,
    // but opening it would fail; report it as missing instead of unavailable so
    // the panel can say something useful about it.
    if handle.validate().is_err() {
        worktree.status = LinkedWorktreeStatus::Missing;
        return worktree;
    }

    match Repository::open_from_worktree(&handle) {
        Ok(opened) => {
            let (branch, head_short_oid) = head_of(&opened);
            worktree.branch = branch;
            worktree.head_short_oid = head_short_oid;
            worktree.status = status_of(&opened);
        }
        Err(error) => {
            worktree.status = LinkedWorktreeStatus::Unavailable(error.message().to_string());
        }
    }

    worktree
}

/// HEAD's branch shorthand (`None` when detached) and its abbreviated id.
fn head_of(repo: &Repository) -> (Option<String>, String) {
    let Ok(head) = repo.head() else {
        // Unborn HEAD: the symbolic target still names the branch that a first
        // commit would create.
        let branch = repo
            .find_reference("HEAD")
            .ok()
            .and_then(|reference| reference.symbolic_target().map(str::to_string))
            .and_then(|target| target.strip_prefix("refs/heads/").map(str::to_string));
        return (branch, String::new());
    };

    let branch = head
        .is_branch()
        .then(|| head.shorthand().map(str::to_string))
        .flatten();
    let short_oid = head
        .peel_to_commit()
        .ok()
        .and_then(|commit| commit.as_object().short_id().ok())
        .and_then(|buf| buf.as_str().map(str::to_string))
        .unwrap_or_default();

    (branch, short_oid)
}

/// The worktree that currently has `branch_name` checked out, if any.
///
/// libgit2 has `git_branch_is_checked_out` for this, but the pinned
/// `libgit2-sys` does not expose it, so the HEAD of the main tree and of every
/// linked worktree is compared instead. `git_worktree_add` enforces the rule on
/// its own regardless; this only exists so the failure can be reported before
/// the user commits to it, naming the worktree that holds the branch.
fn checked_out_in(main: &Repository, branch_name: &str) -> Option<String> {
    if head_of(main).0.as_deref() == Some(branch_name) {
        return main
            .workdir()
            .map(|workdir| directory_name(&normalized(workdir)));
    }

    for name in main.worktrees().ok()?.iter().flatten() {
        let Ok(handle) = main.find_worktree(name) else {
            continue;
        };
        if handle.validate().is_err() {
            continue;
        }
        let Ok(opened) = Repository::open_from_worktree(&handle) else {
            continue;
        };
        if head_of(&opened).0.as_deref() == Some(branch_name) {
            return Some(name.to_string());
        }
    }

    None
}

/// Count what a checkout would lose if it were removed.
///
/// Untracked directories are not recursed into: one entry per directory is
/// enough to say "there is untracked work here", and the whole list is rebuilt
/// on every refresh of every worktree.
fn status_of(repo: &Repository) -> LinkedWorktreeStatus {
    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(false);

    let statuses = match repo.statuses(Some(&mut options)) {
        Ok(statuses) => statuses,
        Err(error) => return LinkedWorktreeStatus::Unavailable(error.message().to_string()),
    };

    let mut modified = 0usize;
    let mut staged = 0usize;
    let mut untracked = 0usize;

    for entry in statuses.iter() {
        let status = entry.status();
        if status.intersects(
            Status::INDEX_NEW
                | Status::INDEX_MODIFIED
                | Status::INDEX_DELETED
                | Status::INDEX_RENAMED
                | Status::INDEX_TYPECHANGE,
        ) {
            staged += 1;
        }
        if status.contains(Status::WT_NEW) {
            untracked += 1;
        } else if status.intersects(
            Status::WT_MODIFIED
                | Status::WT_DELETED
                | Status::WT_TYPECHANGE
                | Status::WT_RENAMED
                | Status::CONFLICTED,
        ) {
            modified += 1;
        }
    }

    if modified == 0 && staged == 0 && untracked == 0 {
        LinkedWorktreeStatus::Clean
    } else {
        LinkedWorktreeStatus::Dirty {
            modified,
            staged,
            untracked,
        }
    }
}

fn directory_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("main")
        .to_string()
}

/// The folder new worktrees are proposed inside: `<repo parent>/<repo name>-worktrees`.
///
/// Only a proposal — the dialog lets the user pick another one. Returns the
/// *parent*, never a path with the worktree name already joined on: a name is
/// appended by the dialog, and `join("")` would leave a trailing separator that
/// `Path::parent` then strips back past the folder itself.
pub fn default_worktree_parent(repo: &Repository) -> PathBuf {
    let root = main_repository(repo)
        .ok()
        .and_then(|main| main.workdir().map(normalized));

    let Some(root) = root else {
        return PathBuf::new();
    };

    let folder = format!("{}-worktrees", directory_name(&root));
    match root.parent() {
        Some(parent) => parent.join(folder),
        None => root.join(folder),
    }
}

/// User-facing reason the request cannot be carried out, or `None` when it can.
///
/// Mirrors [`super::repository::validate_new_branch_name`]: the dialog calls this
/// on every frame for live feedback, and [`add_worktree`] re-checks everything
/// anyway, because the repository can change while the dialog is open.
pub fn validate_new_worktree(repo: &Repository, request: &NewWorktreeRequest) -> Option<String> {
    let name = request.name.trim();
    let branch = request.branch.trim();
    if name.is_empty() || branch.is_empty() {
        return None;
    }

    if let Some(problem) = validate_worktree_name(name) {
        return Some(problem);
    }

    let main = match main_repository(repo) {
        Ok(main) => main,
        Err(error) => return Some(error.message().to_string()),
    };

    if let Ok(existing) = main.worktrees()
        && existing.iter().flatten().any(|taken| taken == name)
    {
        return Some(format!("A worktree named '{name}' already exists."));
    }

    if !git2::Reference::is_valid_name(&format!("refs/heads/{branch}")) {
        return Some(
            "Invalid branch name. Avoid spaces, '..', '~', '^', ':', '?', '*', '[', '\\', and leading/trailing '/' or '.'."
                .into(),
        );
    }

    if main.find_branch(branch, git2::BranchType::Local).is_ok()
        && let Some(holder) = checked_out_in(&main, branch)
    {
        return Some(format!(
            "Branch '{branch}' is already checked out in another worktree ('{holder}')."
        ));
    }

    validate_destination(&request.path).err()
}

/// Reject names git could not use as a directory under `.git/worktrees/`.
fn validate_worktree_name(name: &str) -> Option<String> {
    if name.contains('/') || name.contains('\\') {
        return Some("Worktree names cannot contain '/' or '\\'.".into());
    }
    if name == "." || name == ".." {
        return Some("'.' and '..' are not valid worktree names.".into());
    }
    None
}

/// What [`add_worktree`] should do about the destination directory.
enum Destination {
    /// Nothing is there; create the parent chain and let libgit2 make the leaf.
    Vacant,
    /// An empty directory is there; remove it first, since libgit2 creates the
    /// leaf exclusively and would fail on an existing one.
    EmptyDirectory,
}

fn validate_destination(path: &Path) -> Result<Destination, String> {
    if path.as_os_str().is_empty() {
        return Err("Choose a destination folder for the worktree.".into());
    }
    if path.parent().is_none() {
        return Err(format!(
            "Destination '{}' has no parent folder.",
            path.display()
        ));
    }

    if !path.exists() {
        return Ok(Destination::Vacant);
    }

    if !path.is_dir() {
        return Err(format!(
            "Destination '{}' already exists and is not a folder.",
            path.display()
        ));
    }

    match std::fs::read_dir(path) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                Err(format!(
                    "Destination '{}' already exists and is not empty.",
                    path.display()
                ))
            } else {
                Ok(Destination::EmptyDirectory)
            }
        }
        Err(error) => Err(format!(
            "Destination '{}' could not be read: {error}",
            path.display()
        )),
    }
}

/// Create a worktree, checking out `request.branch` in it.
///
/// The branch is created from `request.base_branch` (or HEAD) when it does not
/// exist yet, and checked out as-is when it does. Creating it here rather than
/// letting `git_worktree_add` do it is what makes a base branch possible at all:
/// left to itself, libgit2 branches from HEAD and names the branch after the
/// worktree.
///
/// Reports what was created, including the commit the checkout starts from —
/// resolved here because this is the only layer that still knows it.
pub fn add_worktree(
    repo: &Repository,
    request: &NewWorktreeRequest,
) -> Result<CreatedWorktree, git2::Error> {
    let name = request.name.trim();
    let branch_name = request.branch.trim();
    if name.is_empty() {
        return Err(git2::Error::from_str("Worktree name cannot be empty"));
    }
    if branch_name.is_empty() {
        return Err(git2::Error::from_str("Branch name cannot be empty"));
    }
    if let Some(problem) = validate_worktree_name(name) {
        return Err(git2::Error::from_str(&problem));
    }
    if !git2::Reference::is_valid_name(&format!("refs/heads/{branch_name}")) {
        return Err(git2::Error::from_str("Invalid branch name"));
    }

    let main = main_repository(repo)?;

    if main
        .worktrees()?
        .iter()
        .flatten()
        .any(|taken| taken == name)
    {
        return Err(git2::Error::from_str(&format!(
            "A worktree named '{name}' already exists."
        )));
    }

    let destination = validate_destination(&request.path).map_err(|problem| {
        // Already user-facing prose; keep it verbatim rather than wrapping it.
        git2::Error::from_str(&problem)
    })?;

    // Create the branch only if it is missing, and remember that we did so: if
    // anything after this fails, the branch has to go back, the way
    // `core::tags::service` rolls a local tag back after a failed push.
    let (created_branch, base) = match main.find_branch(branch_name, git2::BranchType::Local) {
        Ok(existing) => {
            if let Some(holder) = checked_out_in(&main, branch_name) {
                return Err(git2::Error::from_str(&format!(
                    "Branch '{branch_name}' is already checked out in another worktree ('{holder}')."
                )));
            }
            // Reusing a branch: the checkout starts at that branch's tip.
            (false, existing.into_reference().peel_to_commit()?.id())
        }
        Err(_) => {
            let base = base_commit(&main, request.base_branch.as_deref())?;
            let base_id = base.id();
            main.branch(branch_name, &base, false)?;
            (true, base_id)
        }
    };

    let result = create_checkout(&main, name, branch_name, &request.path, destination);

    if result.is_err()
        && created_branch
        && let Ok(mut branch) = main.find_branch(branch_name, git2::BranchType::Local)
    {
        let _ = branch.delete();
    }

    result.map(|()| CreatedWorktree {
        name: name.to_string(),
        branch: branch_name.to_string(),
        path: request.path.clone(),
        base_commit: base.to_string(),
    })
}

/// Prepare the destination directory and hand it to `git_worktree_add`.
fn create_checkout(
    main: &Repository,
    name: &str,
    branch_name: &str,
    path: &Path,
    destination: Destination,
) -> Result<(), git2::Error> {
    match destination {
        // libgit2 creates the leaf directory exclusively and does not create
        // parents, so both cases have to be handled here.
        Destination::Vacant => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| {
                    git2::Error::from_str(&format!(
                        "Could not create '{}': {error}",
                        parent.display()
                    ))
                })?;
            }
        }
        Destination::EmptyDirectory => {
            std::fs::remove_dir(path).map_err(|error| {
                git2::Error::from_str(&format!(
                    "Could not reuse empty folder '{}': {error}",
                    path.display()
                ))
            })?;
        }
    }

    let branch = main.find_branch(branch_name, git2::BranchType::Local)?;
    let reference = branch.into_reference();
    let mut options = WorktreeAddOptions::new();
    options.reference(Some(&reference));

    main.worktree(name, path, Some(&options)).map(|_| ())
}

/// The commit a new branch should start from.
fn base_commit<'repo>(
    repo: &'repo Repository,
    base_branch: Option<&str>,
) -> Result<git2::Commit<'repo>, git2::Error> {
    let reference = match base_branch.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => repo
            .find_branch(name, git2::BranchType::Local)
            .map_err(|_| git2::Error::from_str(&format!("Base branch '{name}' was not found.")))?
            .into_reference(),
        None => repo
            .head()
            .map_err(|_| git2::Error::from_str("Create a commit before adding a worktree."))?,
    };

    reference
        .peel_to_commit()
        .map_err(|_| git2::Error::from_str("Create a commit before adding a worktree."))
}

/// Remove a linked worktree: its metadata and, unless it is already gone, its
/// directory.
///
/// The branch it had checked out is left alone, matching `git worktree remove`.
///
/// `force` only controls whether uncommitted changes may be destroyed. The
/// application never passes `true` — a worktree holding uncommitted work has to
/// be dealt with in the worktree itself — but the flag is the seam an explicit
/// discard-and-remove flow would use.
pub fn remove_worktree(repo: &Repository, name: &str, force: bool) -> Result<(), git2::Error> {
    let main = main_repository(repo)?;
    let handle = main.find_worktree(name).map_err(|_| {
        git2::Error::from_str(&format!(
            "Worktree '{name}' was not found in this repository."
        ))
    })?;

    if let Ok(WorktreeLockStatus::Locked(reason)) = handle.is_locked() {
        let reason = reason.unwrap_or_else(|| "no reason given".into());
        return Err(git2::Error::from_str(&format!(
            "Worktree '{name}' is locked: {reason}. Unlock it before removing."
        )));
    }

    // Only inspect a checkout that is actually on disk; a missing one has
    // nothing left to lose and should stay removable.
    let exists = handle.validate().is_ok();
    if exists && !force {
        let opened = Repository::open_from_worktree(&handle)?;
        match status_of(&opened) {
            LinkedWorktreeStatus::Clean => {}
            LinkedWorktreeStatus::Dirty { .. } => {
                return Err(git2::Error::from_str(&format!(
                    "Cannot remove worktree '{name}' because it contains uncommitted changes. Open it and commit or discard them first."
                )));
            }
            LinkedWorktreeStatus::Missing => {}
            LinkedWorktreeStatus::Unavailable(detail) => {
                return Err(git2::Error::from_str(&format!(
                    "Cannot remove worktree '{name}' because its status could not be read: {detail}"
                )));
            }
        }
    }

    let mut options = git2::WorktreePruneOptions::new();
    // `valid` lets a worktree that still exists be pruned at all; `working_tree`
    // deletes the directory. `locked` is deliberately left off, so the guard
    // above is not the only thing standing between a lock and a deletion.
    options.valid(true).working_tree(true);
    handle.prune(Some(&mut options))
}

#[cfg(test)]
mod tests {
    use super::{
        add_worktree, default_worktree_parent, list_worktrees, remove_worktree,
        validate_new_worktree,
    };
    use crate::shared::worktrees::{LinkedWorktreeStatus, NewWorktreeRequest};
    use crate::testutil::{TestRepoDir, commit_all};
    use git2::Repository;
    use std::path::{Path, PathBuf};

    /// A repository with one commit on `main`, plus a separate directory to put
    /// worktrees in. Both are removed when the returned pair is dropped.
    fn repo_with_commit() -> (TestRepoDir, TestRepoDir) {
        let repo_dir = TestRepoDir::init();
        repo_dir.write("README.md", "hello\n");
        let repo = repo_dir.open();
        commit_all(&repo, "initial");
        (repo_dir, TestRepoDir::empty())
    }

    fn request(name: &str, branch: &str, path: PathBuf) -> NewWorktreeRequest {
        NewWorktreeRequest {
            name: name.into(),
            branch: branch.into(),
            base_branch: None,
            path,
        }
    }

    #[test]
    fn fresh_repository_lists_only_its_main_worktree() {
        let (repo_dir, _worktrees) = repo_with_commit();
        let repo = repo_dir.open();

        let worktrees = list_worktrees(&repo).expect("list worktrees");

        assert_eq!(worktrees.len(), 1);
        assert!(worktrees[0].is_main);
        assert!(worktrees[0].is_current);
        assert_eq!(worktrees[0].branch.as_deref(), Some("main"));
        assert_eq!(worktrees[0].status, LinkedWorktreeStatus::Clean);
    }

    #[test]
    fn adding_a_worktree_creates_the_branch_and_the_checkout() {
        let (repo_dir, worktrees_dir) = repo_with_commit();
        let repo = repo_dir.open();
        let destination = worktrees_dir.path().join("feature-auth");

        let created = add_worktree(
            &repo,
            &request("feature-auth", "feature/auth", destination.clone()),
        )
        .expect("add worktree");

        assert_eq!(created.path, destination);
        assert_eq!(created.name, "feature-auth");
        assert_eq!(created.branch, "feature/auth");
        assert!(destination.join("README.md").is_file());
        assert!(
            repo.find_branch("feature/auth", git2::BranchType::Local)
                .is_ok()
        );

        let worktrees = list_worktrees(&repo).expect("list worktrees");
        assert_eq!(worktrees.len(), 2);
        let linked = worktrees
            .iter()
            .find(|worktree| !worktree.is_main)
            .expect("linked worktree");
        assert_eq!(linked.name, "feature-auth");
        assert_eq!(linked.branch.as_deref(), Some("feature/auth"));
        assert_eq!(linked.path, destination.canonicalize().expect("canonical"));
        assert_eq!(linked.status, LinkedWorktreeStatus::Clean);
        assert!(!linked.is_current);
    }

    #[test]
    fn a_worktree_opens_as_its_own_repository() {
        let (repo_dir, worktrees_dir) = repo_with_commit();
        let repo = repo_dir.open();
        let destination = worktrees_dir.path().join("feature-auth");
        add_worktree(
            &repo,
            &request("feature-auth", "feature/auth", destination.clone()),
        )
        .expect("add worktree");

        let opened = Repository::discover(&destination).expect("discover worktree");

        assert!(opened.is_worktree());
        assert_eq!(
            opened
                .workdir()
                .map(|workdir| workdir.canonicalize().expect("canonical")),
            Some(destination.canonicalize().expect("canonical"))
        );
        assert_eq!(
            opened.head().expect("head").shorthand(),
            Some("feature/auth")
        );
    }

    #[test]
    fn listing_from_a_secondary_worktree_sees_the_whole_repository() {
        let (repo_dir, worktrees_dir) = repo_with_commit();
        let repo = repo_dir.open();
        let destination = worktrees_dir.path().join("feature-auth");
        add_worktree(
            &repo,
            &request("feature-auth", "feature/auth", destination.clone()),
        )
        .expect("add worktree");

        let from_worktree = Repository::discover(&destination).expect("discover worktree");
        let worktrees = list_worktrees(&from_worktree).expect("list from worktree");

        assert_eq!(worktrees.len(), 2);
        assert!(worktrees.iter().any(|worktree| worktree.is_main));
        // The worktree we opened from is the current one, and the main tree is not.
        let current: Vec<&str> = worktrees
            .iter()
            .filter(|worktree| worktree.is_current)
            .map(|worktree| worktree.name.as_str())
            .collect();
        assert_eq!(current, vec!["feature-auth"]);
    }

    #[test]
    fn a_new_branch_starts_from_the_requested_base() {
        let (repo_dir, worktrees_dir) = repo_with_commit();
        let repo = repo_dir.open();
        let base_oid = repo.head().expect("head").target().expect("target");
        repo_dir.write("second.txt", "second\n");
        commit_all(&repo, "second");

        let mut req = request(
            "from-base",
            "feature/from-base",
            worktrees_dir.path().join("from-base"),
        );
        req.base_branch = Some("main".into());
        // `main` has moved on, so name the base explicitly by re-pointing a branch
        // at the first commit and branching from that instead.
        let base_commit = repo.find_commit(base_oid).expect("base commit");
        repo.branch("release", &base_commit, false).expect("branch");
        req.base_branch = Some("release".into());

        add_worktree(&repo, &req).expect("add worktree");

        let created = repo
            .find_branch("feature/from-base", git2::BranchType::Local)
            .expect("created branch");
        assert_eq!(
            created.get().peel_to_commit().expect("commit").id(),
            base_oid
        );
    }

    #[test]
    fn an_existing_free_branch_is_checked_out_instead_of_created() {
        let (repo_dir, worktrees_dir) = repo_with_commit();
        let repo = repo_dir.open();
        let head = repo.head().expect("head").peel_to_commit().expect("commit");
        repo.branch("existing", &head, false).expect("branch");

        add_worktree(
            &repo,
            &request("reuse", "existing", worktrees_dir.path().join("reuse")),
        )
        .expect("add worktree");

        let worktrees = list_worktrees(&repo).expect("list worktrees");
        let linked = worktrees
            .iter()
            .find(|worktree| worktree.name == "reuse")
            .expect("linked worktree");
        assert_eq!(linked.branch.as_deref(), Some("existing"));
    }

    #[test]
    fn a_branch_checked_out_elsewhere_is_refused() {
        let (repo_dir, worktrees_dir) = repo_with_commit();
        let repo = repo_dir.open();

        let error = add_worktree(
            &repo,
            &request("second-main", "main", worktrees_dir.path().join("second")),
        )
        .expect_err("main is checked out in the main worktree");

        assert!(
            error.message().contains("already checked out"),
            "unexpected message: {}",
            error.message()
        );
        assert!(!worktrees_dir.path().join("second").exists());
    }

    #[test]
    fn a_duplicate_worktree_name_is_refused() {
        let (repo_dir, worktrees_dir) = repo_with_commit();
        let repo = repo_dir.open();
        add_worktree(
            &repo,
            &request("dup", "feature/one", worktrees_dir.path().join("one")),
        )
        .expect("add worktree");

        let error = add_worktree(
            &repo,
            &request("dup", "feature/two", worktrees_dir.path().join("two")),
        )
        .expect_err("duplicate name");

        assert!(error.message().contains("already exists"));
        // The rejected attempt must not leave its branch behind.
        assert!(
            repo.find_branch("feature/two", git2::BranchType::Local)
                .is_err()
        );
    }

    #[test]
    fn a_non_empty_destination_is_refused() {
        let (repo_dir, worktrees_dir) = repo_with_commit();
        let repo = repo_dir.open();
        let destination = worktrees_dir.path().join("occupied");
        std::fs::create_dir_all(&destination).expect("create destination");
        std::fs::write(destination.join("keep.txt"), "keep").expect("write");

        let error = add_worktree(&repo, &request("occupied", "feature/x", destination))
            .expect_err("non-empty destination");

        assert!(
            error.message().contains("is not empty"),
            "unexpected message: {}",
            error.message()
        );
        assert!(
            repo.find_branch("feature/x", git2::BranchType::Local)
                .is_err(),
            "a rejected add must not leave a branch behind"
        );
    }

    #[test]
    fn an_empty_destination_directory_is_reused() {
        let (repo_dir, worktrees_dir) = repo_with_commit();
        let repo = repo_dir.open();
        let destination = worktrees_dir.path().join("empty");
        std::fs::create_dir_all(&destination).expect("create destination");

        add_worktree(
            &repo,
            &request("empty", "feature/empty", destination.clone()),
        )
        .expect("add worktree into an empty folder");

        assert!(destination.join("README.md").is_file());
    }

    #[test]
    fn dirty_worktrees_report_their_counts() {
        let (repo_dir, worktrees_dir) = repo_with_commit();
        let repo = repo_dir.open();
        let destination = worktrees_dir.path().join("dirty");
        add_worktree(
            &repo,
            &request("dirty", "feature/dirty", destination.clone()),
        )
        .expect("add worktree");

        std::fs::write(destination.join("README.md"), "changed\n").expect("modify");
        std::fs::write(destination.join("new.txt"), "new\n").expect("add untracked");

        let worktrees = list_worktrees(&repo).expect("list worktrees");
        let linked = worktrees
            .iter()
            .find(|worktree| worktree.name == "dirty")
            .expect("linked worktree");

        assert_eq!(
            linked.status,
            LinkedWorktreeStatus::Dirty {
                modified: 1,
                staged: 0,
                untracked: 1,
            }
        );
    }

    #[test]
    fn removing_a_clean_worktree_deletes_it() {
        let (repo_dir, worktrees_dir) = repo_with_commit();
        let repo = repo_dir.open();
        let destination = worktrees_dir.path().join("clean");
        add_worktree(
            &repo,
            &request("clean", "feature/clean", destination.clone()),
        )
        .expect("add worktree");

        remove_worktree(&repo, "clean", false).expect("remove clean worktree");

        assert!(!destination.exists());
        assert_eq!(list_worktrees(&repo).expect("list worktrees").len(), 1);
        // Removing a worktree leaves its branch alone, like `git worktree remove`.
        assert!(
            repo.find_branch("feature/clean", git2::BranchType::Local)
                .is_ok()
        );
    }

    #[test]
    fn removing_a_dirty_worktree_is_refused_and_changes_survive() {
        let (repo_dir, worktrees_dir) = repo_with_commit();
        let repo = repo_dir.open();
        let destination = worktrees_dir.path().join("dirty");
        add_worktree(
            &repo,
            &request("dirty", "feature/dirty", destination.clone()),
        )
        .expect("add worktree");
        std::fs::write(destination.join("README.md"), "unsaved work\n").expect("modify");

        let error =
            remove_worktree(&repo, "dirty", false).expect_err("dirty removal must be refused");

        assert!(
            error.message().contains("uncommitted changes"),
            "unexpected message: {}",
            error.message()
        );
        assert_eq!(
            std::fs::read_to_string(destination.join("README.md")).expect("read"),
            "unsaved work\n"
        );
        assert_eq!(list_worktrees(&repo).expect("list worktrees").len(), 2);
    }

    #[test]
    fn untracked_only_changes_also_block_removal() {
        let (repo_dir, worktrees_dir) = repo_with_commit();
        let repo = repo_dir.open();
        let destination = worktrees_dir.path().join("untracked");
        add_worktree(
            &repo,
            &request("untracked", "feature/untracked", destination.clone()),
        )
        .expect("add worktree");
        std::fs::write(destination.join("scratch.txt"), "agent output\n").expect("write");

        let error = remove_worktree(&repo, "untracked", false)
            .expect_err("untracked work must block removal");

        assert!(error.message().contains("uncommitted changes"));
        assert!(destination.join("scratch.txt").is_file());
    }

    #[test]
    fn staged_only_changes_also_block_removal() {
        let (repo_dir, worktrees_dir) = repo_with_commit();
        let repo = repo_dir.open();
        let destination = worktrees_dir.path().join("staged");
        add_worktree(
            &repo,
            &request("staged", "feature/staged", destination.clone()),
        )
        .expect("add worktree");
        std::fs::write(destination.join("README.md"), "staged work\n").expect("modify");
        {
            let opened = Repository::discover(&destination).expect("discover");
            let mut index = opened.index().expect("index");
            index.add_path(Path::new("README.md")).expect("stage");
            index.write().expect("write index");
        }

        let error =
            remove_worktree(&repo, "staged", false).expect_err("staged work must block removal");

        assert!(error.message().contains("uncommitted changes"));
    }

    #[test]
    fn force_removal_still_works_for_a_dirty_worktree() {
        let (repo_dir, worktrees_dir) = repo_with_commit();
        let repo = repo_dir.open();
        let destination = worktrees_dir.path().join("forced");
        add_worktree(
            &repo,
            &request("forced", "feature/forced", destination.clone()),
        )
        .expect("add worktree");
        std::fs::write(destination.join("README.md"), "discarded\n").expect("modify");

        remove_worktree(&repo, "forced", true).expect("forced removal");

        assert!(!destination.exists());
        assert_eq!(list_worktrees(&repo).expect("list worktrees").len(), 1);
    }

    #[test]
    fn removing_an_unknown_worktree_names_it() {
        let (repo_dir, _worktrees) = repo_with_commit();
        let repo = repo_dir.open();

        let error = remove_worktree(&repo, "nope", false).expect_err("unknown worktree");

        assert!(error.message().contains("'nope'"));
    }

    #[test]
    fn a_worktree_whose_directory_vanished_is_reported_as_missing() {
        let (repo_dir, worktrees_dir) = repo_with_commit();
        let repo = repo_dir.open();
        let destination = worktrees_dir.path().join("gone");
        add_worktree(&repo, &request("gone", "feature/gone", destination.clone()))
            .expect("add worktree");
        std::fs::remove_dir_all(&destination).expect("remove directory");

        let worktrees = list_worktrees(&repo).expect("list worktrees");
        let linked = worktrees
            .iter()
            .find(|worktree| worktree.name == "gone")
            .expect("linked worktree");

        assert_eq!(linked.status, LinkedWorktreeStatus::Missing);
        // A missing checkout has nothing left to protect, so it stays removable.
        remove_worktree(&repo, "gone", false).expect("remove missing worktree");
        assert_eq!(list_worktrees(&repo).expect("list worktrees").len(), 1);
    }

    #[test]
    fn validation_rejects_bad_names_duplicates_and_busy_branches() {
        let (repo_dir, worktrees_dir) = repo_with_commit();
        let repo = repo_dir.open();
        add_worktree(
            &repo,
            &request("taken", "feature/taken", worktrees_dir.path().join("taken")),
        )
        .expect("add worktree");

        let vacant = worktrees_dir.path().join("fresh");

        // Incomplete input says nothing yet, so the dialog stays quiet while typing.
        assert!(validate_new_worktree(&repo, &request("", "", vacant.clone())).is_none());
        assert!(
            validate_new_worktree(&repo, &request("ok", "feature/ok", vacant.clone())).is_none()
        );

        assert_eq!(
            validate_new_worktree(&repo, &request("a/b", "feature/ok", vacant.clone())),
            Some("Worktree names cannot contain '/' or '\\'.".into())
        );
        assert_eq!(
            validate_new_worktree(&repo, &request("taken", "feature/ok", vacant.clone())),
            Some("A worktree named 'taken' already exists.".into())
        );
        assert!(
            validate_new_worktree(&repo, &request("ok", "main", vacant.clone()))
                .is_some_and(|problem| problem
                    .starts_with("Branch 'main' is already checked out in another worktree")),
        );
        assert!(
            validate_new_worktree(&repo, &request("ok", "has space", vacant))
                .is_some_and(|problem| problem.starts_with("Invalid branch name"))
        );
        assert!(
            validate_new_worktree(
                &repo,
                &request("ok", "feature/ok", worktrees_dir.path().join("taken"))
            )
            .is_some_and(|problem| problem.contains("is not empty"))
        );
    }

    #[test]
    fn the_proposed_parent_is_a_sibling_worktrees_folder() {
        let (repo_dir, _worktrees) = repo_with_commit();
        let repo = repo_dir.open();

        let parent = default_worktree_parent(&repo);

        let root = repo
            .workdir()
            .expect("workdir")
            .canonicalize()
            .expect("canonical");
        let repo_name = root
            .file_name()
            .expect("name")
            .to_string_lossy()
            .to_string();
        assert_eq!(
            parent,
            root.parent()
                .expect("parent")
                .join(format!("{repo_name}-worktrees"))
        );
        // The name is appended by the caller, so the folder must survive the join.
        assert_eq!(
            parent.join("feature-auth"),
            root.parent()
                .expect("parent")
                .join(format!("{repo_name}-worktrees"))
                .join("feature-auth")
        );
    }
}
