//! What a worktree has done since its base commit.
//!
//! Read-only, like [`super::commits`], and it borrows that module's delta and
//! patch helpers so there is one place that knows how a `git2::Diff` becomes a
//! file list or patch text.
//!
//! Two things make this different from a commit diff:
//!
//! * The "to" side is the worktree **as it stands on disk** — committed, staged
//!   and unstaged together. Work done in a worktree often has not been committed
//!   yet, and that is exactly the work a reviewer needs to see.
//! * The "from" side has to be found. The app records a base commit for the
//!   worktrees it creates; anything made with `git worktree add` in a terminal
//!   has none, so the fork point is derived instead.

use git2::Repository;

use crate::shared::git::CommitFileChange;
use crate::shared::review::{ReviewBase, ReviewBaseSource, ReviewSummary};

use super::commits;
use super::linked_worktrees;

/// The commit this worktree's work should be measured from.
///
/// A recorded base wins when it still resolves; history can be rewritten under
/// it, so an oid that no longer exists falls through to the fork point rather
/// than failing the review.
pub fn resolve_base(
    worktree_repo: &Repository,
    recorded: Option<&str>,
) -> Result<ReviewBase, git2::Error> {
    if let Some(oid) = recorded
        .map(str::trim)
        .filter(|recorded| !recorded.is_empty())
        .and_then(|recorded| git2::Oid::from_str(recorded).ok())
        && worktree_repo.find_commit(oid).is_ok()
    {
        return Ok(base_from(worktree_repo, oid, ReviewBaseSource::Recorded));
    }

    let fork_point = fork_point(worktree_repo)?;
    Ok(base_from(
        worktree_repo,
        fork_point,
        ReviewBaseSource::ForkPoint,
    ))
}

/// Where this worktree's HEAD diverged from the main worktree's.
fn fork_point(worktree_repo: &Repository) -> Result<git2::Oid, git2::Error> {
    let head = worktree_repo
        .head()
        .and_then(|head| head.peel_to_commit())
        .map_err(|_| {
            git2::Error::from_str("This worktree has no commits to compare against yet.")
        })?;

    let main = linked_worktrees::main_repository(worktree_repo)?;
    let main_head = main
        .head()
        .and_then(|head| head.peel_to_commit())
        .map_err(|_| {
            git2::Error::from_str("The main worktree has no commits to compare against yet.")
        })?;

    // Same commit: the worktree has committed nothing of its own, so its own
    // HEAD is the base and any difference is uncommitted work.
    if head.id() == main_head.id() {
        return Ok(head.id());
    }

    worktree_repo
        .merge_base(head.id(), main_head.id())
        .map_err(|_| {
            git2::Error::from_str(
                "This worktree shares no history with the main worktree, so there is no base to compare against.",
            )
        })
}

fn base_from(repo: &Repository, oid: git2::Oid, source: ReviewBaseSource) -> ReviewBase {
    let short_oid = repo
        .find_object(oid, None)
        .ok()
        .and_then(|object| object.short_id().ok())
        .and_then(|buf| buf.as_str().map(str::to_string))
        .unwrap_or_else(|| oid.to_string());

    ReviewBase {
        oid: oid.to_string(),
        short_oid,
        source,
    }
}

/// Every file that differs between the base commit and the worktree right now.
pub fn changed_files(
    worktree_repo: &Repository,
    base: &ReviewBase,
) -> Result<Vec<CommitFileChange>, git2::Error> {
    let mut opts = diff_options();
    let diff = diff_base_to_worktree(worktree_repo, base, &mut opts)?;

    Ok(commits::changed_files(&diff))
}

/// How many files and lines differ between the base commit and the worktree.
///
/// Counts the same set [`changed_files`] lists, so the two can never disagree
/// about what "changed" means. `show_untracked_content` is set for the reason
/// [`file_diff`] sets it: without it an untracked file is reported as a delta
/// carrying no lines, and a brand-new file — the very thing an agent leaves
/// behind — would count zero insertions.
pub fn summary(
    worktree_repo: &Repository,
    base: &ReviewBase,
) -> Result<ReviewSummary, git2::Error> {
    let mut opts = diff_options();
    opts.show_untracked_content(true);
    let diff = diff_base_to_worktree(worktree_repo, base, &mut opts)?;
    let stats = diff.stats()?;

    Ok(ReviewSummary {
        files_changed: stats.files_changed(),
        insertions: stats.insertions(),
        deletions: stats.deletions(),
    })
}

/// Unified patch text for one path. Empty when the path is unchanged or binary.
pub fn file_diff(
    worktree_repo: &Repository,
    base: &ReviewBase,
    path: &str,
) -> Result<String, git2::Error> {
    let mut opts = diff_options();
    opts.pathspec(path);
    // `path` is one exact entry from `changed_files`, not a pattern, so don't
    // let `*`, `?` or `[..]` in a filename glob-match its siblings.
    opts.disable_pathspec_match(true);
    // Without this an untracked file still appears as a delta but carries no
    // lines, so a brand-new file — the very thing an agent leaves behind —
    // would render as an empty patch. `worktree::get_file_diff` needs the same
    // option for the same reason.
    opts.show_untracked_content(true);
    let diff = diff_base_to_worktree(worktree_repo, base, &mut opts)?;

    commits::patch_text(&diff)
}

/// Untracked files are included and their directories recursed into: a file an
/// agent created is exactly the kind of work a review must not miss.
fn diff_options() -> git2::DiffOptions {
    let mut opts = git2::DiffOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    opts
}

/// The base commit's tree against the worktree's own working directory.
///
/// `diff_tree_to_workdir_with_index` is what folds committed, staged and
/// unstaged work into one diff. It has to run on the worktree's own
/// `Repository`, because that is the handle with a working directory; the base
/// commit itself lives in the object store every worktree shares.
fn diff_base_to_worktree<'repo>(
    worktree_repo: &'repo Repository,
    base: &ReviewBase,
    opts: &mut git2::DiffOptions,
) -> Result<git2::Diff<'repo>, git2::Error> {
    let oid = git2::Oid::from_str(&base.oid)?;
    let tree = worktree_repo.find_commit(oid)?.tree()?;

    worktree_repo.diff_tree_to_workdir_with_index(Some(&tree), Some(opts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::git::linked_worktrees::add_worktree;
    use crate::shared::worktrees::NewWorktreeRequest;
    use crate::testutil::{TestRepoDir, commit_all};
    use std::path::{Path, PathBuf};

    /// A repository with one commit, plus a directory to put worktrees in.
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

    /// Create a worktree and hand back its own repository handle.
    fn worktree_at(repo: &Repository, dir: &Path, name: &str) -> (PathBuf, Repository) {
        let destination = dir.join(name);
        add_worktree(
            repo,
            &request(name, &format!("feature/{name}"), destination.clone()),
        )
        .expect("add worktree");
        let opened = Repository::discover(&destination).expect("discover worktree");
        (destination, opened)
    }

    fn write(path: &Path, relative: &str, contents: &str) {
        let full = path.join(relative);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(full, contents).expect("write");
    }

    fn paths(files: &[CommitFileChange]) -> Vec<&str> {
        files.iter().map(|file| file.path.as_str()).collect()
    }

    #[test]
    fn a_recorded_base_is_used_as_given() {
        let (repo_dir, worktrees) = repo_with_commit();
        let repo = repo_dir.open();
        let recorded = repo.head().expect("head").target().expect("target");
        let (_, worktree) = worktree_at(&repo, worktrees.path(), "wt");

        let base = resolve_base(&worktree, Some(&recorded.to_string())).expect("base");

        assert_eq!(base.oid, recorded.to_string());
        assert_eq!(base.source, ReviewBaseSource::Recorded);
    }

    #[test]
    fn without_a_recorded_base_the_fork_point_is_used() {
        let (repo_dir, worktrees) = repo_with_commit();
        let repo = repo_dir.open();
        let fork = repo.head().expect("head").target().expect("target");
        let (destination, worktree) = worktree_at(&repo, worktrees.path(), "wt");

        // The worktree commits of its own, so HEAD is no longer the fork point.
        write(&destination, "own.txt", "own work\n");
        commit_all(&worktree, "work in the worktree");

        let base = resolve_base(&worktree, None).expect("base");

        assert_eq!(base.source, ReviewBaseSource::ForkPoint);
        assert_eq!(base.oid, fork.to_string(), "should be where it diverged");
    }

    #[test]
    fn a_recorded_base_that_no_longer_exists_falls_back_to_the_fork_point() {
        let (repo_dir, worktrees) = repo_with_commit();
        let repo = repo_dir.open();
        let (_, worktree) = worktree_at(&repo, worktrees.path(), "wt");
        let vanished = "0".repeat(40);

        let base = resolve_base(&worktree, Some(&vanished)).expect("base");

        assert_eq!(base.source, ReviewBaseSource::ForkPoint);
        assert_ne!(base.oid, vanished);
    }

    /// The case that matters most: an unattended agent leaves new files behind.
    #[test]
    fn untracked_files_are_part_of_the_review() {
        let (repo_dir, worktrees) = repo_with_commit();
        let repo = repo_dir.open();
        let (destination, worktree) = worktree_at(&repo, worktrees.path(), "wt");
        write(&destination, "src/created.rs", "fn created() {}\n");

        let base = resolve_base(&worktree, None).expect("base");
        let files = changed_files(&worktree, &base).expect("files");

        assert_eq!(paths(&files), vec!["src/created.rs"]);
        assert_eq!(files[0].display_status, "new");
    }

    /// The highest-value test here: without `show_untracked_content(true)` a new
    /// file is still reported as a delta, but with no lines, so the totals would
    /// read "1 file, +0 -0" for work an agent just wrote.
    #[test]
    fn a_brand_new_file_counts_its_lines_in_the_summary() {
        let (repo_dir, worktrees) = repo_with_commit();
        let repo = repo_dir.open();
        let (destination, worktree) = worktree_at(&repo, worktrees.path(), "wt");
        write(&destination, "src/created.rs", "one\ntwo\nthree\n");

        let base = resolve_base(&worktree, None).expect("base");
        let totals = summary(&worktree, &base).expect("summary");

        assert_eq!(totals.files_changed, 1);
        assert_eq!(
            totals.insertions, 3,
            "a new file's lines are all insertions"
        );
        assert_eq!(totals.deletions, 0);
    }

    #[test]
    fn the_summary_counts_files_and_lines_since_the_base() {
        let (repo_dir, worktrees) = repo_with_commit();
        let repo = repo_dir.open();
        let (destination, worktree) = worktree_at(&repo, worktrees.path(), "wt");

        // Committed.
        write(&destination, "committed.txt", "a\nb\n");
        commit_all(&worktree, "committed work");

        // Staged.
        write(&destination, "staged.txt", "c\n");
        {
            let mut index = worktree.index().expect("index");
            index.add_path(Path::new("staged.txt")).expect("stage");
            index.write().expect("write index");
        }

        // Unstaged: replaces the one line the base committed.
        write(&destination, "README.md", "changed\n");

        // Untracked.
        write(&destination, "untracked.txt", "d\ne\n");

        let base = resolve_base(&worktree, None).expect("base");
        let totals = summary(&worktree, &base).expect("summary");

        assert_eq!(totals.files_changed, 4, "all four states count");
        assert_eq!(totals.insertions, 2 + 1 + 1 + 2);
        assert_eq!(totals.deletions, 1, "README.md's original line");
        assert_eq!(totals.label(), "4 files, +6 -1");
    }

    #[test]
    fn a_worktree_that_has_done_nothing_summarises_as_zero() {
        let (repo_dir, worktrees) = repo_with_commit();
        let repo = repo_dir.open();
        let (_, worktree) = worktree_at(&repo, worktrees.path(), "wt");

        let base = resolve_base(&worktree, None).expect("base");
        let totals = summary(&worktree, &base).expect("summary");

        assert!(totals.is_empty());
        assert_eq!(totals, crate::shared::review::ReviewSummary::default());
    }

    #[test]
    fn committed_staged_and_unstaged_work_all_appear_together() {
        let (repo_dir, worktrees) = repo_with_commit();
        let repo = repo_dir.open();
        let (destination, worktree) = worktree_at(&repo, worktrees.path(), "wt");

        write(&destination, "committed.txt", "committed\n");
        commit_all(&worktree, "committed work");

        write(&destination, "staged.txt", "staged\n");
        {
            let mut index = worktree.index().expect("index");
            index.add_path(Path::new("staged.txt")).expect("stage");
            index.write().expect("write index");
        }

        write(&destination, "README.md", "changed but not staged\n");

        let base = resolve_base(&worktree, None).expect("base");
        let changed = changed_files(&worktree, &base).expect("files");
        let mut files = paths(&changed);
        files.sort_unstable();

        assert_eq!(files, vec!["README.md", "committed.txt", "staged.txt"]);
    }

    #[test]
    fn a_worktree_that_has_done_nothing_reviews_as_empty() {
        let (repo_dir, worktrees) = repo_with_commit();
        let repo = repo_dir.open();
        let (_, worktree) = worktree_at(&repo, worktrees.path(), "wt");

        let base = resolve_base(&worktree, None).expect("base");

        assert!(changed_files(&worktree, &base).expect("files").is_empty());
    }

    #[test]
    fn a_patch_is_produced_for_one_path() {
        let (repo_dir, worktrees) = repo_with_commit();
        let repo = repo_dir.open();
        let (destination, worktree) = worktree_at(&repo, worktrees.path(), "wt");
        write(&destination, "README.md", "hello\nworld\n");

        let base = resolve_base(&worktree, None).expect("base");
        let patch = file_diff(&worktree, &base, "README.md").expect("patch");

        assert!(patch.contains("+world"), "unexpected patch: {patch}");
    }

    /// A brand-new file must show its contents, not just its name: without
    /// `show_untracked_content` git lists the delta but emits no lines.
    #[test]
    fn a_brand_new_file_shows_its_contents() {
        let (repo_dir, worktrees) = repo_with_commit();
        let repo = repo_dir.open();
        let (destination, worktree) = worktree_at(&repo, worktrees.path(), "wt");
        write(&destination, "src/created.rs", "fn created() {}\n");

        let base = resolve_base(&worktree, None).expect("base");
        let patch = file_diff(&worktree, &base, "src/created.rs").expect("patch");

        assert!(
            patch.contains("fn created()"),
            "a new file's contents must reach the review: {patch:?}"
        );
    }

    /// A filename may legitimately contain glob metacharacters.
    #[test]
    fn the_path_is_exact_not_a_glob() {
        let (repo_dir, worktrees) = repo_with_commit();
        let repo = repo_dir.open();
        let (destination, worktree) = worktree_at(&repo, worktrees.path(), "wt");
        write(&destination, "a[0].txt", "bracket\n");
        write(&destination, "ab.txt", "sibling\n");

        let base = resolve_base(&worktree, None).expect("base");
        let patch = file_diff(&worktree, &base, "a[0].txt").expect("patch");

        assert!(patch.contains("bracket"), "unexpected patch: {patch:?}");
        assert!(
            !patch.contains("sibling"),
            "the glob must not sweep in a sibling: {patch}"
        );
    }

    #[test]
    fn a_worktree_without_commits_reports_why_rather_than_panicking() {
        let repo_dir = TestRepoDir::init();
        let repo = repo_dir.open();

        // An unborn HEAD has nothing to measure from.
        let error = resolve_base(&repo, None).expect_err("no commits");

        assert!(
            error.message().contains("no commits"),
            "unexpected message: {}",
            error.message()
        );
    }
}
