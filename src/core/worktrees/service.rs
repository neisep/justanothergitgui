//! Worktree workflow rules.
//!
//! Everything here is policy about *which* worktree operations are allowed; the
//! git calls themselves live behind [`GitLinkedWorktreePort`]. Listing has no
//! rules to apply, so it stays a plain read through `AppRepoRead` like every
//! other synchronous repository read; only the two mutations come through here.
//!
//! The dialog validates the same things while the user types, but the repository
//! can change between opening a dialog and confirming it, so the checks are
//! repeated here — the same reason the file-action handler re-validates in infra.

use std::path::Path;

use crate::core::ports::GitLinkedWorktreePort;
use crate::shared::worktrees::{LinkedWorktree, NewWorktreeRequest};

/// Phase 1 never destroys uncommitted work: a worktree holding changes has to be
/// opened and dealt with explicitly. Agent runs may exist only as uncommitted
/// changes, so the manager protects them by default.
///
/// The port still takes the flag, so an explicit discard-and-remove flow is a
/// change of this constant plus a confirmation step, not a change of shape.
const ALLOW_DESTROYING_UNCOMMITTED_WORK: bool = false;

/// Create a worktree and report what was created.
pub fn create(
    repo_path: &Path,
    request: &NewWorktreeRequest,
    git: &impl GitLinkedWorktreePort,
) -> Result<String, String> {
    let name = request.name.trim();
    let branch = request.branch.trim();

    if name.is_empty() {
        return Err("Enter a name for the worktree.".into());
    }
    if branch.is_empty() {
        return Err("Enter a branch for the worktree.".into());
    }
    if request.path.as_os_str().is_empty() {
        return Err("Choose a destination folder for the worktree.".into());
    }

    let existing = git.list_worktrees(repo_path)?;

    if existing.iter().any(|worktree| worktree.name == name) {
        return Err(format!("A worktree named '{name}' already exists."));
    }

    if let Some(holder) = existing
        .iter()
        .find(|worktree| worktree.branch.as_deref() == Some(branch))
    {
        return Err(format!(
            "Branch '{branch}' is already checked out in another worktree ('{}').",
            holder.name
        ));
    }

    let path = git.add_worktree(repo_path, request)?;

    Ok(format!(
        "Created worktree '{name}' on '{branch}' at {}",
        path.display()
    ))
}

/// Remove a linked worktree, refusing anything that would lose work.
pub fn remove(
    repo_path: &Path,
    name: &str,
    git: &impl GitLinkedWorktreePort,
) -> Result<String, String> {
    let existing = git.list_worktrees(repo_path)?;

    let worktree = existing
        .iter()
        .find(|worktree| worktree.name == name)
        .ok_or_else(|| format!("Worktree '{name}' was not found in this repository."))?;

    if let Some(problem) = removal_blocker(worktree) {
        return Err(problem);
    }

    git.remove_worktree(repo_path, name, ALLOW_DESTROYING_UNCOMMITTED_WORK)?;

    Ok(format!("Removed worktree '{name}'"))
}

/// Why this worktree may not be removed, or `None` when it may.
///
/// Shared with the UI so the confirmation dialog can disable its button for the
/// same reason the service would refuse, instead of guessing at the rule.
pub fn removal_blocker(worktree: &LinkedWorktree) -> Option<String> {
    if worktree.is_main {
        return Some(format!(
            "'{}' is the repository's main worktree and cannot be removed.",
            worktree.name
        ));
    }

    if worktree.is_locked {
        let reason = worktree
            .lock_reason
            .clone()
            .unwrap_or_else(|| "no reason given".into());
        return Some(format!(
            "Worktree '{}' is locked: {reason}. Unlock it before removing.",
            worktree.name
        ));
    }

    if worktree.has_uncommitted_changes() {
        return Some(format!(
            "Worktree '{}' has uncommitted changes. Open it and commit or discard them first.",
            worktree.name
        ));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::worktrees::LinkedWorktreeStatus;
    use std::cell::RefCell;
    use std::path::PathBuf;

    fn path() -> &'static Path {
        Path::new("/virtual/repo")
    }

    fn worktree(name: &str, is_main: bool, status: LinkedWorktreeStatus) -> LinkedWorktree {
        LinkedWorktree {
            name: name.into(),
            path: PathBuf::from("/virtual").join(name),
            branch: Some(format!("feature/{name}")),
            head_short_oid: "abc1234".into(),
            is_main,
            is_current: false,
            is_locked: false,
            lock_reason: None,
            status,
        }
    }

    #[derive(Default)]
    struct FakeWorktreeGit {
        worktrees: Vec<LinkedWorktree>,
        add_calls: RefCell<Vec<NewWorktreeRequest>>,
        remove_calls: RefCell<Vec<(String, bool)>>,
        add_error: Option<String>,
    }

    impl GitLinkedWorktreePort for FakeWorktreeGit {
        fn list_worktrees(&self, _repo_path: &Path) -> Result<Vec<LinkedWorktree>, String> {
            Ok(self.worktrees.clone())
        }

        fn add_worktree(
            &self,
            _repo_path: &Path,
            request: &NewWorktreeRequest,
        ) -> Result<PathBuf, String> {
            self.add_calls.borrow_mut().push(request.clone());
            match &self.add_error {
                Some(error) => Err(error.clone()),
                None => Ok(request.path.clone()),
            }
        }

        fn remove_worktree(
            &self,
            _repo_path: &Path,
            name: &str,
            force: bool,
        ) -> Result<(), String> {
            self.remove_calls
                .borrow_mut()
                .push((name.to_string(), force));
            Ok(())
        }
    }

    fn request(name: &str, branch: &str) -> NewWorktreeRequest {
        NewWorktreeRequest {
            name: name.into(),
            branch: branch.into(),
            base_branch: Some("main".into()),
            path: PathBuf::from("/virtual/worktrees").join(name),
        }
    }

    #[test]
    fn creating_reports_the_name_branch_and_path() {
        let git = FakeWorktreeGit::default();

        let message = create(path(), &request("feature-auth", "feature/auth"), &git)
            .expect("create worktree");

        assert_eq!(
            message,
            "Created worktree 'feature-auth' on 'feature/auth' at /virtual/worktrees/feature-auth"
        );
        assert_eq!(git.add_calls.borrow().len(), 1);
    }

    #[test]
    fn a_duplicate_name_is_refused_before_reaching_git() {
        let git = FakeWorktreeGit {
            worktrees: vec![worktree("taken", false, LinkedWorktreeStatus::Clean)],
            ..FakeWorktreeGit::default()
        };

        let error = create(path(), &request("taken", "feature/new"), &git).expect_err("duplicate");

        assert_eq!(error, "A worktree named 'taken' already exists.");
        assert!(git.add_calls.borrow().is_empty());
    }

    #[test]
    fn a_branch_checked_out_elsewhere_is_refused_before_reaching_git() {
        let git = FakeWorktreeGit {
            worktrees: vec![worktree("busy", false, LinkedWorktreeStatus::Clean)],
            ..FakeWorktreeGit::default()
        };

        let error =
            create(path(), &request("another", "feature/busy"), &git).expect_err("branch in use");

        assert_eq!(
            error,
            "Branch 'feature/busy' is already checked out in another worktree ('busy')."
        );
        assert!(git.add_calls.borrow().is_empty());
    }

    #[test]
    fn incomplete_requests_are_refused_before_reaching_git() {
        let git = FakeWorktreeGit::default();

        assert_eq!(
            create(path(), &request("", "feature/x"), &git).expect_err("no name"),
            "Enter a name for the worktree."
        );
        assert_eq!(
            create(path(), &request("x", ""), &git).expect_err("no branch"),
            "Enter a branch for the worktree."
        );
        let mut no_path = request("x", "feature/x");
        no_path.path = PathBuf::new();
        assert_eq!(
            create(path(), &no_path, &git).expect_err("no path"),
            "Choose a destination folder for the worktree."
        );
        assert!(git.add_calls.borrow().is_empty());
    }

    #[test]
    fn removing_a_clean_worktree_never_forces() {
        let git = FakeWorktreeGit {
            worktrees: vec![worktree("clean", false, LinkedWorktreeStatus::Clean)],
            ..FakeWorktreeGit::default()
        };

        let message = remove(path(), "clean", &git).expect("remove");

        assert_eq!(message, "Removed worktree 'clean'");
        assert_eq!(
            git.remove_calls.borrow().as_slice(),
            [("clean".to_string(), false)]
        );
    }

    #[test]
    fn the_main_worktree_can_never_be_removed() {
        let git = FakeWorktreeGit {
            worktrees: vec![worktree("myapp", true, LinkedWorktreeStatus::Clean)],
            ..FakeWorktreeGit::default()
        };

        let error = remove(path(), "myapp", &git).expect_err("main worktree");

        assert_eq!(
            error,
            "'myapp' is the repository's main worktree and cannot be removed."
        );
        assert!(git.remove_calls.borrow().is_empty());
    }

    #[test]
    fn uncommitted_work_blocks_removal_and_is_never_forced_away() {
        let git = FakeWorktreeGit {
            worktrees: vec![worktree(
                "dirty",
                false,
                LinkedWorktreeStatus::Dirty {
                    modified: 4,
                    staged: 0,
                    untracked: 0,
                },
            )],
            ..FakeWorktreeGit::default()
        };

        let error = remove(path(), "dirty", &git).expect_err("dirty worktree");

        assert_eq!(
            error,
            "Worktree 'dirty' has uncommitted changes. Open it and commit or discard them first."
        );
        assert!(git.remove_calls.borrow().is_empty());
    }

    #[test]
    fn a_locked_worktree_reports_its_reason() {
        let mut locked = worktree("locked", false, LinkedWorktreeStatus::Clean);
        locked.is_locked = true;
        locked.lock_reason = Some("release build".into());
        let git = FakeWorktreeGit {
            worktrees: vec![locked],
            ..FakeWorktreeGit::default()
        };

        let error = remove(path(), "locked", &git).expect_err("locked worktree");

        assert_eq!(
            error,
            "Worktree 'locked' is locked: release build. Unlock it before removing."
        );
        assert!(git.remove_calls.borrow().is_empty());
    }

    #[test]
    fn an_unknown_worktree_is_named_in_the_error() {
        let git = FakeWorktreeGit::default();

        let error = remove(path(), "nope", &git).expect_err("unknown worktree");

        assert_eq!(error, "Worktree 'nope' was not found in this repository.");
    }

    #[test]
    fn a_worktree_missing_from_disk_stays_removable() {
        let git = FakeWorktreeGit {
            worktrees: vec![worktree("gone", false, LinkedWorktreeStatus::Missing)],
            ..FakeWorktreeGit::default()
        };

        assert!(removal_blocker(&git.worktrees[0]).is_none());
        remove(path(), "gone", &git).expect("remove missing worktree");
    }
}
