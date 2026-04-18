use super::*;
use crate::state::{CenterView, SelectedFile};

pub(super) fn refresh_status(state: &mut AppState, repo: &Repository) -> Option<String> {
    let mut error_detail = None;
    state.has_origin_remote = git_ops::has_origin_remote(repo);
    state.has_github_origin = git_ops::has_github_origin(repo);
    state.has_github_https_origin = git_ops::has_github_https_origin(repo);
    state.outgoing_commit_count = git_ops::get_outgoing_commit_count(repo).unwrap_or(0);
    match git_ops::get_file_statuses(repo) {
        Ok((unstaged, staged)) => {
            state.unstaged = unstaged;
            state.staged = staged;
            let changed_paths = if state.staged.is_empty() {
                state
                    .unstaged
                    .iter()
                    .map(|file| file.path.as_str())
                    .collect::<Vec<_>>()
            } else {
                state
                    .staged
                    .iter()
                    .map(|file| file.path.as_str())
                    .collect::<Vec<_>>()
            };
            state.inferred_commit_scopes =
                commit_rules::infer_commit_scopes(state.repo_path.as_deref(), changed_paths);
        }
        Err(error) => {
            let detail = error.to_string();
            state.status_msg = status_message_for_error("Refresh", &detail);
            error_detail = Some(detail);
            state.inferred_commit_scopes.clear();
        }
    }
    state.branch = git_ops::get_current_branch(repo).unwrap_or_default();
    state.branches = git_ops::get_branches(repo).unwrap_or_default();
    state.commit_history = git_ops::get_commit_history(repo, 200).unwrap_or_default();
    sync_pull_request_prompt(state);
    sync_selected_file(state, repo);
    error_detail
}

pub(super) fn reset_repo_view_state(state: &mut AppState) {
    state.has_origin_remote = false;
    state.has_github_origin = false;
    state.has_github_https_origin = false;
    state.branch.clear();
    state.outgoing_commit_count = 0;
    state.branches.clear();
    state.new_branch_name.clear();
    state.focus_new_branch_name_requested = false;
    state.show_create_branch_dialog = false;
    state.show_create_branch_confirm = false;
    state.create_branch_preview = None;
    state.pending_new_branch_name = None;
    state.new_tag_name.clear();
    state.focus_new_tag_name_requested = false;
    state.show_create_tag_dialog = false;
    state.stale_branches.clear();
    state.show_cleanup_branches_dialog = false;
    state.show_discard_dialog = false;
    state.discard_preview = None;
    state.discard_clean_untracked = false;
    state.unstaged.clear();
    state.staged.clear();
    state.inferred_commit_scopes.clear();
    state.commit_summary.clear();
    state.commit_body.clear();
    state.focus_commit_summary_requested = false;
    state.selected_file = None;
    state.diff_content.clear();
    state.actions.clear();
    state.center_view = CenterView::Diff;
    state.commit_history.clear();
    state.pull_request_prompt = None;
    state.conflict_data = None;
    state.dragging = None;
    state.busy = None;
}

pub(super) fn load_selected_file(
    state: &mut AppState,
    repo: &Repository,
    path: String,
    staged: bool,
) {
    let is_conflicted = state
        .unstaged
        .iter()
        .any(|file| file.path == path && file.is_conflicted);

    if is_conflicted {
        state.selected_file = Some(SelectedFile {
            path: path.clone(),
            staged: false,
        });
        match git_ops::read_conflict_file(repo, &path) {
            Ok(conflict_data) => {
                state.conflict_data = Some(conflict_data);
                state.diff_content.clear();
            }
            Err(error) => {
                state.conflict_data = None;
                state.diff_content = format!("Error loading conflict data: {}", error);
            }
        }
        return;
    }

    state.conflict_data = None;
    match git_ops::get_file_diff(repo, &path, staged) {
        Ok(diff) => state.diff_content = diff,
        Err(error) => state.diff_content = format!("Error loading diff: {}", error),
    }
    state.selected_file = Some(SelectedFile { path, staged });
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

fn sync_pull_request_prompt(state: &mut AppState) {
    let keep_prompt = matches!(
        state.pull_request_prompt.as_ref(),
        Some(PullRequestPrompt::Open { branch, .. } | PullRequestPrompt::Create { branch, .. })
            if branch == &state.branch && state.has_origin_remote
    );

    if !keep_prompt {
        state.pull_request_prompt = None;
    }
}

fn sync_selected_file(state: &mut AppState, repo: &Repository) {
    let Some(selected) = state.selected_file.clone() else {
        state.conflict_data = None;
        return;
    };

    let in_unstaged = state.unstaged.iter().any(|file| file.path == selected.path);
    let in_staged = state.staged.iter().any(|file| file.path == selected.path);

    if !in_unstaged && !in_staged {
        state.selected_file = None;
        state.diff_content.clear();
        state.conflict_data = None;
        return;
    }

    let staged = if selected.staged && in_staged {
        true
    } else if !selected.staged && in_unstaged {
        false
    } else {
        in_staged && !in_unstaged
    };

    load_selected_file(state, repo, selected.path, staged);
}
