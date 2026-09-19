use super::ports::AppWorktreeMetadata;
use super::*;
use crate::shared::diff::{DiffLineKind, parse_diff_rows, to_side_by_side};

use crate::shared::git::CommitFileChange;
use crate::shared::worktree_metadata::storage_key;
use crate::shared::worktrees::{LinkedWorktree, LinkedWorktreeStatus};
use crate::state::{
    BranchDialogState, CenterView, ChangeSet, CleanupBranchesDialogState, CommitState, DialogState,
    DiscardDialogState, FileActionDialogState, InspectorState, RepoState, SelectedCommit,
    SelectedFile, SelectedReview, SelectedWorktree, TagDialogState, UiState, WorktreeDialogState,
    WorktreeMetadataDialogState, WorktreeState,
};

pub(super) fn refresh_status(
    repo_state: &mut RepoState,
    worktree_state: &mut WorktreeState,
    commit_state: &mut CommitState,
    inspector_state: &mut InspectorState,
    ui_state: &mut UiState,
    repo: &Repository,
) -> Option<String> {
    let mut errors: Vec<String> = Vec::new();
    repo_state.has_origin_remote = AppRepoRead::has_origin_remote(repo);
    repo_state.has_github_origin = AppRepoRead::has_github_origin(repo);
    repo_state.has_github_https_origin = AppRepoRead::has_github_https_origin(repo);
    match AppRepoRead::outgoing_commit_count(repo) {
        Ok(count) => repo_state.outgoing_commit_count = count,
        Err(error) => {
            errors.push(format!("outgoing commit count: {error}"));
            repo_state.outgoing_commit_count = 0;
        }
    }
    match AppRepoRead::file_statuses(repo) {
        Ok((unstaged, staged)) => {
            worktree_state.unstaged = unstaged;
            worktree_state.staged = staged;
            let changed_paths = if worktree_state.staged.is_empty() {
                worktree_state
                    .unstaged
                    .iter()
                    .map(|file| file.path.as_str())
                    .collect::<Vec<_>>()
            } else {
                worktree_state
                    .staged
                    .iter()
                    .map(|file| file.path.as_str())
                    .collect::<Vec<_>>()
            };
            commit_state.inferred_commit_scopes =
                commit_rules::infer_commit_scopes(repo_state.path.as_deref(), changed_paths);
        }
        Err(error) => {
            errors.push(format!("file statuses: {error}"));
            commit_state.inferred_commit_scopes.clear();
        }
    }
    match AppRepoRead::current_branch(repo) {
        Ok(branch) => repo_state.branch = branch,
        Err(error) => {
            errors.push(format!("current branch: {error}"));
            repo_state.branch = String::new();
        }
    }
    match AppRepoRead::branches(repo) {
        Ok(branches) => repo_state.branches = branches,
        Err(error) => {
            errors.push(format!("branches: {error}"));
            repo_state.branches = Vec::new();
        }
    }
    match AppRepoRead::remote_branches(repo) {
        Ok(branches) => repo_state.remote_branches = branches,
        Err(error) => {
            errors.push(format!("remote branches: {error}"));
            repo_state.remote_branches = Vec::new();
        }
    }
    match AppRepoRead::commit_history(repo, 200) {
        Ok(history) => repo_state.commit_history = history,
        Err(error) => {
            errors.push(format!("commit history: {error}"));
            repo_state.commit_history = Vec::new();
        }
    }
    match AppRepoRead::linked_worktrees(repo) {
        Ok(worktrees) => repo_state.linked_worktrees = worktrees,
        Err(error) => {
            errors.push(format!("worktrees: {error}"));
            repo_state.linked_worktrees = Vec::new();
        }
    }
    // Re-read rather than cache across refreshes: several tabs can be showing
    // the same repository, and the file on disk is the only thing they share.
    match AppWorktreeMetadata::load(repo) {
        Ok(metadata) => repo_state.worktree_metadata = metadata,
        Err(error) => {
            errors.push(format!("worktree metadata: {error}"));
            repo_state.worktree_metadata.clear();
        }
    }
    sync_pull_request_prompt(repo_state);
    sync_selected_file(worktree_state, inspector_state, repo);
    sync_selected_commit(repo_state, inspector_state);
    sync_selected_review(repo_state, inspector_state);
    sync_selected_worktree(repo_state, inspector_state);
    if errors.is_empty() {
        None
    } else {
        let detail = errors.join("; ");
        ui_state.status = status_message_for_error("Refresh", &detail);
        Some(detail)
    }
}

/// How long ago a worktree was started, or `None` when the app did not create
/// it and so never recorded a time.
///
/// The clock is read here rather than in `ui/`, which renders what it is given
/// and has no business reading the system time.
pub(super) fn started_label(started: i64) -> Option<String> {
    if started <= 0 {
        return None;
    }
    Some(crate::shared::git::relative_time(now_secs(), started))
}

/// Unix seconds now, or `0` if the system clock predates the epoch.
pub(super) fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}

pub(super) fn reset_repo_state(repo_state: &mut RepoState) {
    repo_state.has_origin_remote = false;
    repo_state.has_github_origin = false;
    repo_state.has_github_https_origin = false;
    repo_state.branch.clear();
    repo_state.outgoing_commit_count = 0;
    repo_state.branches.clear();
    repo_state.remote_branches.clear();
    repo_state.commit_history.clear();
    repo_state.pull_request_prompt = None;
    repo_state.linked_worktrees.clear();
    repo_state.worktree_metadata.clear();
}

pub(super) fn reset_worktree_state(worktree_state: &mut WorktreeState) {
    worktree_state.unstaged.clear();
    worktree_state.staged.clear();
}

pub(super) fn reset_commit_state(commit_state: &mut CommitState) {
    commit_state.inferred_commit_scopes.clear();
    commit_state.commit_summary.clear();
    commit_state.commit_body.clear();
    commit_state.focus_commit_summary_requested = false;
}

pub(super) fn reset_inspector_state(inspector_state: &mut InspectorState) {
    inspector_state.selected_file = None;
    inspector_state.clear_diff();
    inspector_state.diff_wrap = false;
    inspector_state.file_filter.clear();
    inspector_state.center_view = CenterView::Diff;
    inspector_state.set_conflict(None);
    inspector_state.set_commit(None);
    inspector_state.set_review(None);
    inspector_state.set_selected_worktree(None);
    inspector_state.dragging = None;
}

pub(super) fn reset_dialog_state(dialog_state: &mut DialogState) {
    reset_branch_dialog_state(&mut dialog_state.branch);
    reset_tag_dialog_state(&mut dialog_state.tag);
    reset_cleanup_dialog_state(&mut dialog_state.cleanup);
    reset_discard_dialog_state(&mut dialog_state.discard);
    reset_file_action_dialog_state(&mut dialog_state.file_action);
    reset_worktree_dialog_state(&mut dialog_state.worktree);
    reset_worktree_metadata_dialog_state(&mut dialog_state.worktree_metadata);
}

pub(super) fn reset_ui_state(ui_state: &mut UiState) {
    ui_state.actions.clear();
    ui_state.busy = None;
}

fn reset_branch_dialog_state(dialog_state: &mut BranchDialogState) {
    dialog_state.new_branch_name.clear();
    dialog_state.focus_new_branch_name_requested = false;
    dialog_state.show_create_branch_dialog = false;
    dialog_state.show_create_branch_confirm = false;
    dialog_state.create_branch_preview = None;
    dialog_state.pending_new_branch_name = None;
}

fn reset_tag_dialog_state(dialog_state: &mut TagDialogState) {
    dialog_state.new_tag_name.clear();
    dialog_state.focus_new_tag_name_requested = false;
    dialog_state.show_create_tag_dialog = false;
}

fn reset_cleanup_dialog_state(dialog_state: &mut CleanupBranchesDialogState) {
    dialog_state.stale_branches.clear();
    dialog_state.show_cleanup_branches_dialog = false;
}

fn reset_discard_dialog_state(dialog_state: &mut DiscardDialogState) {
    dialog_state.show_discard_dialog = false;
    dialog_state.discard_preview = None;
    dialog_state.discard_clean_untracked = false;
}

fn reset_file_action_dialog_state(dialog_state: &mut FileActionDialogState) {
    dialog_state.pending = None;
}

pub(super) fn reset_worktree_metadata_dialog_state(dialog_state: &mut WorktreeMetadataDialogState) {
    *dialog_state = WorktreeMetadataDialogState::default();
}

pub(super) fn reset_worktree_dialog_state(dialog_state: &mut WorktreeDialogState) {
    dialog_state.show_new_worktree_dialog = false;
    dialog_state.name.clear();
    dialog_state.branch.clear();
    dialog_state.base_branch = None;
    dialog_state.path.clear();
    dialog_state.path_parent.clear();
    dialog_state.focus_name_requested = false;
    dialog_state.branch_follows_name = true;
    dialog_state.path_follows_name = true;
    dialog_state.pending_remove = None;
}

pub(super) fn load_selected_file(
    worktree_state: &WorktreeState,
    inspector_state: &mut InspectorState,
    repo: &Repository,
    path: String,
    staged: bool,
) {
    let is_conflicted = worktree_state
        .unstaged
        .iter()
        .any(|file| file.path == path && file.is_conflicted());

    if is_conflicted {
        inspector_state.selected_file = Some(SelectedFile {
            path: path.clone(),
            staged: false,
        });
        match AppRepoRead::read_conflict_file(repo, &path) {
            Ok(conflict_data) => {
                inspector_state.set_conflict(Some(conflict_data));
                inspector_state.clear_diff();
            }
            Err(error) => {
                inspector_state.set_conflict(None);
                inspector_state.set_diff(format!("Error loading conflict data: {error}"));
            }
        }
        return;
    }

    inspector_state.set_conflict(None);
    match AppRepoRead::file_diff(repo, &path, staged) {
        Ok(diff) => inspector_state.set_diff(diff),
        Err(error) => inspector_state.set_diff(format!("Error loading diff: {error}")),
    }
    inspector_state.selected_file = Some(SelectedFile { path, staged });
}

/// Open a commit in the read-only commit view.
///
/// Metadata comes from the `CommitEntry` already loaded into `repo_state`, so
/// only the changed files and the first file's patch are read from git here.
/// Returns the failure detail when the commit's file list could not be read, so
/// the caller can log it — the view shows its own copy via
/// [`SelectedCommit::load_error`].
pub(super) fn load_selected_commit(
    repo_state: &RepoState,
    inspector_state: &mut InspectorState,
    repo: &Repository,
    oid: String,
) -> Option<String> {
    let Some(entry) = repo_state
        .commit_history
        .iter()
        .find(|commit| commit.oid == oid)
    else {
        inspector_state.set_commit(None);
        return Some(format!("commit {oid} is no longer in the loaded history"));
    };

    let mut commit = SelectedCommit {
        oid: entry.oid.clone(),
        short_oid: entry.short_oid.clone(),
        summary: entry.message.clone(),
        author: entry.author.clone(),
        time: entry.time.clone(),
        changes: ChangeSet::default(),
    };

    // An empty file list is a real outcome (an empty commit), so a failure here
    // has to be recorded rather than left to look like one.
    let detail = fill_files(
        &mut commit.changes,
        AppRepoRead::commit_changed_files(repo, &commit.oid),
    );

    inspector_state.set_commit(Some(commit));

    let first_path = inspector_state
        .selected_commit
        .as_ref()
        .and_then(|commit| commit.changes.files.first())
        .map(|file| file.path.clone());
    if let Some(path) = first_path {
        load_commit_file_diff(inspector_state, repo, path);
    }

    detail
}

/// Record a file list, keeping a read failure distinct from a genuinely empty
/// set. Returns the failure detail so the caller can log it.
fn fill_files<E: std::fmt::Display>(
    changes: &mut ChangeSet,
    result: Result<Vec<CommitFileChange>, E>,
) -> Option<String> {
    match result {
        Ok(files) => {
            changes.files = files;
            None
        }
        Err(error) => {
            let detail = error.to_string();
            changes.load_error = Some(detail.clone());
            Some(detail)
        }
    }
}

/// Parse a patch into the paired rows the side-by-side panes paint, tally its
/// added/removed lines, and open it.
///
/// Shared by the commit view and the review: only where the patch text came
/// from differs.
fn set_patch(changes: &mut ChangeSet, path: String, diff_content: String) {
    changes.diff_content = diff_content;

    let mut rows = parse_diff_rows(&changes.diff_content);
    changes.added_lines = rows
        .iter()
        .filter(|row| row.kind == DiffLineKind::Added)
        .count();
    changes.removed_lines = rows
        .iter()
        .filter(|row| row.kind == DiffLineKind::Removed)
        .count();

    // The `diff --git`/`index`/`---`/`+++` preamble describes the file as a
    // whole and the path is already in the view's header, so both panes start at
    // the first hunk instead.
    rows.retain(|row| row.kind != DiffLineKind::FileHeader);
    changes.diff_entries = to_side_by_side(&rows);

    changes.selected_path = Some(path);
    changes.scroll = 0.0;
}

/// Load one file's patch inside the currently open commit, parsed and paired
/// ready for rendering.
pub(super) fn load_commit_file_diff(
    inspector_state: &mut InspectorState,
    repo: &Repository,
    path: String,
) {
    let Some(commit) = inspector_state.selected_commit.as_mut() else {
        return;
    };

    let diff = match AppRepoRead::commit_file_diff(repo, &commit.oid, &path) {
        Ok(diff) => diff,
        Err(error) => format!("Error loading diff: {}", error),
    };

    set_patch(&mut commit.changes, path, diff);
}

/// Open a worktree for review: resolve its base, list what it has done since,
/// and show the first file.
pub(super) fn load_selected_review(
    repo_state: &RepoState,
    inspector_state: &mut InspectorState,
    worktree: &LinkedWorktree,
) -> Option<String> {
    let recorded = repo_state
        .worktree_metadata
        .get(&storage_key(worktree))
        .map(|metadata| metadata.base_commit.clone())
        .unwrap_or_default();

    let opened = match AppRepoRead::open(&worktree.path) {
        Ok(opened) => opened,
        Err(error) => {
            inspector_state.set_review(None);
            return Some(format!(
                "worktree '{}' could not be opened: {error}",
                worktree.name
            ));
        }
    };

    let base = match AppRepoRead::review_base(&opened, &recorded) {
        Ok(base) => base,
        Err(error) => {
            inspector_state.set_review(None);
            return Some(error.message().to_string());
        }
    };

    let mut review = SelectedReview {
        worktree_name: worktree.name.clone(),
        storage_key: storage_key(worktree),
        worktree_path: worktree.path.clone(),
        branch: worktree.branch.clone(),
        base,
        uncommitted: uncommitted_count(worktree),
        changes: ChangeSet::default(),
    };

    let detail = fill_files(
        &mut review.changes,
        AppRepoRead::review_changed_files(&opened, &review.base),
    );

    inspector_state.set_review(Some(review));

    let first_path = inspector_state
        .selected_review
        .as_ref()
        .and_then(|review| review.changes.files.first())
        .map(|file| file.path.clone());
    if let Some(path) = first_path {
        load_review_file_diff(inspector_state, path);
    }

    detail
}

/// Pick a worktree: record which one, then work out its base and totals.
///
/// Mirrors [`load_selected_review`], but keeps far less — see
/// [`SelectedWorktree`] for why branch, status and metadata are not cached.
pub(super) fn load_selected_worktree(
    repo_state: &RepoState,
    inspector_state: &mut InspectorState,
    worktree: &LinkedWorktree,
) -> Option<String> {
    let mut selected = SelectedWorktree {
        storage_key: storage_key(worktree),
        worktree_name: worktree.name.clone(),
        path: worktree.path.clone(),
        base: None,
        summary: None,
        load_error: None,
    };

    let detail = fill_worktree_summary(repo_state, &mut selected);
    inspector_state.set_selected_worktree(Some(selected));
    detail
}

/// Re-derive a selected worktree's base and totals from disk.
///
/// Shared by the click and by every refresh, so the number the details panel
/// shows can never mean something different from the one it showed a moment
/// ago. A failure is recorded on the selection rather than clearing it: a
/// checkout an agent deleted is exactly the row the view has to explain.
fn fill_worktree_summary(
    repo_state: &RepoState,
    selected: &mut SelectedWorktree,
) -> Option<String> {
    selected.base = None;
    selected.summary = None;
    selected.load_error = None;

    let recorded = repo_state
        .worktree_metadata
        .get(&selected.storage_key)
        .map(|metadata| metadata.base_commit.clone())
        .unwrap_or_default();

    let opened = match AppRepoRead::open(&selected.path) {
        Ok(opened) => opened,
        Err(error) => {
            let detail = format!(
                "worktree '{}' could not be opened: {error}",
                selected.worktree_name
            );
            return Some(selected.record_failure(
                "This checkout could not be opened. It may have been deleted; see Logs.",
                detail,
            ));
        }
    };

    // These messages are already written for a person to read.
    let base = match AppRepoRead::review_base(&opened, &recorded) {
        Ok(base) => base,
        Err(error) => {
            let detail = error.message().to_string();
            return Some(selected.record_failure(detail.clone(), detail));
        }
    };

    match AppRepoRead::review_summary(&opened, &base) {
        Ok(summary) => {
            selected.base = Some(base);
            selected.summary = Some(summary);
            None
        }
        Err(error) => {
            selected.base = Some(base);
            let detail = error.message().to_string();
            Some(selected.record_failure(detail.clone(), detail))
        }
    }
}

/// How much of a worktree's work is not committed yet, from the status the
/// listing already read — no second diff.
fn uncommitted_count(worktree: &LinkedWorktree) -> usize {
    match worktree.status {
        LinkedWorktreeStatus::Dirty {
            modified,
            staged,
            untracked,
        } => modified + staged + untracked,
        _ => 0,
    }
}

pub(super) fn load_review_file_diff(inspector_state: &mut InspectorState, path: String) {
    let Some(review) = inspector_state.selected_review.as_mut() else {
        return;
    };

    // Reopened per patch: the review's own worktree is a different repository
    // from the tab's, and nothing else here holds that handle.
    let diff = match AppRepoRead::open(&review.worktree_path) {
        Ok(opened) => match AppRepoRead::review_file_diff(&opened, &review.base, &path) {
            Ok(diff) => diff,
            Err(error) => format!("Error loading diff: {}", error),
        },
        Err(error) => format!("Error loading diff: {}", error),
    };

    set_patch(&mut review.changes, path, diff);
}

pub(super) fn repo_root_path(repo: &Repository) -> PathBuf {
    repo.workdir()
        .map(|path| path.to_path_buf())
        .unwrap_or_else(|| repo.path().parent().unwrap_or(repo.path()).to_path_buf())
}

pub(super) fn repo_tab_label(path: Option<&Path>) -> String {
    path.and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "Repository".into())
}

pub(super) fn default_repo_name_for_path(path: &Path) -> String {
    repo_tab_label(Some(path))
}

pub(super) fn status_message_for_error(context: &str, detail: &str) -> StatusMessage {
    StatusMessage::error(format!(
        "{} failed: {}. See Logs.",
        context,
        logging::summarize_for_ui(detail)
    ))
}

pub(super) const WORKER_DISPATCH_ERROR_DETAIL: &str = "worker rejected task dispatch";

pub(super) fn status_message_for_worker_dispatch(context: &str) -> StatusMessage {
    StatusMessage::error(format!("{context} could not start. Please try again."))
}

fn sync_pull_request_prompt(repo_state: &mut RepoState) {
    let keep_prompt = matches!(
        repo_state.pull_request_prompt.as_ref(),
        Some(PullRequestPrompt::Open { branch, .. } | PullRequestPrompt::Create { branch, .. })
            if branch == &repo_state.branch && repo_state.has_origin_remote
    );

    if !keep_prompt {
        repo_state.pull_request_prompt = None;
    }
}

/// Whether a selection made against a worktree survives this refresh.
///
/// Deliberately not "drop when the list is empty": `refresh_status` leaves
/// `linked_worktrees` empty when the *listing itself* failed (see the error arm
/// where it is read), and a transient git error must not throw away what the
/// user is reading. A refresh fires after every stage, unstage and worker
/// result, so this runs constantly.
///
/// Keyed on [`storage_key`] rather than the name: a linked worktree may legally
/// be called the same thing as the main one, and removing it would otherwise
/// look like the main worktree surviving in its place.
fn selection_survives_refresh(repo_state: &RepoState, key: &str) -> bool {
    repo_state.linked_worktrees.is_empty() || repo_state.worktree_by_key(key).is_some()
}

/// Drop an open review when its worktree is really gone.
fn sync_selected_review(repo_state: &RepoState, inspector_state: &mut InspectorState) {
    let Some(selected) = inspector_state.selected_review.as_ref() else {
        return;
    };

    if !selection_survives_refresh(repo_state, &selected.storage_key) {
        inspector_state.set_review(None);
    }
}

/// Drop the picked worktree when it is really gone, and otherwise re-total it.
///
/// The recompute's failure lands on the selection, never in `refresh_status`'s
/// error list: a worktree whose directory was deleted would otherwise raise an
/// error banner after every single stage and unstage.
fn sync_selected_worktree(repo_state: &RepoState, inspector_state: &mut InspectorState) {
    let Some(selected) = inspector_state.selected_worktree.as_mut() else {
        return;
    };

    if !selection_survives_refresh(repo_state, &selected.storage_key) {
        inspector_state.set_selected_worktree(None);
        return;
    }

    // Totals taken at click time go stale in exactly the situation this view
    // exists for: watching another checkout change while you work in this one.
    let _ = fill_worktree_summary(repo_state, selected);
}

fn sync_selected_commit(repo_state: &RepoState, inspector_state: &mut InspectorState) {
    let Some(selected) = inspector_state.selected_commit.as_ref() else {
        return;
    };

    let still_present = repo_state
        .commit_history
        .iter()
        .any(|commit| commit.oid == selected.oid);
    if !still_present {
        inspector_state.set_commit(None);
    }
}

fn sync_selected_file(
    worktree_state: &WorktreeState,
    inspector_state: &mut InspectorState,
    repo: &Repository,
) {
    let Some(selected) = inspector_state.selected_file.clone() else {
        inspector_state.set_conflict(None);
        return;
    };

    let in_unstaged = worktree_state
        .unstaged
        .iter()
        .any(|file| file.path == selected.path);
    let in_staged = worktree_state
        .staged
        .iter()
        .any(|file| file.path == selected.path);

    if !in_unstaged && !in_staged {
        inspector_state.selected_file = None;
        inspector_state.clear_diff();
        inspector_state.set_conflict(None);
        return;
    }

    // An open conflict holds unsaved decisions — per-line picks and hand edits
    // that exist nowhere else. Reloading it would silently discard them, and a
    // refresh fires after every stage/unstage and worker result, so touching an
    // unrelated file would wipe the merge in progress. Only reload once the file
    // stops being the conflict we are editing.
    let editing_this_conflict = inspector_state
        .conflict_data
        .as_ref()
        .is_some_and(|data| data.path == selected.path)
        && worktree_state
            .unstaged
            .iter()
            .any(|file| file.path == selected.path && file.is_conflicted());
    if editing_this_conflict {
        return;
    }

    let staged = if selected.staged && in_staged {
        true
    } else if !selected.staged && in_unstaged {
        false
    } else {
        in_staged && !in_unstaged
    };

    load_selected_file(worktree_state, inspector_state, repo, selected.path, staged);
}

#[cfg(test)]
mod tests {
    use super::{
        SelectedFile, load_selected_worktree, reset_inspector_state, status_message_for_error,
        status_message_for_worker_dispatch, sync_selected_file, sync_selected_review,
        sync_selected_worktree,
    };
    use crate::shared::conflicts::{ConflictChoice, ConflictData, ConflictPart, FileStyle};
    use crate::shared::git::{FileChangeKind, FileEntry};
    use crate::shared::review::{ReviewBase, ReviewBaseSource};
    use crate::shared::worktrees::{LinkedWorktree, LinkedWorktreeStatus};
    use crate::state::{ChangeSet, RepoState, SelectedReview, SelectedWorktree};
    use crate::state::{InspectorState, StatusLevel, WorktreeState};
    use crate::testutil::TestRepoDir;

    fn entry(path: &str, is_conflicted: bool) -> FileEntry {
        let kind = if is_conflicted {
            FileChangeKind::Conflicted
        } else {
            FileChangeKind::Modified
        };
        FileEntry {
            path: path.to_string(),
            display_status: if is_conflicted {
                "conflicted".to_string()
            } else {
                "modified".to_string()
            },
            kind,
        }
    }

    /// A conflict the user has already made a decision on.
    fn decided_conflict(path: &str) -> ConflictData {
        ConflictData::new(
            path.to_string(),
            vec![ConflictPart::Conflict {
                ours: "mine".into(),
                theirs: "yours".into(),
                resolution: ConflictChoice::Ours,
            }],
            FileStyle::default(),
        )
    }

    fn state_for(path: &str, is_conflicted: bool) -> (WorktreeState, InspectorState) {
        let worktree = WorktreeState {
            unstaged: vec![entry(path, is_conflicted)],
            ..Default::default()
        };

        let mut inspector = InspectorState {
            selected_file: Some(SelectedFile {
                path: path.to_string(),
                staged: false,
            }),
            ..Default::default()
        };
        inspector.set_conflict(Some(decided_conflict(path)));

        (worktree, inspector)
    }

    #[test]
    fn refresh_keeps_the_conflict_currently_being_resolved() {
        let dir = TestRepoDir::init();
        let repo = dir.open();
        let (worktree, mut inspector) = state_for("merge.txt", true);

        sync_selected_file(&worktree, &mut inspector, &repo);

        // A reload would have reset the section to Unresolved, silently throwing
        // away the user's choice. Refreshes fire after every stage/unstage.
        assert_eq!(
            inspector
                .conflict_data
                .as_ref()
                .map(|data| data.unresolved_count()),
            Some(0),
            "the in-progress resolution must survive a refresh"
        );
    }

    #[test]
    fn refresh_drops_the_conflict_once_the_file_is_no_longer_conflicted() {
        let dir = TestRepoDir::init();
        let repo = dir.open();
        // Same path, but the merge has been resolved and staged elsewhere.
        let (worktree, mut inspector) = state_for("merge.txt", false);

        sync_selected_file(&worktree, &mut inspector, &repo);

        assert!(
            inspector.conflict_data.is_none(),
            "a file that stopped being conflicted must leave the merge editor"
        );
    }

    #[test]
    fn failures_report_at_error_level_and_keep_the_summary() {
        let status = status_message_for_error("Push", "fatal: could not read Username");

        assert_eq!(status.level(), StatusLevel::Error);
        assert!(
            status.text().starts_with("Push failed: "),
            "unexpected text: {}",
            status.text()
        );
        assert!(status.text().ends_with("See Logs."));
    }

    #[test]
    fn a_worker_that_will_not_start_is_an_error_too() {
        let status = status_message_for_worker_dispatch("Pull");

        assert_eq!(status.level(), StatusLevel::Error);
        assert_eq!(status.text(), "Pull could not start. Please try again.");
    }

    #[test]
    fn refresh_drops_the_conflict_when_the_file_disappears() {
        let dir = TestRepoDir::init();
        let repo = dir.open();
        let (_worktree, mut inspector) = state_for("merge.txt", true);
        // Nothing left in the working tree — e.g. the merge was aborted.
        let worktree = WorktreeState::default();

        sync_selected_file(&worktree, &mut inspector, &repo);

        assert!(inspector.conflict_data.is_none());
        assert!(inspector.selected_file.is_none());
    }

    fn reviewed(name: &str) -> SelectedReview {
        SelectedReview {
            worktree_name: name.into(),
            storage_key: format!("wt:{name}"),
            worktree_path: std::path::PathBuf::from("/tmp").join(name),
            branch: Some("feature/x".into()),
            base: ReviewBase {
                oid: "a".repeat(40),
                short_oid: "aaaaaaa".into(),
                source: ReviewBaseSource::ForkPoint,
            },
            uncommitted: 0,
            changes: ChangeSet::default(),
        }
    }

    fn listed(name: &str) -> LinkedWorktree {
        LinkedWorktree {
            name: name.into(),
            path: std::path::PathBuf::from("/tmp").join(name),
            branch: None,
            head_short_oid: String::new(),
            is_main: false,
            is_current: false,
            is_locked: false,
            lock_reason: None,
            status: LinkedWorktreeStatus::Clean,
        }
    }

    /// A refresh fires after every stage, unstage and worker result, so an open
    /// review has to survive one.
    #[test]
    fn an_open_review_survives_an_ordinary_refresh() {
        let mut inspector = InspectorState::default();
        inspector.set_review(Some(reviewed("feature-auth")));
        let repo_state = RepoState {
            linked_worktrees: vec![listed("myapp"), listed("feature-auth")],
            ..RepoState::default()
        };

        sync_selected_review(&repo_state, &mut inspector);

        assert!(inspector.selected_review.is_some());
    }

    /// `refresh_status` leaves the list empty when the *listing itself* failed.
    /// A transient git error must not throw away what the user is reading.
    #[test]
    fn a_failed_listing_does_not_discard_an_open_review() {
        let mut inspector = InspectorState::default();
        inspector.set_review(Some(reviewed("feature-auth")));
        let repo_state = RepoState::default();

        sync_selected_review(&repo_state, &mut inspector);

        assert!(
            inspector.selected_review.is_some(),
            "an empty list means the refresh failed, not that the worktree is gone"
        );
    }

    #[test]
    fn a_review_is_dropped_once_its_worktree_really_goes() {
        let mut inspector = InspectorState::default();
        inspector.set_review(Some(reviewed("feature-auth")));
        let repo_state = RepoState {
            linked_worktrees: vec![listed("myapp")],
            ..RepoState::default()
        };

        sync_selected_review(&repo_state, &mut inspector);

        assert!(inspector.selected_review.is_none());
    }

    fn picked(name: &str) -> SelectedWorktree {
        SelectedWorktree {
            storage_key: format!("wt:{name}"),
            worktree_name: name.into(),
            // Nowhere in particular: the reconciliation tests never open it.
            path: std::path::PathBuf::from("/tmp/does-not-exist").join(name),
            base: None,
            summary: None,
            load_error: None,
        }
    }

    #[test]
    fn an_open_worktree_selection_survives_an_ordinary_refresh() {
        let mut inspector = InspectorState::default();
        inspector.set_selected_worktree(Some(picked("feature-auth")));
        let repo_state = RepoState {
            linked_worktrees: vec![listed("myapp"), listed("feature-auth")],
            ..RepoState::default()
        };

        sync_selected_worktree(&repo_state, &mut inspector);

        assert!(inspector.selected_worktree.is_some());
    }

    #[test]
    fn a_failed_listing_does_not_discard_the_selected_worktree() {
        let mut inspector = InspectorState::default();
        inspector.set_selected_worktree(Some(picked("feature-auth")));
        let repo_state = RepoState::default();

        sync_selected_worktree(&repo_state, &mut inspector);

        assert!(
            inspector.selected_worktree.is_some(),
            "an empty list means the refresh failed, not that the worktree is gone"
        );
    }

    #[test]
    fn a_selection_is_dropped_once_its_worktree_really_goes() {
        let mut inspector = InspectorState::default();
        inspector.set_selected_worktree(Some(picked("feature-auth")));
        let repo_state = RepoState {
            linked_worktrees: vec![listed("myapp")],
            ..RepoState::default()
        };

        sync_selected_worktree(&repo_state, &mut inspector);

        assert!(inspector.selected_worktree.is_none());
    }

    /// The two selections share one predicate on purpose. This is what stops
    /// them being copy-pasted apart later.
    #[test]
    fn the_review_and_the_worktree_selection_obey_the_same_rule() {
        let present = RepoState {
            linked_worktrees: vec![listed("myapp"), listed("feature-auth")],
            ..RepoState::default()
        };
        let gone = RepoState {
            linked_worktrees: vec![listed("myapp")],
            ..RepoState::default()
        };
        let listing_failed = RepoState::default();

        for (repo_state, expected) in [(&present, true), (&gone, false), (&listing_failed, true)] {
            let mut review_side = InspectorState::default();
            review_side.set_review(Some(reviewed("feature-auth")));
            sync_selected_review(repo_state, &mut review_side);

            let mut worktree_side = InspectorState::default();
            worktree_side.set_selected_worktree(Some(picked("feature-auth")));
            sync_selected_worktree(repo_state, &mut worktree_side);

            assert_eq!(review_side.selected_review.is_some(), expected);
            assert_eq!(
                worktree_side.selected_worktree.is_some(),
                expected,
                "the two selections disagreed about the same listing"
            );
        }
    }

    /// A linked worktree may legally be called the same thing as the repository
    /// folder, which is the only name the main worktree has. Matching on the
    /// name alone would let the main worktree stand in for a linked one that
    /// had just been removed.
    #[test]
    fn a_linked_worktree_named_like_the_main_one_is_not_mistaken_for_it() {
        let mut inspector = InspectorState::default();
        inspector.set_selected_worktree(Some(picked("myapp")));

        let mut main = listed("myapp");
        main.is_main = true;
        let repo_state = RepoState {
            // The linked `myapp` is gone; only the main worktree of the same
            // name is left.
            linked_worktrees: vec![main],
            ..RepoState::default()
        };

        sync_selected_worktree(&repo_state, &mut inspector);

        assert!(
            inspector.selected_worktree.is_none(),
            "the main worktree must not stand in for a removed linked one"
        );
    }

    #[test]
    fn resetting_the_inspector_forgets_the_selected_worktree() {
        let mut inspector = InspectorState::default();
        inspector.set_selected_worktree(Some(picked("feature-auth")));

        reset_inspector_state(&mut inspector);

        assert!(inspector.selected_worktree.is_none());
    }

    /// A checkout that is no longer on disk is exactly the row the Agents view
    /// has to explain, so it stays selected and records why instead.
    #[test]
    fn a_worktree_that_cannot_be_opened_stays_selected_and_records_why() {
        let mut inspector = InspectorState::default();
        inspector.set_selected_worktree(Some(picked("feature-auth")));
        let repo_state = RepoState {
            linked_worktrees: vec![listed("feature-auth")],
            ..RepoState::default()
        };

        sync_selected_worktree(&repo_state, &mut inspector);

        let selected = inspector
            .selected_worktree
            .as_ref()
            .expect("the selection must survive a checkout it cannot read");
        assert!(selected.base.is_none());
        assert!(selected.summary.is_none());
        assert!(
            selected.load_error.is_some(),
            "the panel has to be able to say what went wrong"
        );
    }

    /// The click path and the refresh path go through one function, so a real
    /// worktree gives the same answer either way.
    #[test]
    fn selecting_a_worktree_records_its_base_and_totals() {
        let repo_dir = TestRepoDir::init();
        repo_dir.write("README.md", "hello\n");
        let repo = repo_dir.open();
        crate::testutil::commit_all(&repo, "initial");

        let worktrees = TestRepoDir::empty();
        let destination = worktrees.path().join("wt");
        crate::infra::git::linked_worktrees::add_worktree(
            &repo,
            &crate::shared::worktrees::NewWorktreeRequest {
                name: "wt".into(),
                branch: "feature/wt".into(),
                base_branch: None,
                path: destination.clone(),
            },
        )
        .expect("add worktree");
        std::fs::write(destination.join("new.txt"), "a\nb\n").expect("write");

        let mut listed_wt = listed("wt");
        listed_wt.path = destination;
        let repo_state = RepoState {
            linked_worktrees: vec![listed_wt.clone()],
            ..RepoState::default()
        };
        let mut inspector = InspectorState::default();

        let detail = load_selected_worktree(&repo_state, &mut inspector, &listed_wt);

        assert!(detail.is_none(), "unexpected failure: {detail:?}");
        let selected = inspector.selected_worktree.as_ref().expect("selected");
        assert_eq!(selected.storage_key, "wt:wt");
        assert!(selected.base.is_some());
        let summary = selected.summary.expect("totals");
        assert_eq!(summary.files_changed, 1);
        assert_eq!(summary.insertions, 2);
        assert_eq!(summary.deletions, 0);
    }
}
