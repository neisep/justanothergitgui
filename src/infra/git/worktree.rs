use git2::{Repository, Status, StatusOptions};
use std::path::Path;

use crate::infra::git::error::ConflictError;
use crate::shared::conflicts::{ConflictChoice, ConflictData, ConflictPart, Eol, FileStyle, diff3};
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

pub fn undo_last_commit(repo: &Repository) -> Result<String, git2::Error> {
    let head = repo.head()?;
    if !head.is_branch() {
        return Err(git2::Error::from_str(
            "Undo last commit requires a checked-out branch",
        ));
    }

    let branch_name = head
        .shorthand()
        .ok_or_else(|| git2::Error::from_str("Branch has no short name"))?
        .to_string();
    let branch_ref_name = head
        .name()
        .map(ToOwned::to_owned)
        .ok_or_else(|| git2::Error::from_str("Undo last commit requires a named branch"))?;
    let head_commit = head.peel_to_commit()?;
    let short_oid = short_object_id(repo, head_commit.id())?;
    drop(head);

    if head_commit.parent_count() > 0 {
        let parent = head_commit.parent(0)?;
        repo.reset(parent.as_object(), git2::ResetType::Soft, None)?;
    } else {
        let mut branch = repo.find_reference(&branch_ref_name)?;
        branch.delete()?;
    }

    Ok(format!(
        "Removed commit {} from {} and kept its changes staged",
        short_oid, branch_name
    ))
}

fn short_object_id(repo: &Repository, oid: git2::Oid) -> Result<String, git2::Error> {
    Ok(repo
        .find_object(oid, None)?
        .short_id()?
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| oid.to_string()))
}

pub fn get_file_diff(repo: &Repository, path: &str, staged: bool) -> Result<String, git2::Error> {
    let mut opts = git2::DiffOptions::new();
    opts.pathspec(path);
    // `path` is one exact entry from `get_file_statuses`, not a pattern, so don't
    // let `*`, `?` or `[..]` in a filename glob-match its siblings into the patch.
    opts.disable_pathspec_match(true);

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
        // Lossy rather than skipped: dropping the body of a non-UTF-8 line would
        // render it as an empty line, which reads as "this line is blank" rather
        // than "this line could not be decoded".
        result.push_str(&String::from_utf8_lossy(line.content()));
        true
    })?;

    Ok(result)
}

pub fn read_conflict_file(repo: &Repository, path: &str) -> Result<ConflictData, ConflictError> {
    // Prefer the index's conflict stages: they carry the merge base, so we can
    // run a real 3-way merge and pre-resolve edits only one side made. Fall back
    // to parsing the in-tree markers (base-less) when stages are unavailable —
    // e.g. the file was hand-edited or the merge state is gone.
    let (sections, style) = match read_index_conflict_sections(repo, path)? {
        Some(parsed) => parsed,
        None => {
            let full_path = conflict_workdir(repo)?.join(path);
            let content = std::fs::read_to_string(&full_path)?;
            let style = FileStyle::detect(&content);
            (parse_conflict_markers(&content, style.eol)?, style)
        }
    };

    Ok(ConflictData::new(path.to_string(), sections, style))
}

/// Build the conflict sections from the index's ancestor/ours/theirs stages via
/// a 3-way merge, paired with the formatting to restore on write.
///
/// `Ok(None)` when the path is not a full three-stage conflict or any stage is
/// non-UTF-8, signalling the caller to fall back to marker parsing.
fn read_index_conflict_sections(
    repo: &Repository,
    path: &str,
) -> Result<Option<(Vec<ConflictPart>, FileStyle)>, ConflictError> {
    let index = repo.index()?;
    let target = Path::new(path);

    let (Some(base_entry), Some(ours_entry), Some(theirs_entry)) = (
        index.get_path(target, 1),
        index.get_path(target, 2),
        index.get_path(target, 3),
    ) else {
        return Ok(None);
    };

    // Validate in place; the copy only happens for a stage we actually keep,
    // not for one that turns out to be binary and gets discarded.
    let read_blob = |oid| -> Result<Option<String>, ConflictError> {
        let blob = repo.find_blob(oid)?;
        Ok(std::str::from_utf8(blob.content()).ok().map(str::to_owned))
    };

    let (Some(base), Some(ours), Some(theirs)) = (
        read_blob(base_entry.id)?,
        read_blob(ours_entry.id)?,
        read_blob(theirs_entry.id)?,
    ) else {
        return Ok(None);
    };

    // Take the style from the checked-out side: that is the file the user has
    // in front of them, and the one being rewritten.
    let style = FileStyle::detect(&ours);

    Ok(Some((diff3(&base, &ours, &theirs, style.eol), style)))
}

/// Write the user's resolved merge text to the working tree and stage it.
///
/// The content is the final text from the merge editor, already carrying the
/// source file's line terminator and final-newline state (see
/// [`ConflictData::compose`]). It is written byte for byte: guessing a
/// terminator here would rewrite files git deliberately tracks as having none.
pub fn write_resolved_content(
    repo: &Repository,
    path: &str,
    content: &str,
) -> Result<(), ConflictError> {
    let full_path = conflict_workdir(repo)?.join(path);

    std::fs::write(&full_path, content)?;
    stage_file(repo, path)?;

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

/// Rebuild the section list from the `<<<<<<<` / `=======` / `>>>>>>>` markers
/// left in a working-tree file, for when the index stages are gone.
///
/// Every run is collected as a list of lines and joined only at the end. Growing
/// a `String` and using "is it still empty?" to decide whether a separator is
/// due would read a leading blank line as "nothing here yet" and swallow it —
/// and this text is written back to the user's file verbatim.
fn parse_conflict_markers(content: &str, eol: Eol) -> Result<Vec<ConflictPart>, ConflictError> {
    let sep = eol.as_str();
    let mut sections = Vec::new();
    let mut common: Vec<&str> = Vec::new();
    let mut ours: Vec<&str> = Vec::new();
    let mut theirs: Vec<&str> = Vec::new();
    let mut in_ours = false;
    let mut in_theirs = false;

    for line in content.lines() {
        if line.starts_with("<<<<<<<") {
            if !common.is_empty() {
                sections.push(ConflictPart::Common(join_lines(&mut common, sep)));
            }
            in_ours = true;
        } else if line.starts_with("=======") && in_ours {
            in_ours = false;
            in_theirs = true;
        } else if line.starts_with(">>>>>>>") && in_theirs {
            in_theirs = false;
            sections.push(ConflictPart::Conflict {
                ours: join_lines(&mut ours, sep),
                theirs: join_lines(&mut theirs, sep),
                resolution: ConflictChoice::default(),
            });
        } else if in_ours {
            ours.push(line);
        } else if in_theirs {
            theirs.push(line);
        } else {
            common.push(line);
        }
    }

    if in_ours || in_theirs {
        return Err(ConflictError::UnbalancedMarkers);
    }

    if !common.is_empty() {
        sections.push(ConflictPart::Common(join_lines(&mut common, sep)));
    }

    Ok(sections)
}

/// Join a run of lines and clear it, ready for the next run.
fn join_lines(lines: &mut Vec<&str>, sep: &str) -> String {
    let joined = lines.join(sep);
    lines.clear();
    joined
}

fn repo_workdir(repo: &Repository) -> Result<&Path, git2::Error> {
    repo.workdir()
        .ok_or_else(|| git2::Error::from_str("Bare repositories are not supported"))
}

/// The same lookup for the merge paths, which report a bare repository as its
/// own variant rather than burying it in an opaque libgit2 error.
fn conflict_workdir(repo: &Repository) -> Result<&Path, ConflictError> {
    repo.workdir().ok_or(ConflictError::BareRepository)
}

#[cfg(test)]
mod tests {
    use super::{
        clean_untracked_files, create_commit, get_file_diff, get_file_statuses,
        parse_conflict_markers, read_conflict_file, short_object_id, stage_all, stage_file,
        undo_last_commit, unstage_all, unstage_file, write_resolved_content,
    };
    use crate::infra::git::repository::{get_commit_history, get_current_branch};
    use crate::shared::conflicts::{ConflictChoice, ConflictPart, Eol};
    use crate::testutil::{TestRepoDir, commit_all, signature};
    use git2::{Repository, RepositoryState};
    use std::path::{Path, PathBuf};
    #[cfg(unix)]
    use std::{
        ffi::OsStr,
        fs::Permissions,
        os::unix::{ffi::OsStrExt, fs::PermissionsExt},
    };

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
            Eol::Lf,
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
    fn parse_conflict_markers_keeps_blank_lines() {
        // Blank lines are content. A leading one on either side of a conflict, or
        // in the common run before it, used to read as "nothing accumulated yet"
        // and vanish — and this text is written back to the file verbatim.
        let sections = parse_conflict_markers(
            "keep\n\n<<<<<<< HEAD\n\nours\n=======\ntheirs\n>>>>>>> main\ntail",
            Eol::Lf,
        )
        .expect("parse conflict markers");

        assert_eq!(sections.len(), 3);
        assert!(matches!(sections[0], ConflictPart::Common(ref text) if text == "keep\n"));
        assert!(matches!(
            sections[1],
            ConflictPart::Conflict {
                ref ours,
                ref theirs,
                ..
            } if ours == "\nours" && theirs == "theirs"
        ));
        assert!(matches!(sections[2], ConflictPart::Common(ref text) if text == "tail"));
    }

    #[test]
    fn parse_conflict_markers_keeps_a_common_run_of_only_blank_lines() {
        // Two conflicts separated by a single blank line: the run joins to the
        // empty string, so an "is it empty?" test would drop the line entirely.
        let sections = parse_conflict_markers(
            "<<<<<<< HEAD\na\n=======\n1\n>>>>>>> main\n\n<<<<<<< HEAD\nb\n=======\n2\n>>>>>>> main",
            Eol::Lf,
        )
        .expect("parse conflict markers");

        assert_eq!(sections.len(), 3);
        assert!(matches!(sections[1], ConflictPart::Common(ref text) if text.is_empty()));
    }

    #[test]
    fn rejects_unbalanced_conflict_markers_at_eof() {
        let error = parse_conflict_markers("<<<<<<< HEAD\nours\n=======\ntheirs", Eol::Lf)
            .expect_err("unbalanced conflict markers should fail");

        assert!(matches!(error, super::ConflictError::UnbalancedMarkers));
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

        assert!(matches!(error, super::ConflictError::UnbalancedMarkers));
    }

    #[test]
    fn write_resolved_content_writes_buffer_verbatim_and_stages_file() {
        let repo_dir = TestRepoDir::init();
        let repo = Repository::open(repo_dir.path()).expect("open temp repo");

        // The terminator is `compose`'s business — it knows what the source file
        // looked like. This writer must not second-guess it.
        write_resolved_content(&repo, "merged.txt", "line one\nline two")
            .expect("write resolved content");

        let written =
            std::fs::read_to_string(repo_dir.path().join("merged.txt")).expect("read written file");
        assert_eq!(written, "line one\nline two");

        let (_unstaged, staged) = get_file_statuses(&repo).expect("read file statuses");
        assert!(staged.iter().any(|file| file.path == "merged.txt"));
    }

    #[test]
    fn write_resolved_content_preserves_crlf_bytes() {
        let repo_dir = TestRepoDir::init();
        let repo = Repository::open(repo_dir.path()).expect("open temp repo");

        write_resolved_content(&repo, "crlf.txt", "one\r\ntwo\r\n")
            .expect("write resolved content");

        let written = std::fs::read(repo_dir.path().join("crlf.txt")).expect("read written file");
        assert_eq!(written, b"one\r\ntwo\r\n");
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

    #[test]
    fn undo_last_commit_soft_resets_latest_commit_and_keeps_changes_staged() {
        let repo_dir = TestRepoDir::init();
        let repo = Repository::open(repo_dir.path()).expect("open temp repo");
        let file_path = repo_dir.path().join("tracked.txt");

        std::fs::write(&file_path, "first").expect("write first content");
        let first_oid = commit_all(&repo, "first");

        std::fs::write(&file_path, "second").expect("write second content");
        let second_oid = commit_all(&repo, "second");

        let message = undo_last_commit(&repo).expect("undo latest commit");

        assert!(message.contains(&short_object_id(&repo, second_oid).expect("short oid")));
        assert!(message.contains("kept its changes staged"));
        assert_eq!(repo.head().expect("head").target(), Some(first_oid));
        assert_eq!(
            std::fs::read_to_string(&file_path).expect("read file"),
            "second"
        );

        let (unstaged, staged) = get_file_statuses(&repo).expect("file statuses");
        assert!(unstaged.is_empty());
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].path, "tracked.txt");
    }

    #[test]
    fn undo_last_commit_supports_initial_commit_by_restoring_unborn_head() {
        let repo_dir = TestRepoDir::init();
        let repo = Repository::open(repo_dir.path()).expect("open temp repo");
        let file_path = repo_dir.path().join("tracked.txt");

        std::fs::write(&file_path, "first").expect("write first content");
        let first_oid = commit_all(&repo, "first");

        let message = undo_last_commit(&repo).expect("undo initial commit");

        assert!(message.contains(&short_object_id(&repo, first_oid).expect("short oid")));
        assert_eq!(get_current_branch(&repo).expect("current branch"), "main");
        assert!(get_commit_history(&repo, 10).expect("history").is_empty());
        assert_eq!(
            std::fs::read_to_string(&file_path).expect("read file"),
            "first"
        );

        let (unstaged, staged) = get_file_statuses(&repo).expect("file statuses");
        assert!(unstaged.is_empty());
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].path, "tracked.txt");
    }

    // --- get_file_statuses classification ---------------------------------

    #[test]
    fn get_file_statuses_reports_untracked_file_as_unstaged() {
        let repo_dir = TestRepoDir::init();
        let repo = repo_dir.open();
        repo_dir.write("new.txt", "hello");

        let (unstaged, staged) = get_file_statuses(&repo).expect("file statuses");

        assert!(staged.is_empty());
        assert_eq!(unstaged.len(), 1);
        assert_eq!(unstaged[0].path, "new.txt");
        assert_eq!(unstaged[0].display_status, "untracked");
    }

    #[test]
    fn get_file_statuses_reports_staged_new_file() {
        let repo_dir = TestRepoDir::init();
        let repo = repo_dir.open();
        repo_dir.write("new.txt", "hello");
        stage_file(&repo, "new.txt").expect("stage new file");

        let (unstaged, staged) = get_file_statuses(&repo).expect("file statuses");

        assert!(unstaged.is_empty());
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].path, "new.txt");
        assert_eq!(staged[0].display_status, "new");
    }

    #[test]
    fn get_file_statuses_reports_unstaged_modification() {
        let repo_dir = TestRepoDir::init();
        let repo = repo_dir.open();
        repo_dir.write("tracked.txt", "v1");
        commit_all(&repo, "add tracked");
        repo_dir.write("tracked.txt", "v2");

        let (unstaged, staged) = get_file_statuses(&repo).expect("file statuses");

        assert!(staged.is_empty());
        assert_eq!(unstaged.len(), 1);
        assert_eq!(unstaged[0].display_status, "modified");
    }

    #[test]
    fn get_file_statuses_lists_partially_staged_file_in_both_sets() {
        let repo_dir = TestRepoDir::init();
        let repo = repo_dir.open();
        repo_dir.write("tracked.txt", "v1");
        commit_all(&repo, "add tracked");

        // Stage one version, then edit again without staging.
        repo_dir.write("tracked.txt", "v2");
        stage_file(&repo, "tracked.txt").expect("stage v2");
        repo_dir.write("tracked.txt", "v3");

        let (unstaged, staged) = get_file_statuses(&repo).expect("file statuses");

        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].display_status, "modified");
        assert_eq!(unstaged.len(), 1);
        assert_eq!(unstaged[0].display_status, "modified");
    }

    #[test]
    fn get_file_statuses_reports_worktree_deletion() {
        let repo_dir = TestRepoDir::init();
        let repo = repo_dir.open();
        repo_dir.write("tracked.txt", "v1");
        commit_all(&repo, "add tracked");
        std::fs::remove_file(repo_dir.path().join("tracked.txt")).expect("remove file");

        let (unstaged, staged) = get_file_statuses(&repo).expect("file statuses");

        assert!(staged.is_empty());
        assert_eq!(unstaged.len(), 1);
        assert_eq!(unstaged[0].display_status, "deleted");
    }

    #[test]
    fn get_file_statuses_reports_staged_deletion() {
        let repo_dir = TestRepoDir::init();
        let repo = repo_dir.open();
        repo_dir.write("tracked.txt", "v1");
        commit_all(&repo, "add tracked");
        std::fs::remove_file(repo_dir.path().join("tracked.txt")).expect("remove file");
        stage_file(&repo, "tracked.txt").expect("stage deletion");

        let (unstaged, staged) = get_file_statuses(&repo).expect("file statuses");

        assert!(unstaged.is_empty());
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].display_status, "deleted");
    }

    // --- stage_file / unstage_file ----------------------------------------

    #[test]
    fn stage_file_moves_new_file_into_staged() {
        let repo_dir = TestRepoDir::init();
        let repo = repo_dir.open();
        repo_dir.write("new.txt", "hello");

        stage_file(&repo, "new.txt").expect("stage new file");

        let (unstaged, staged) = get_file_statuses(&repo).expect("file statuses");
        assert!(unstaged.is_empty());
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].path, "new.txt");
    }

    #[test]
    fn stage_file_stages_deletion_of_tracked_file() {
        let repo_dir = TestRepoDir::init();
        let repo = repo_dir.open();
        repo_dir.write("tracked.txt", "v1");
        commit_all(&repo, "add tracked");
        std::fs::remove_file(repo_dir.path().join("tracked.txt")).expect("remove file");

        stage_file(&repo, "tracked.txt").expect("stage deletion");

        let (unstaged, staged) = get_file_statuses(&repo).expect("file statuses");
        assert!(unstaged.is_empty());
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].display_status, "deleted");
    }

    #[test]
    fn unstage_file_restores_modified_file_to_head_index() {
        let repo_dir = TestRepoDir::init();
        let repo = repo_dir.open();
        repo_dir.write("tracked.txt", "v1");
        commit_all(&repo, "add tracked");

        repo_dir.write("tracked.txt", "v2");
        stage_file(&repo, "tracked.txt").expect("stage v2");

        unstage_file(&repo, "tracked.txt").expect("unstage");

        let (unstaged, staged) = get_file_statuses(&repo).expect("file statuses");
        assert!(staged.is_empty(), "modification should leave the index");
        assert_eq!(unstaged.len(), 1);
        assert_eq!(unstaged[0].display_status, "modified");
        // The working-tree edit is preserved.
        assert_eq!(
            std::fs::read_to_string(repo_dir.path().join("tracked.txt")).expect("read"),
            "v2"
        );
    }

    #[test]
    fn unstage_file_removes_newly_added_file_from_index() {
        let repo_dir = TestRepoDir::init();
        let repo = repo_dir.open();
        repo_dir.write("base.txt", "base");
        commit_all(&repo, "base commit");

        repo_dir.write("new.txt", "hello");
        stage_file(&repo, "new.txt").expect("stage new file");

        unstage_file(&repo, "new.txt").expect("unstage");

        let (unstaged, staged) = get_file_statuses(&repo).expect("file statuses");
        assert!(staged.is_empty());
        assert_eq!(unstaged.len(), 1);
        assert_eq!(unstaged[0].path, "new.txt");
        assert_eq!(unstaged[0].display_status, "untracked");
    }

    // --- stage_all / unstage_all ------------------------------------------

    #[test]
    fn stage_all_stages_new_modified_and_deleted() {
        let repo_dir = TestRepoDir::init();
        let repo = repo_dir.open();
        repo_dir.write("keep.txt", "v1");
        repo_dir.write("remove.txt", "gone");
        commit_all(&repo, "base commit");

        repo_dir.write("keep.txt", "v2"); // modified
        repo_dir.write("added.txt", "new"); // new
        std::fs::remove_file(repo_dir.path().join("remove.txt")).expect("remove file"); // deleted

        stage_all(&repo).expect("stage all");

        let (unstaged, staged) = get_file_statuses(&repo).expect("file statuses");
        assert!(unstaged.is_empty());
        let mut labels: Vec<_> = staged
            .iter()
            .map(|f| (f.path.clone(), f.display_status.clone()))
            .collect();
        labels.sort();
        assert_eq!(
            labels,
            vec![
                ("added.txt".to_string(), "new".to_string()),
                ("keep.txt".to_string(), "modified".to_string()),
                ("remove.txt".to_string(), "deleted".to_string()),
            ]
        );
    }

    #[test]
    fn unstage_all_clears_staged_but_keeps_worktree_changes() {
        let repo_dir = TestRepoDir::init();
        let repo = repo_dir.open();
        repo_dir.write("base.txt", "v1");
        commit_all(&repo, "base commit");

        repo_dir.write("base.txt", "v2");
        repo_dir.write("added.txt", "new");
        stage_all(&repo).expect("stage all");

        unstage_all(&repo).expect("unstage all");

        let (unstaged, staged) = get_file_statuses(&repo).expect("file statuses");
        assert!(staged.is_empty(), "nothing should remain staged");
        assert_eq!(unstaged.len(), 2, "worktree changes are preserved");
    }

    // --- create_commit ----------------------------------------------------

    #[test]
    fn create_commit_advances_head_with_parent_and_message() {
        let repo_dir = TestRepoDir::init();
        let repo = repo_dir.open();
        repo_dir.write("file.txt", "v1");
        let first = commit_all(&repo, "first");

        repo_dir.write("file.txt", "v2");
        stage_all(&repo).expect("stage all");
        let second = create_commit(&repo, "second").expect("create commit");

        let commit = repo.find_commit(second).expect("find commit");
        assert_eq!(commit.message(), Some("second"));
        assert_eq!(commit.parent_count(), 1);
        assert_eq!(commit.parent_id(0).expect("parent"), first);
        assert_eq!(repo.head().expect("head").target(), Some(second));

        // Nothing left to commit.
        let (unstaged, staged) = get_file_statuses(&repo).expect("file statuses");
        assert!(unstaged.is_empty());
        assert!(staged.is_empty());
    }

    #[test]
    fn create_commit_with_merge_head_records_two_parents_and_clears_state() {
        let repo_dir = TestRepoDir::init();
        let repo = repo_dir.open();
        repo_dir.write("file.txt", "v1");
        let base = commit_all(&repo, "base");
        let base_commit = repo.find_commit(base).expect("find base");

        // A second, divergent commit to stand in as the merged-in branch tip.
        let sig = signature();
        let tree = base_commit.tree().expect("base tree");
        let other = repo
            .commit(
                Some("refs/heads/other"),
                &sig,
                &sig,
                "other",
                &tree,
                &[&base_commit],
            )
            .expect("other commit");

        // Put the repository into a merge state.
        std::fs::write(repo.path().join("MERGE_HEAD"), format!("{}\n", other))
            .expect("write MERGE_HEAD");
        assert_eq!(repo.state(), RepositoryState::Merge);

        repo_dir.write("file.txt", "merged");
        stage_all(&repo).expect("stage all");
        let merge = create_commit(&repo, "merge commit").expect("create merge commit");

        let commit = repo.find_commit(merge).expect("find merge commit");
        assert_eq!(commit.parent_count(), 2);
        let parents: Vec<_> = commit.parent_ids().collect();
        assert!(parents.contains(&base));
        assert!(parents.contains(&other));
        assert_eq!(repo.state(), RepositoryState::Clean, "merge state cleared");
    }

    // --- get_file_diff ----------------------------------------------------

    #[test]
    fn get_file_diff_returns_staged_patch() {
        let repo_dir = TestRepoDir::init();
        let repo = repo_dir.open();
        repo_dir.write("file.txt", "line1\n");
        commit_all(&repo, "base");

        repo_dir.write("file.txt", "line1\nline2\n");
        stage_file(&repo, "file.txt").expect("stage");

        let diff = get_file_diff(&repo, "file.txt", true).expect("staged diff");
        assert!(diff.contains("file.txt"), "diff names the file");
        assert!(diff.contains("+line2"), "diff shows the added line");
    }

    #[test]
    fn get_file_diff_returns_unstaged_patch() {
        let repo_dir = TestRepoDir::init();
        let repo = repo_dir.open();
        repo_dir.write("file.txt", "line1\n");
        commit_all(&repo, "base");

        // Modify the working tree only; do not stage.
        repo_dir.write("file.txt", "line1\nline2\n");

        let diff = get_file_diff(&repo, "file.txt", false).expect("unstaged diff");
        assert!(diff.contains("file.txt"), "diff names the file");
        assert!(diff.contains("+line2"), "diff shows the added line");
    }

    #[test]
    fn get_file_diff_treats_the_path_as_exact_not_as_a_glob() {
        let repo_dir = TestRepoDir::init();
        let repo = repo_dir.open();
        repo_dir.write("a.rs", "one\n");
        repo_dir.write("b.rs", "two\n");
        commit_all(&repo, "base");

        repo_dir.write("a.rs", "one changed\n");
        repo_dir.write("b.rs", "two changed\n");

        // A pattern must match nothing, rather than sweeping in every sibling it
        // happens to glob onto.
        let patch = get_file_diff(&repo, "*.rs", false).expect("unstaged diff");
        assert!(!patch.contains("one changed"), "patch was: {patch}");
        assert!(!patch.contains("two changed"), "patch was: {patch}");

        // The real path still resolves, and only to itself.
        let exact = get_file_diff(&repo, "a.rs", false).expect("unstaged diff");
        assert!(exact.contains("one changed"), "patch was: {exact}");
        assert!(!exact.contains("two changed"), "patch was: {exact}");
    }

    #[test]
    fn get_file_diff_decodes_non_utf8_lines_lossily() {
        let repo_dir = TestRepoDir::init();
        let repo = repo_dir.open();
        repo_dir.write("data.txt", "clean\n");
        commit_all(&repo, "base");

        // An undecodable line must still show up as *something*: rendering it as
        // an empty line reads as "this line is blank", not "cannot decode".
        std::fs::write(repo_dir.path().join("data.txt"), b"clean\n\xFF\xFEbad\n")
            .expect("write non-utf8 content");

        let diff = get_file_diff(&repo, "data.txt", false).expect("unstaged diff");
        assert!(
            diff.contains('\u{FFFD}'),
            "undecodable bytes should survive as replacement chars: {diff:?}"
        );
        assert!(diff.contains("bad"), "diff was: {diff:?}");
    }
}
