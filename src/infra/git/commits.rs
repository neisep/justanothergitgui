//! Read-only diffs of a single commit against its first parent.
//!
//! Everything here is history inspection: nothing in this module writes to the
//! repository, the index, or the working tree.

use git2::Repository;

use crate::shared::git::CommitFileChange;

/// List the files a commit touched, compared with its **first** parent.
///
/// Merge commits are therefore shown from the perspective of the branch they
/// were merged into. A root commit (no parents) is compared with an empty tree,
/// so every file in it shows up as added.
pub fn commit_changed_files(
    repo: &Repository,
    oid: &str,
) -> Result<Vec<CommitFileChange>, git2::Error> {
    let mut opts = git2::DiffOptions::new();
    let diff = diff_against_first_parent(repo, oid, &mut opts)?;

    let mut changes = Vec::new();
    for delta in diff.deltas() {
        let path = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path())
            .and_then(|path| path.to_str());
        let Some(path) = path else {
            continue;
        };

        changes.push(CommitFileChange {
            path: path.to_string(),
            display_status: display_status_for(delta.status()).to_string(),
        });
    }

    Ok(changes)
}

/// Unified patch text for one path inside a commit, compared with its first
/// parent. Returns an empty string when the path is unchanged or binary.
pub fn commit_file_diff(repo: &Repository, oid: &str, path: &str) -> Result<String, git2::Error> {
    let mut opts = git2::DiffOptions::new();
    opts.pathspec(path);
    let diff = diff_against_first_parent(repo, oid, &mut opts)?;

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

fn diff_against_first_parent<'repo>(
    repo: &'repo Repository,
    oid: &str,
    opts: &mut git2::DiffOptions,
) -> Result<git2::Diff<'repo>, git2::Error> {
    let oid = git2::Oid::from_str(oid)?;
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;
    let parent_tree = match commit.parent(0) {
        Ok(parent) => Some(parent.tree()?),
        Err(_) => None,
    };

    repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(opts))
}

fn display_status_for(status: git2::Delta) -> &'static str {
    match status {
        git2::Delta::Added | git2::Delta::Copied | git2::Delta::Untracked => "new",
        git2::Delta::Deleted => "deleted",
        git2::Delta::Renamed => "renamed",
        git2::Delta::Modified | git2::Delta::Typechange => "modified",
        _ => "changed",
    }
}

#[cfg(test)]
mod tests {
    use super::{commit_changed_files, commit_file_diff};
    use crate::testutil::{TestRepoDir, commit_all};

    #[test]
    fn lists_every_file_of_a_root_commit_as_new() {
        let dir = TestRepoDir::init();
        let repo = dir.open();
        dir.write("a.txt", "one\n");
        dir.write("nested/b.txt", "two\n");
        let oid = commit_all(&repo, "root");

        let mut changes = commit_changed_files(&repo, &oid.to_string()).expect("changed files");
        changes.sort_by(|left, right| left.path.cmp(&right.path));

        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].path, "a.txt");
        assert_eq!(changes[0].display_status, "new");
        assert_eq!(changes[1].path, "nested/b.txt");
        assert_eq!(changes[1].display_status, "new");
    }

    #[test]
    fn lists_only_what_the_commit_changed() {
        let dir = TestRepoDir::init();
        let repo = dir.open();
        dir.write("kept.txt", "kept\n");
        dir.write("changed.txt", "before\n");
        commit_all(&repo, "root");

        dir.write("changed.txt", "after\n");
        let oid = commit_all(&repo, "second");

        let changes = commit_changed_files(&repo, &oid.to_string()).expect("changed files");

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "changed.txt");
        assert_eq!(changes[0].display_status, "modified");
    }

    #[test]
    fn reports_deleted_paths() {
        let dir = TestRepoDir::init();
        let repo = dir.open();
        dir.write("gone.txt", "bye\n");
        commit_all(&repo, "root");

        std::fs::remove_file(dir.path().join("gone.txt")).expect("remove file");
        let oid = commit_all(&repo, "delete");

        let changes = commit_changed_files(&repo, &oid.to_string()).expect("changed files");

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "gone.txt");
        assert_eq!(changes[0].display_status, "deleted");
    }

    #[test]
    fn produces_patch_text_for_one_path() {
        let dir = TestRepoDir::init();
        let repo = dir.open();
        dir.write("a.txt", "before\n");
        dir.write("other.txt", "untouched\n");
        commit_all(&repo, "root");

        dir.write("a.txt", "after\n");
        let oid = commit_all(&repo, "second");

        let patch = commit_file_diff(&repo, &oid.to_string(), "a.txt").expect("patch");

        assert!(patch.contains("-before"), "patch was: {patch}");
        assert!(patch.contains("+after"), "patch was: {patch}");
        assert!(!patch.contains("untouched"), "patch was: {patch}");
    }

    #[test]
    fn diffs_a_merge_commit_against_its_first_parent() {
        let dir = TestRepoDir::init();
        let repo = dir.open();
        dir.write("base.txt", "base\n");
        let base = commit_all(&repo, "root");

        // Side branch adds its own file.
        repo.branch("side", &repo.find_commit(base).expect("base commit"), false)
            .expect("create branch");
        repo.set_head("refs/heads/side").expect("checkout side");
        dir.write("side.txt", "side\n");
        let side = commit_all(&repo, "side work");

        // Back on main, add a file, then merge the side branch in.
        repo.set_head("refs/heads/main").expect("checkout main");
        repo.reset(
            repo.find_commit(base).expect("base commit").as_object(),
            git2::ResetType::Hard,
            None,
        )
        .expect("reset to base");
        dir.write("main.txt", "main\n");
        let main = commit_all(&repo, "main work");

        let merge = {
            let main_commit = repo.find_commit(main).expect("main commit");
            let side_commit = repo.find_commit(side).expect("side commit");
            dir.write("side.txt", "side\n");
            let tree_id = {
                let mut index = repo.index().expect("index");
                index
                    .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
                    .expect("add all");
                index.write().expect("write index");
                index.write_tree().expect("write tree")
            };
            let tree = repo.find_tree(tree_id).expect("find tree");
            let sig = crate::testutil::signature();
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "merge side",
                &tree,
                &[&main_commit, &side_commit],
            )
            .expect("merge commit")
        };

        let changes = commit_changed_files(&repo, &merge.to_string()).expect("changed files");

        // Against the first parent (main), only the side branch's file is new.
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "side.txt");
        assert_eq!(changes[0].display_status, "new");
    }

    #[test]
    fn rejects_an_unknown_object_id() {
        let dir = TestRepoDir::init();
        let repo = dir.open();
        dir.write("a.txt", "one\n");
        commit_all(&repo, "root");

        assert!(commit_changed_files(&repo, "not-a-valid-oid").is_err());
        assert!(commit_file_diff(&repo, "not-a-valid-oid", "a.txt").is_err());
    }
}
