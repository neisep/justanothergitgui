//! Cross-layer model for git's *linked* worktrees (`git worktree`).
//!
//! Deliberately distinct from [`crate::state::WorktreeState`] and
//! [`crate::infra::git::worktree`], which are about *the* working tree — the
//! staged/unstaged file lists of one checkout. This module is about the set of
//! checkouts that share one object store.
//!
//! Phase 1 carries only what git itself knows. Per-worktree application metadata
//! (task, agent, base commit, review state, test state) is meant to be joined in
//! later by [`LinkedWorktree::name`], which is git's own stable handle for a
//! worktree, so nothing here has to change to make room for it.

use std::path::PathBuf;

/// One checkout belonging to a repository: either the main working tree or a
/// linked worktree under `.git/worktrees/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedWorktree {
    /// Git's handle for the worktree — the name `find_worktree` takes. For the
    /// main worktree, which git does not name, the directory name stands in.
    pub name: String,
    /// Top level of the checkout, not the `.git` file inside it.
    pub path: PathBuf,
    /// `None` when HEAD is detached; [`Self::head_short_oid`] identifies it then.
    pub branch: Option<String>,
    pub head_short_oid: String,
    pub is_main: bool,
    /// Whether this is the checkout the current tab has open.
    pub is_current: bool,
    pub is_locked: bool,
    pub lock_reason: Option<String>,
    pub status: LinkedWorktreeStatus,
}

impl LinkedWorktree {
    /// Whether the checkout holds work that removing it would destroy.
    pub fn has_uncommitted_changes(&self) -> bool {
        matches!(self.status, LinkedWorktreeStatus::Dirty { .. })
    }

    /// What the branch column shows: the branch name, or the detached head.
    pub fn branch_label(&self) -> String {
        match &self.branch {
            Some(branch) => branch.clone(),
            None => format!("detached at {}", self.head_short_oid),
        }
    }
}

/// How much uncommitted work a checkout is holding, or why that is unknown.
///
/// `Missing` and `Unavailable` are kept apart from `Clean` on purpose: a
/// worktree whose directory was deleted externally reports no changed files, and
/// calling that "clean" would invite removing it as if that were safe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkedWorktreeStatus {
    Clean,
    Dirty {
        modified: usize,
        staged: usize,
        untracked: usize,
    },
    /// The metadata is there but the checkout is not, so git would prune it.
    Missing,
    /// The checkout could not be inspected; the string says why.
    Unavailable(String),
}

impl LinkedWorktreeStatus {
    /// One-line summary for the worktree row.
    pub fn summary(&self) -> String {
        match self {
            Self::Clean => "Clean".into(),
            Self::Dirty {
                modified,
                staged,
                untracked,
            } => {
                let parts = count_labels(*modified, *staged, *untracked);
                if parts.is_empty() {
                    "Clean".into()
                } else {
                    parts.join(", ")
                }
            }
            Self::Missing => "Missing from disk".into(),
            Self::Unavailable(_) => "Status unavailable".into(),
        }
    }

    /// The bullet list a removal confirmation shows. Empty when there is nothing
    /// at risk.
    pub fn damage_lines(&self) -> Vec<String> {
        match self {
            Self::Dirty {
                modified,
                staged,
                untracked,
            } => count_labels(*modified, *staged, *untracked),
            _ => Vec::new(),
        }
    }
}

/// `"4 modified files"`-style fragments, skipping the zero counts.
fn count_labels(modified: usize, staged: usize, untracked: usize) -> Vec<String> {
    [
        (modified, "modified"),
        (staged, "staged"),
        (untracked, "untracked"),
    ]
    .into_iter()
    .filter(|(count, _)| *count > 0)
    .map(|(count, label)| format!("{count} {label} file{}", if count == 1 { "" } else { "s" }))
    .collect()
}

/// What the New Worktree dialog collected, before any of it has been validated
/// against the repository.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NewWorktreeRequest {
    /// Directory name under `.git/worktrees/`; may not contain path separators.
    pub name: String,
    /// Branch to check out in the new worktree, created if it does not exist.
    pub branch: String,
    /// Branch the new branch starts from. `None` means the current HEAD.
    pub base_branch: Option<String>,
    pub path: PathBuf,
}

/// What the app learned by creating a worktree.
///
/// Carries the base commit out of the git layer because nothing above it can
/// recover the value afterwards: by the time the background task's result is
/// applied the refreshed worktree list does not exist yet, and a commit the user
/// typed by hand would be worthless. Per-worktree metadata records it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatedWorktree {
    pub name: String,
    pub branch: String,
    pub path: PathBuf,
    /// Full oid the new checkout starts from.
    pub base_commit: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_names_every_non_zero_count() {
        let status = LinkedWorktreeStatus::Dirty {
            modified: 4,
            staged: 0,
            untracked: 1,
        };
        assert_eq!(status.summary(), "4 modified files, 1 untracked file");
        assert_eq!(
            status.damage_lines(),
            vec![
                "4 modified files".to_string(),
                "1 untracked file".to_string()
            ]
        );
    }

    #[test]
    fn clean_and_missing_never_report_damage() {
        assert_eq!(LinkedWorktreeStatus::Clean.summary(), "Clean");
        assert!(LinkedWorktreeStatus::Clean.damage_lines().is_empty());
        assert_eq!(LinkedWorktreeStatus::Missing.summary(), "Missing from disk");
        assert!(LinkedWorktreeStatus::Missing.damage_lines().is_empty());
    }

    #[test]
    fn detached_head_falls_back_to_the_short_oid() {
        let worktree = LinkedWorktree {
            name: "wt".into(),
            path: PathBuf::from("/tmp/wt"),
            branch: None,
            head_short_oid: "abc1234".into(),
            is_main: false,
            is_current: false,
            is_locked: false,
            lock_reason: None,
            status: LinkedWorktreeStatus::Clean,
        };
        assert_eq!(worktree.branch_label(), "detached at abc1234");
        assert!(!worktree.has_uncommitted_changes());
    }
}
