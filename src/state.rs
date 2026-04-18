use std::path::PathBuf;

use crate::shared::actions::UiAction;
use crate::shared::conflicts::ConflictData;
use crate::shared::git::{
    CommitEntry, CreateBranchPreview, DiscardPreview, FileEntry, StaleBranch,
};
use crate::shared::github::PullRequestPrompt;

#[derive(Clone, Debug)]
pub struct SelectedFile {
    pub path: String,
    pub staged: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum CenterView {
    #[default]
    Diff,
    History,
}

#[derive(Clone, Debug)]
pub struct DragFile {
    pub path: String,
    pub from_staged: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusyAction {
    Push,
    Pull,
    CreateTag,
    OpenPullRequest,
    CreatePullRequest,
    DiscardAndReset,
    GithubSignIn,
    PublishRepository,
    CloneRepository,
}

#[derive(Clone, Debug)]
pub struct BusyState {
    pub action: BusyAction,
    pub label: String,
}

impl BusyState {
    pub fn new(action: BusyAction, label: impl Into<String>) -> Self {
        Self {
            action,
            label: label.into(),
        }
    }
}

#[derive(Default)]
pub struct AppState {
    pub repo: RepoState,
    pub worktree: WorktreeState,
    pub inspector: InspectorState,
    pub commit: CommitState,
    pub dialogs: DialogState,
    pub ui: UiState,
}

impl AppState {
    pub fn refresh_parts_mut(
        &mut self,
    ) -> (
        &mut RepoState,
        &mut WorktreeState,
        &mut CommitState,
        &mut InspectorState,
        &mut UiState,
    ) {
        let Self {
            repo,
            worktree,
            commit,
            inspector,
            ui,
            ..
        } = self;
        (repo, worktree, commit, inspector, ui)
    }
}

#[derive(Default)]
pub struct RepoState {
    pub path: Option<PathBuf>,
    pub has_origin_remote: bool,
    pub has_github_origin: bool,
    pub has_github_https_origin: bool,
    pub branch: String,
    pub outgoing_commit_count: usize,
    pub branches: Vec<String>,
    pub commit_history: Vec<CommitEntry>,
    pub pull_request_prompt: Option<PullRequestPrompt>,
}

#[derive(Default)]
pub struct WorktreeState {
    pub unstaged: Vec<FileEntry>,
    pub staged: Vec<FileEntry>,
}

#[derive(Default)]
pub struct InspectorState {
    pub selected_file: Option<SelectedFile>,
    pub diff_content: String,
    pub diff_wrap: bool,
    pub center_view: CenterView,
    pub conflict_data: Option<ConflictData>,
    pub dragging: Option<DragFile>,
}

#[derive(Default)]
pub struct CommitState {
    pub inferred_commit_scopes: Vec<String>,
    pub commit_summary: String,
    pub commit_body: String,
    pub focus_commit_summary_requested: bool,
}

#[derive(Default)]
pub struct DialogState {
    pub branch: BranchDialogState,
    pub tag: TagDialogState,
    pub cleanup: CleanupBranchesDialogState,
    pub discard: DiscardDialogState,
}

#[derive(Default)]
pub struct BranchDialogState {
    pub new_branch_name: String,
    pub focus_new_branch_name_requested: bool,
    pub show_create_branch_dialog: bool,
    pub show_create_branch_confirm: bool,
    pub create_branch_preview: Option<CreateBranchPreview>,
    pub pending_new_branch_name: Option<String>,
}

#[derive(Default)]
pub struct TagDialogState {
    pub new_tag_name: String,
    pub focus_new_tag_name_requested: bool,
    pub show_create_tag_dialog: bool,
}

#[derive(Default)]
pub struct CleanupBranchesDialogState {
    pub stale_branches: Vec<StaleBranch>,
    pub show_cleanup_branches_dialog: bool,
}

#[derive(Default)]
pub struct DiscardDialogState {
    pub show_discard_dialog: bool,
    pub discard_preview: Option<DiscardPreview>,
    pub discard_clean_untracked: bool,
}

pub struct UiState {
    pub status_msg: String,
    pub actions: Vec<UiAction>,
    pub busy: Option<BusyState>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            status_msg: "No repository open".into(),
            actions: Vec::new(),
            busy: None,
        }
    }
}
