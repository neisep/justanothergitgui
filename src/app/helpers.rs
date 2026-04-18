use super::*;
use crate::state::{
    BranchDialogState, CenterView, CleanupBranchesDialogState, CommitState, DialogState,
    DiscardDialogState, InspectorState, RepoState, SelectedFile, TagDialogState, UiState,
    WorktreeState,
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
    match AppRepoRead::commit_history(repo, 200) {
        Ok(history) => repo_state.commit_history = history,
        Err(error) => {
            errors.push(format!("commit history: {error}"));
            repo_state.commit_history = Vec::new();
        }
    }
    sync_pull_request_prompt(repo_state);
    sync_selected_file(worktree_state, inspector_state, repo);
    if errors.is_empty() {
        None
    } else {
        let detail = errors.join("; ");
        ui_state.status_msg = status_message_for_error("Refresh", &detail);
        Some(detail)
    }
}

pub(super) fn reset_repo_state(repo_state: &mut RepoState) {
    repo_state.has_origin_remote = false;
    repo_state.has_github_origin = false;
    repo_state.has_github_https_origin = false;
    repo_state.branch.clear();
    repo_state.outgoing_commit_count = 0;
    repo_state.branches.clear();
    repo_state.commit_history.clear();
    repo_state.pull_request_prompt = None;
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
    inspector_state.diff_content.clear();
    inspector_state.diff_wrap = false;
    inspector_state.center_view = CenterView::Diff;
    inspector_state.conflict_data = None;
    inspector_state.dragging = None;
}

pub(super) fn reset_dialog_state(dialog_state: &mut DialogState) {
    reset_branch_dialog_state(&mut dialog_state.branch);
    reset_tag_dialog_state(&mut dialog_state.tag);
    reset_cleanup_dialog_state(&mut dialog_state.cleanup);
    reset_discard_dialog_state(&mut dialog_state.discard);
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
        .any(|file| file.path == path && file.is_conflicted);

    if is_conflicted {
        inspector_state.selected_file = Some(SelectedFile {
            path: path.clone(),
            staged: false,
        });
        match AppRepoRead::read_conflict_file(repo, &path) {
            Ok(conflict_data) => {
                inspector_state.conflict_data = Some(conflict_data);
                inspector_state.diff_content.clear();
            }
            Err(error) => {
                inspector_state.conflict_data = None;
                inspector_state.diff_content = format!("Error loading conflict data: {}", error);
            }
        }
        return;
    }

    inspector_state.conflict_data = None;
    match AppRepoRead::file_diff(repo, &path, staged) {
        Ok(diff) => inspector_state.diff_content = diff,
        Err(error) => inspector_state.diff_content = format!("Error loading diff: {}", error),
    }
    inspector_state.selected_file = Some(SelectedFile { path, staged });
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

pub(super) fn status_message_for_error(context: &str, detail: &str) -> String {
    format!(
        "{} failed: {}. See Logs.",
        context,
        logging::summarize_for_ui(detail)
    )
}

pub(super) const WORKER_DISPATCH_ERROR_DETAIL: &str = "worker rejected task dispatch";

pub(super) fn status_message_for_worker_dispatch(context: &str) -> String {
    format!("{context} could not start. Please try again.")
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

fn sync_selected_file(
    worktree_state: &WorktreeState,
    inspector_state: &mut InspectorState,
    repo: &Repository,
) {
    let Some(selected) = inspector_state.selected_file.clone() else {
        inspector_state.conflict_data = None;
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
        inspector_state.diff_content.clear();
        inspector_state.conflict_data = None;
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
