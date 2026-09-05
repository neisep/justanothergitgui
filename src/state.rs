use std::path::PathBuf;

use crate::shared::actions::UiAction;
use crate::shared::conflicts::ConflictData;
use crate::shared::diff::{DiffLineKind, ParsedDiffLine, SideBySideEntry, parse_diff_rows};
use crate::shared::git::{
    CommitEntry, CommitFileChange, CreateBranchPreview, DiscardPreview, FileEntry, StaleBranch,
};
use crate::shared::github::PullRequestPrompt;

#[derive(Clone, Debug)]
pub struct SelectedFile {
    pub path: String,
    pub staged: bool,
}

/// A working-tree patch parsed for rendering: the rows the table paints plus the
/// tallies its header shows.
///
/// Built once per file selection. The Changes tab repaints many times a second
/// while the patch only changes when the selection does, and parsing allocates a
/// `String` per line — far too much to redo on the render path.
#[derive(Debug, Default)]
pub struct ParsedDiff {
    pub rows: Vec<ParsedDiffLine>,
    pub added_lines: usize,
    pub removed_lines: usize,
}

impl ParsedDiff {
    fn from_patch(content: &str) -> Self {
        let rows = parse_diff_rows(content);
        let added_lines = rows
            .iter()
            .filter(|row| row.kind == DiffLineKind::Added)
            .count();
        let removed_lines = rows
            .iter()
            .filter(|row| row.kind == DiffLineKind::Removed)
            .count();

        Self {
            rows,
            added_lines,
            removed_lines,
        }
    }
}

/// The commit currently open in the History tab's read-only commit view.
///
/// Holds everything that view renders, so it can be dropped in one move when the
/// user goes back to the list or the history it came from changes.
///
/// Deliberately not `Clone`: it owns the whole parsed patch, and nothing needs a
/// second copy — `set_commit` moves it in and out.
#[derive(Debug)]
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
    /// Why the commit's file list could not be read, if it could not.
    ///
    /// An empty `files` is also what a genuinely empty commit looks like, so the
    /// failure has to be recorded separately or the view reports a git error as
    /// "this commit changed no files".
    pub load_error: Option<String>,
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

/// How much attention a status message deserves.
///
/// Kept inside [`StatusMessage`] rather than beside it in [`UiState`] so the two
/// can never drift apart: every writer has to state the severity it means.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StatusLevel {
    #[default]
    Info,
    Success,
    Error,
}

/// The one-line message shown in the bottom bar, with the severity the status
/// area renders it at.
#[derive(Clone, Debug, Default)]
pub struct StatusMessage {
    text: String,
    level: StatusLevel,
}

impl StatusMessage {
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            level: StatusLevel::Info,
        }
    }

    pub fn success(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            level: StatusLevel::Success,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            level: StatusLevel::Error,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn level(&self) -> StatusLevel {
        self.level
    }
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
    /// `diff_content` parsed for the Changes tab. Kept in step with it by
    /// [`Self::set_diff`] / [`Self::clear_diff`], which are the only two ways in.
    pub parsed_diff: ParsedDiff,
    pub diff_wrap: bool,
    /// Substring the file panel narrows both of its lists by. View state only —
    /// it never reaches git.
    pub file_filter: String,
    pub center_view: CenterView,
    pub conflict_data: Option<ConflictData>,
    /// Which conflict (by section index) is open for inline editing, plus its
    /// draft text. `None` when no conflict is being hand-edited.
    pub conflict_edit: Option<ConflictEdit>,
    /// Shared vertical scroll offset for the two top merge-editor panes.
    pub conflict_scroll: f32,
    /// Ordinal of the conflict selected by Previous/Next navigation.
    pub conflict_focus: usize,
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
    /// Replace the working-tree patch, reparsing it in the same move so the two
    /// can never disagree about what is on screen.
    pub fn set_diff(&mut self, content: String) {
        self.parsed_diff = ParsedDiff::from_patch(&content);
        self.diff_content = content;
    }

    pub fn clear_diff(&mut self) {
        self.diff_content.clear();
        self.parsed_diff = ParsedDiff::default();
    }

    /// Set (or clear) the active conflict, resetting the inline-edit slot and
    /// shared scroll offset so no state leaks between files.
    pub fn set_conflict(&mut self, data: Option<ConflictData>) {
        self.conflict_data = data;
        self.conflict_edit = None;
        self.conflict_scroll = 0.0;
        self.conflict_focus = 0;
    }

    /// Shared by the save control and action handler so queued actions cannot
    /// write an earlier resolution while a draft is still being edited.
    pub fn resolution_save_error(&self) -> Option<&'static str> {
        if self.conflict_edit.is_some() {
            Some("Apply or cancel your edit before saving.")
        } else if self
            .conflict_data
            .as_ref()
            .is_none_or(|data| data.unresolved_count() > 0)
        {
            Some("Resolve every conflict before saving.")
        } else {
            None
        }
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
    pub status: StatusMessage,
    pub actions: Vec<UiAction>,
    pub busy: Option<BusyState>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            status: StatusMessage::info("No repository open"),
            actions: Vec::new(),
            busy: None,
        }
    }
}

#[cfg(test)]
mod merge_safety_tests {
    use super::*;
    use crate::shared::conflicts::{ConflictChoice, ConflictPart, FileStyle};

    #[test]
    fn open_draft_blocks_saving_even_when_previous_choice_is_resolved() {
        let mut inspector = InspectorState::default();
        inspector.set_conflict(Some(ConflictData::new(
            "test.txt".into(),
            vec![ConflictPart::Conflict {
                ours: "ours".into(),
                theirs: "theirs".into(),
                resolution: ConflictChoice::Ours,
            }],
            FileStyle::default(),
        )));
        assert!(inspector.resolution_save_error().is_none());
        inspector.conflict_edit = Some(ConflictEdit {
            index: 0,
            buffer: "unapplied".into(),
        });
        assert_eq!(
            inspector.resolution_save_error(),
            Some("Apply or cancel your edit before saving.")
        );
        inspector.conflict_edit = None;
        assert!(inspector.resolution_save_error().is_none());
        inspector
            .conflict_data
            .as_mut()
            .unwrap()
            .set_resolution(0, ConflictChoice::Unresolved);
        assert!(inspector.resolution_save_error().is_some());
        inspector.conflict_focus = 4;
        inspector.set_conflict(None);
        assert_eq!(inspector.conflict_focus, 0);
        assert!(inspector.resolution_save_error().is_some());
    }
}
