use std::path::PathBuf;

use crate::shared::actions::UiAction;
use crate::shared::conflicts::ConflictData;
use crate::shared::diff::SideBySideEntry;
use crate::shared::git::{
    CommitEntry, CommitFileChange, CreateBranchPreview, DiscardPreview, FileEntry, StaleBranch,
};
use crate::shared::github::PullRequestPrompt;

#[derive(Clone, Debug)]
pub struct SelectedFile {
    pub path: String,
    pub staged: bool,
}

/// The commit currently open in the History tab's read-only commit view.
///
/// Holds everything that view renders, so it can be dropped in one move when the
/// user goes back to the list or the history it came from changes.
#[derive(Clone, Debug)]
pub struct SelectedCommit {
    pub oid: String,
    pub short_oid: String,
    pub summary: String,
    pub author: String,
    pub time: String,
    pub files: Vec<CommitFileChange>,
    pub selected_path: Option<String>,
    pub diff_content: String,
    /// `diff_content` parsed and paired for the side-by-side panes.
    ///
    /// Built once per file selection: parsing allocates a `String` per line and
    /// pairing clones each of those again, which is far too much to redo on
    /// every frame of a repaint.
    pub diff_entries: Vec<SideBySideEntry>,
    /// Shared vertical scroll offset for the two read-only diff panes.
    pub scroll: f32,
    /// Added/removed line counts for the header, tallied with the parse.
    pub added_lines: usize,
    pub removed_lines: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
    UndoLastCommit,
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
    /// Which conflict (by section index) is open for inline editing, plus its
    /// draft text. `None` when no conflict is being hand-edited.
    pub conflict_edit: Option<ConflictEdit>,
    /// Shared vertical scroll offset for the two top merge-editor panes.
    pub conflict_scroll: f32,
    /// Commit opened from the History tab, or `None` while the list is showing.
    pub selected_commit: Option<SelectedCommit>,
    pub dragging: Option<DragFile>,
}

/// Draft state for editing one conflict's resolution text inline.
#[derive(Clone, Debug)]
pub struct ConflictEdit {
    pub index: usize,
    pub buffer: String,
}

impl InspectorState {
    /// Set (or clear) the active conflict, resetting the inline-edit slot and
    /// shared scroll offset so no state leaks between files.
    pub fn set_conflict(&mut self, data: Option<ConflictData>) {
        self.conflict_data = data;
        self.conflict_edit = None;
        self.conflict_scroll = 0.0;
    }

    /// Open (or close) the read-only commit view. The whole view state travels
    /// with the commit, so nothing leaks between commits.
    pub fn set_commit(&mut self, commit: Option<SelectedCommit>) {
        self.selected_commit = commit;
    }
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
