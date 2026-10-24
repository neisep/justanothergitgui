//! Persistence for per-worktree metadata.
//!
//! One JSON file for every repository, `<config dir>/worktree-metadata.json`,
//! reached through [`crate::settings::config_dir`] exactly as
//! [`crate::session`] reaches `session.json`. Distinct from
//! [`crate::settings::AppSettings`] (preferences) and [`crate::session`] (which
//! repositories were open): this is what the user recorded *about* their
//! worktrees.
//!
//! Repositories are keyed by a path the caller resolves — see
//! `infra::git::linked_worktrees::repository_key`, which returns the *shared
//! object store*, so a repository reached through one of its worktrees is still
//! the same repository here.
//!
//! Deliberately not a hash of the path: `logging::repo_log_file_name` hashes
//! with `DefaultHasher`, whose output is not guaranteed stable across Rust
//! releases — tolerable for a log file, but it would silently orphan user data
//! on a toolchain upgrade.
//!
//! Every public entry point takes the file path, with a thin zero-argument
//! wrapper over it. That is what makes this testable: the config directory is
//! read from the process environment and cannot be redirected from a test
//! (`std::env::set_var` is `unsafe` in Rust 2024 and tests share one process),
//! so the tests drive `*_from`/`*_to` with a temporary path instead — the same
//! escape hatch `AppLogger` gets from storing its path as a field.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::settings;
use crate::shared::worktree_metadata::{WorktreeMetadata, WorktreeMetadataMap};

/// The whole file: every repository's worktree metadata.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct WorktreeMetadataFile {
    #[serde(default)]
    repositories: BTreeMap<String, WorktreeMetadataMap>,
}

/// How a repository path becomes a key inside the file.
fn repository_key(repo_key: &Path) -> String {
    repo_key.to_string_lossy().into_owned()
}

fn metadata_path() -> PathBuf {
    settings::config_dir().join("worktree-metadata.json")
}

/// Read one repository's metadata. A repository that has never had any comes
/// back empty, which is not an error.
pub fn load_for_repo(repo_key: &Path) -> Result<WorktreeMetadataMap, String> {
    load_for_repo_from(&metadata_path(), repo_key)
}

/// Set or clear one worktree's entry, returning the repository's metadata as it
/// now stands on disk.
///
/// Deliberately **one entry at a time**. One file holds every repository, and
/// several tabs can be showing the same repository at once — a tab that wrote
/// its whole in-memory map would silently undo an edit another tab made after
/// that map was read. Re-reading here and touching a single key makes the last
/// writer win per entry instead of per repository.
pub fn update_entry(
    repo_key: &Path,
    worktree_key: &str,
    entry: Option<WorktreeMetadata>,
) -> Result<WorktreeMetadataMap, String> {
    update_entry_in(&metadata_path(), repo_key, worktree_key, entry)
}

pub(crate) fn load_for_repo_from(
    file: &Path,
    repo_key: &Path,
) -> Result<WorktreeMetadataMap, String> {
    let stored = load_file(file)?;
    Ok(stored
        .repositories
        .get(&repository_key(repo_key))
        .cloned()
        .unwrap_or_default())
}

pub(crate) fn update_entry_in(
    file: &Path,
    repo_key: &Path,
    worktree_key: &str,
    entry: Option<WorktreeMetadata>,
) -> Result<WorktreeMetadataMap, String> {
    let mut stored = load_file(file)?;
    let key = repository_key(repo_key);
    let entries = stored.repositories.entry(key.clone()).or_default();

    // An entry with nothing in it is removed rather than stored, so clearing a
    // worktree's metadata leaves no trace behind.
    match entry.map(WorktreeMetadata::clamped) {
        Some(metadata) if !metadata.is_empty() => {
            entries.insert(worktree_key.to_string(), metadata);
        }
        _ => {
            entries.remove(worktree_key);
        }
    }

    let remaining = entries.clone();
    if remaining.is_empty() {
        stored.repositories.remove(&key);
    }

    save_file(file, &stored)?;
    Ok(remaining)
}

/// A missing file is an empty one; a corrupt file is an error and is never
/// silently replaced, matching `settings.rs` and `session.rs`.
fn load_file(file: &Path) -> Result<WorktreeMetadataFile, String> {
    if !file.exists() {
        return Ok(WorktreeMetadataFile::default());
    }

    let payload = fs::read_to_string(file).map_err(|error| {
        format!(
            "Could not read worktree metadata file {}: {}",
            file.display(),
            error
        )
    })?;

    serde_json::from_str(&payload).map_err(|error| {
        format!(
            "Could not parse worktree metadata file {}: {}",
            file.display(),
            error
        )
    })
}

fn save_file(file: &Path, stored: &WorktreeMetadataFile) -> Result<(), String> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Could not create worktree metadata directory {}: {}",
                parent.display(),
                error
            )
        })?;
    }

    let payload = serde_json::to_string_pretty(stored)
        .map_err(|error| format!("Could not serialize worktree metadata: {}", error))?;

    // Write to a sibling and rename over the target, rather than the plain
    // `fs::write` settings and session use. A truncated write here would lose
    // the user's own prose for every repository at once, which is worth more
    // than a list of open tabs.
    let temporary = file.with_extension("json.tmp");
    fs::write(&temporary, payload).map_err(|error| {
        format!(
            "Could not write worktree metadata file {}: {}",
            temporary.display(),
            error
        )
    })?;

    fs::rename(&temporary, file).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!(
            "Could not replace worktree metadata file {}: {}",
            file.display(),
            error
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::worktree_metadata::{ReviewState, TestState, WorktreeMetadata};
    use std::time::{SystemTime, UNIX_EPOCH};

    /// A temporary metadata file, removed with its directory on drop.
    struct TestFile {
        dir: PathBuf,
    }

    impl TestFile {
        fn new(area: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let dir = std::env::temp_dir().join(format!(
                "justanothergitgui-{}-meta-{}-{}",
                area,
                std::process::id(),
                unique
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self { dir }
        }

        fn path(&self) -> PathBuf {
            self.dir.join("worktree-metadata.json")
        }
    }

    impl Drop for TestFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn metadata(task: &str) -> WorktreeMetadata {
        WorktreeMetadata {
            task: task.into(),
            agent: "claude".into(),
            notes: "waiting on review".into(),
            base_commit: "a1b2c3d4".into(),
            started: 1_700_000_000,
            review: ReviewState::NeedsReview,
            test: TestState::Passing,
        }
    }

    #[test]
    fn metadata_survives_a_round_trip() {
        let file = TestFile::new("roundtrip");
        let repo = Path::new("/projects/myapp");

        let returned = update_entry_in(
            &file.path(),
            repo,
            "wt:feature-auth",
            Some(metadata("Add OAuth login")),
        )
        .expect("save");

        assert_eq!(returned["wt:feature-auth"].task, "Add OAuth login");
        assert_eq!(
            load_for_repo_from(&file.path(), repo).expect("load"),
            returned
        );
    }

    #[test]
    fn a_missing_file_loads_as_empty_rather_than_failing() {
        let file = TestFile::new("missing");

        let loaded = load_for_repo_from(&file.path(), Path::new("/projects/myapp")).expect("load");

        assert!(loaded.is_empty());
        assert!(!file.path().exists(), "loading must not create the file");
    }

    #[test]
    fn a_corrupt_file_is_reported_and_left_alone() {
        let file = TestFile::new("corrupt");
        std::fs::write(file.path(), "{ not json").expect("write");
        let repo = Path::new("/projects/myapp");

        let error = load_for_repo_from(&file.path(), repo).expect_err("corrupt file is reported");
        assert!(
            error.starts_with("Could not parse worktree metadata file"),
            "unexpected message: {error}"
        );

        // A write must refuse too, rather than replace notes it could not read.
        assert!(update_entry_in(&file.path(), repo, "wt:a", Some(metadata("x"))).is_err());
        assert_eq!(
            std::fs::read_to_string(file.path()).expect("read"),
            "{ not json",
            "a corrupt file must never be silently rewritten"
        );
    }

    /// The reason writes go one entry at a time: one file holds every
    /// repository, and several tabs can show the same repository at once.
    #[test]
    fn writing_one_entry_never_disturbs_another() {
        let file = TestFile::new("entries");
        let repo = Path::new("/projects/myapp");

        update_entry_in(&file.path(), repo, "wt:alpha", Some(metadata("alpha work")))
            .expect("first");
        // A second writer that never saw the first entry must not erase it.
        update_entry_in(&file.path(), repo, "wt:beta", Some(metadata("beta work")))
            .expect("second");

        let loaded = load_for_repo_from(&file.path(), repo).expect("load");
        assert_eq!(loaded["wt:alpha"].task, "alpha work");
        assert_eq!(loaded["wt:beta"].task, "beta work");
    }

    #[test]
    fn saving_one_repository_leaves_the_others_untouched() {
        let file = TestFile::new("multi");
        let first = Path::new("/projects/alpha");
        let second = Path::new("/projects/beta");

        update_entry_in(&file.path(), first, "wt:a", Some(metadata("alpha work"))).expect("first");
        update_entry_in(&file.path(), second, "wt:b", Some(metadata("beta work"))).expect("second");

        assert_eq!(
            load_for_repo_from(&file.path(), first).expect("load")["wt:a"].task,
            "alpha work"
        );
        assert_eq!(
            load_for_repo_from(&file.path(), second).expect("load")["wt:b"].task,
            "beta work"
        );
    }

    #[test]
    fn an_entry_with_nothing_in_it_is_not_persisted() {
        let file = TestFile::new("emptyentry");
        let repo = Path::new("/projects/myapp");

        let returned = update_entry_in(
            &file.path(),
            repo,
            "wt:blank",
            Some(WorktreeMetadata::default()),
        )
        .expect("save");

        assert!(returned.is_empty());
        assert!(
            load_for_repo_from(&file.path(), repo)
                .expect("load")
                .is_empty()
        );
    }

    #[test]
    fn clearing_the_last_entry_removes_the_repository_from_the_file() {
        let file = TestFile::new("clear");
        let repo = Path::new("/projects/myapp");
        update_entry_in(&file.path(), repo, "wt:only", Some(metadata("work"))).expect("save");

        let returned = update_entry_in(&file.path(), repo, "wt:only", None).expect("clear");

        assert!(returned.is_empty());
        let raw = std::fs::read_to_string(file.path()).expect("read");
        assert!(
            !raw.contains("myapp"),
            "an emptied repository should leave no section behind: {raw}"
        );
    }

    #[test]
    fn oversized_text_is_clamped_before_it_reaches_the_file() {
        let file = TestFile::new("clamp");
        let repo = Path::new("/projects/myapp");
        let huge = WorktreeMetadata {
            task: "x".repeat(10_000),
            ..WorktreeMetadata::default()
        };

        let returned = update_entry_in(&file.path(), repo, "wt:big", Some(huge)).expect("save");

        assert!(returned["wt:big"].task.len() <= crate::shared::worktree_metadata::MAX_TASK_LEN);
    }

    #[test]
    fn a_file_written_by_a_newer_build_still_loads() {
        let file = TestFile::new("forward");
        let repo = Path::new("/projects/myapp");
        std::fs::write(
            file.path(),
            r#"{
              "schema_hint": "written by a later version",
              "repositories": {
                "/projects/myapp": {
                  "wt:feature-auth": { "task": "kept", "unknown_field": true }
                }
              }
            }"#,
        )
        .expect("write");

        let loaded = load_for_repo_from(&file.path(), repo).expect("load");

        assert_eq!(loaded["wt:feature-auth"].task, "kept");
        assert_eq!(loaded["wt:feature-auth"].review, ReviewState::Unreviewed);
    }

    #[test]
    fn no_temporary_file_is_left_behind() {
        let file = TestFile::new("tmp");
        let repo = Path::new("/projects/myapp");

        update_entry_in(&file.path(), repo, "wt:a", Some(metadata("work"))).expect("save");

        assert!(file.path().exists());
        assert!(
            !file.path().with_extension("json.tmp").exists(),
            "the atomic write must clean up after itself"
        );
    }
}
