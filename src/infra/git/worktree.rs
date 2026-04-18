use git2::{Repository, Status, StatusOptions};
use std::path::Path;

use crate::shared::conflicts::{ConflictChoice, ConflictData, ConflictPart};
use crate::shared::git::FileEntry;

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct CleanUntrackedResult {
    pub removed_count: usize,
    pub failures: Vec<UntrackedRemovalFailure>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct UntrackedRemovalFailure {
    pub path: String,
    pub error: String,
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
        let Some(path) = entry.path() else {
            // libgit2 only exposes UTF-8 status paths here; skip entries we cannot
            // safely represent because downstream staging/unstaging expects `&str`.
            continue;
        };
        let path = path.to_string();
        let status = entry.status();

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

pub fn stage_file(repo: &Repository, path: &str) -> Result<(), git2::Error> {
    let mut index = repo.index()?;
    let full_path = repo_workdir(repo)?.join(path);

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
    let path_ref = Path::new(path);

    match repo.head() {
        Ok(head_ref) => {
            let commit = head_ref.peel_to_commit()?;
            let tree = commit.tree()?;
            match tree.get_path(path_ref) {
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
                    index.remove_path(path_ref)?;
                }
            }
        }
        Err(_) => {
            index.remove_path(path_ref)?;
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
    let signature = repo.signature()?;
    let mut parents = Vec::new();

    if let Ok(head) = repo.head() {
        parents.push(head.peel_to_commit()?);
    }

    if repo.state() == git2::RepositoryState::Merge
        && let Ok(merge_head) = repo.find_reference("MERGE_HEAD")
        && let Some(merge_oid) = merge_head.target()
    {
        parents.push(repo.find_commit(merge_oid)?);
    }

    let parent_refs: Vec<&git2::Commit<'_>> = parents.iter().collect();
    let oid = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &parent_refs,
    )?;

    if repo.state() == git2::RepositoryState::Merge {
        repo.cleanup_state()?;
    }

    Ok(oid)
}

pub fn get_file_diff(repo: &Repository, path: &str, staged: bool) -> Result<String, git2::Error> {
    let mut opts = git2::DiffOptions::new();
    opts.pathspec(path);

    let diff = if staged {
        let head_tree = repo.head().ok().and_then(|head| head.peel_to_tree().ok());
        repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))?
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

pub fn read_conflict_file(repo: &Repository, path: &str) -> Result<ConflictData, String> {
    let full_path = repo_workdir(repo)
        .map_err(|error| error.to_string())?
        .join(path);
    let content = std::fs::read_to_string(&full_path).map_err(|error| error.to_string())?;
    let sections = parse_conflict_markers(&content)?;
    Ok(ConflictData {
        path: path.to_string(),
        sections,
    })
}

pub fn write_resolved_file(repo: &Repository, data: &ConflictData) -> Result<(), String> {
    let full_path = repo_workdir(repo)
        .map_err(|error| error.to_string())?
        .join(&data.path);
    let mut content = String::new();

    for (index, section) in data.sections.iter().enumerate() {
        if index > 0 {
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
                ConflictChoice::Unresolved => return Err("Not all conflicts resolved".into()),
            },
        }
    }

    content.push('\n');
    std::fs::write(&full_path, &content).map_err(|error| error.to_string())?;
    stage_file(repo, &data.path).map_err(|error| error.to_string())?;

    Ok(())
}

pub(crate) fn clean_untracked_files(
    repo: &Repository,
) -> Result<CleanUntrackedResult, git2::Error> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| git2::Error::from_str("Repository has no workdir"))?
        .to_path_buf();

    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(false);
    let statuses = repo.statuses(Some(&mut opts))?;

    let mut result = CleanUntrackedResult::default();
    for entry in statuses.iter() {
        if entry.status() != Status::WT_NEW {
            continue;
        }
        let Some(path) = entry.path() else {
            continue;
        };
        let full_path = workdir.join(path);
        let removal = if full_path.is_dir() {
            Some(std::fs::remove_dir_all(&full_path))
        } else if full_path.exists() {
            Some(std::fs::remove_file(&full_path))
        } else {
            None
        };

        match removal {
            Some(Ok(())) => result.removed_count += 1,
            Some(Err(error)) => result.failures.push(UntrackedRemovalFailure {
                path: path.trim_end_matches('/').to_string(),
                error: error.to_string(),
            }),
            None => {}
        }
    }

    Ok(result)
}

fn status_label_staged(status: Status) -> &'static str {
    if status.contains(Status::INDEX_NEW) {
        "new"
    } else if status.contains(Status::INDEX_MODIFIED) {
        "modified"
    } else if status.contains(Status::INDEX_DELETED) {
        "deleted"
    } else if status.contains(Status::INDEX_RENAMED) {
        "renamed"
    } else {
        "changed"
    }
}

fn status_label_unstaged(status: Status) -> &'static str {
    if status.contains(Status::WT_NEW) {
        "untracked"
    } else if status.contains(Status::WT_MODIFIED) {
        "modified"
    } else if status.contains(Status::WT_DELETED) {
        "deleted"
    } else {
        "changed"
    }
}

fn parse_conflict_markers(content: &str) -> Result<Vec<ConflictPart>, String> {
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

    if in_ours || in_theirs {
        return Err("Unbalanced conflict markers".into());
    }

    if !common.is_empty() {
        sections.push(ConflictPart::Common(common));
    }

    Ok(sections)
}

fn repo_workdir(repo: &Repository) -> Result<&Path, git2::Error> {
    repo.workdir()
        .ok_or_else(|| git2::Error::from_str("Bare repositories are not supported"))
}

#[cfg(test)]
mod tests {
    use super::{
        clean_untracked_files, get_file_statuses, parse_conflict_markers, read_conflict_file,
    };
    use crate::shared::conflicts::{ConflictChoice, ConflictPart};
    use git2::Repository;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};
    #[cfg(unix)]
    use std::{
        ffi::OsStr,
        fs::Permissions,
        os::unix::{ffi::OsStrExt, fs::PermissionsExt},
    };

    struct TestRepoDir {
        path: PathBuf,
    }

    impl TestRepoDir {
        fn init() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "justanothergitgui-worktree-test-{}-{}",
                std::process::id(),
                unique
            ));
            std::fs::create_dir_all(&path).expect("create temp repo dir");
            Repository::init(&path).expect("init temp repo");
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

    #[cfg(unix)]
    struct PermissionsGuard {
        path: PathBuf,
        original: Permissions,
    }

    #[cfg(unix)]
    impl PermissionsGuard {
        fn lock_dir(path: &Path) -> Self {
            let original = std::fs::metadata(path)
                .expect("read permissions")
                .permissions();
            let mut locked = original.clone();
            locked.set_mode(0o555);
            std::fs::set_permissions(path, locked).expect("lock directory");
            Self {
                path: path.to_path_buf(),
                original,
            }
        }
    }

    #[cfg(unix)]
    impl Drop for PermissionsGuard {
        fn drop(&mut self) {
            let _ = std::fs::set_permissions(&self.path, self.original.clone());
        }
    }

    #[test]
    fn parses_complete_conflict_markers() {
        let sections = parse_conflict_markers(
            "before\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> main\nafter",
        )
        .expect("parse complete conflict markers");

        assert_eq!(sections.len(), 3);
        assert!(matches!(sections[0], ConflictPart::Common(ref text) if text == "before"));
        assert!(matches!(
            sections[1],
            ConflictPart::Conflict {
                ref ours,
                ref theirs,
                resolution: ConflictChoice::Unresolved,
            } if ours == "ours" && theirs == "theirs"
        ));
        assert!(matches!(sections[2], ConflictPart::Common(ref text) if text == "after"));
    }

    #[test]
    fn rejects_unbalanced_conflict_markers_at_eof() {
        let error = parse_conflict_markers("<<<<<<< HEAD\nours\n=======\ntheirs")
            .expect_err("unbalanced conflict markers should fail");

        assert!(error.contains("Unbalanced conflict markers"));
    }

    #[test]
    fn read_conflict_file_rejects_malformed_markers() {
        let repo_dir = TestRepoDir::init();
        let repo = Repository::open(repo_dir.path()).expect("open temp repo");
        std::fs::write(
            repo_dir.path().join("conflicted.txt"),
            "<<<<<<< HEAD\nours\n=======\ntheirs",
        )
        .expect("write conflict file");

        let error = read_conflict_file(&repo, "conflicted.txt")
            .expect_err("malformed conflict file should fail");

        assert!(error.contains("Unbalanced conflict markers"));
    }

    #[cfg(unix)]
    #[test]
    fn clean_untracked_files_reports_failed_removals() {
        let repo_dir = TestRepoDir::init();
        let repo = Repository::open(repo_dir.path()).expect("open temp repo");

        std::fs::write(repo_dir.path().join("removable.txt"), "remove me")
            .expect("write removable file");

        let blocked_dir = repo_dir.path().join("blocked");
        std::fs::create_dir(&blocked_dir).expect("create blocked dir");
        std::fs::write(blocked_dir.join("locked.txt"), "keep me").expect("write blocked file");
        let _guard = PermissionsGuard::lock_dir(&blocked_dir);

        let cleanup = clean_untracked_files(&repo).expect("clean untracked files");

        assert_eq!(cleanup.removed_count, 1);
        assert_eq!(cleanup.failures.len(), 1);
        assert_eq!(cleanup.failures[0].path, "blocked");
        assert!(!cleanup.failures[0].error.is_empty());
        assert!(!repo_dir.path().join("removable.txt").exists());
        assert!(blocked_dir.exists());
    }

    #[cfg(unix)]
    #[test]
    fn get_file_statuses_skips_non_utf8_paths() {
        let repo_dir = TestRepoDir::init();
        let repo = Repository::open(repo_dir.path()).expect("open temp repo");
        let invalid_name = OsStr::from_bytes(b"bad-\xFF-name.txt");
        let invalid_path = repo_dir.path().join(invalid_name);

        std::fs::write(&invalid_path, "non-utf8 path").expect("write non-utf8 file");

        let (unstaged, staged) = get_file_statuses(&repo).expect("read file statuses");

        assert!(staged.is_empty());
        assert!(unstaged.iter().all(|file| !file.path.is_empty()));
        assert!(unstaged.is_empty());
    }
}
