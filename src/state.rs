use std::path::PathBuf;

use crate::shared::actions::{PendingFileAction, UiAction};
use crate::shared::conflicts::ConflictData;
use crate::shared::diff::{DiffLineKind, ParsedDiffLine, SideBySideEntry, parse_diff_rows};
use crate::shared::git::{
    CommitEntry, CommitFileChange, CreateBranchPreview, DiscardPreview, FileEntry, StaleBranch,
};
use crate::shared::github::PullRequestPrompt;
use crate::shared::review::{ReviewBase, ReviewSummary};
use crate::shared::worktree_metadata::{
    ReviewState, TestState, WorktreeMetadata, WorktreeMetadataMap, storage_key,
};
use crate::shared::worktrees::{LinkedWorktree, NewWorktreeRequest};

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

/// A set of changed files with one of them open, as the read-only inspectors
/// render it.
///
/// Shared by the commit view and the worktree review: both are "here are the
/// files that differ, and here is the patch for the one you picked", and the
/// only thing that differs is where the set came from. Keeping one struct keeps
/// one renderer, so the two views cannot drift apart.
///
/// Deliberately not `Clone`: it owns the whole parsed patch, and nothing needs a
/// second copy.
#[derive(Debug, Default)]
pub struct ChangeSet {
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
    /// Why the file list could not be read, if it could not.
    ///
    /// An empty `files` is also what a genuinely empty change set looks like, so
    /// the failure has to be recorded separately or the view reports a git error
    /// as "nothing changed here".
    pub load_error: Option<String>,
}

/// The commit currently open in the History tab's read-only commit view.
///
/// Holds everything that view renders, so it can be dropped in one move when the
/// user goes back to the list or the history it came from changes.
#[derive(Debug)]
pub struct SelectedCommit {
    pub oid: String,
    pub short_oid: String,
    pub summary: String,
    pub author: String,
    pub time: String,
    pub changes: ChangeSet,
}

/// The worktree currently open in the Review tab.
///
/// Identified by worktree name rather than a commit: the "to" side of a review
/// is the worktree as it stands on disk, which has no oid.
#[derive(Debug)]
pub struct SelectedReview {
    pub worktree_name: String,
    /// Its [`crate::shared::worktree_metadata::storage_key`]. The reconciliation
    /// key, because a linked worktree may legally be called the same thing as
    /// the main one and the name alone cannot tell them apart.
    pub storage_key: String,
    /// The checkout's own directory. Held because each patch reopens it: a
    /// worktree is a different repository from the tab's, and nothing else in
    /// the inspector holds that handle.
    pub worktree_path: PathBuf,
    pub branch: Option<String>,
    pub base: ReviewBase,
    /// How many of the reviewed files are still uncommitted, taken from the
    /// worktree's own status rather than a second diff.
    pub uncommitted: usize,
    pub changes: ChangeSet,
}

/// The worktree the user picked in the Agents view, or in the sidebar.
///
/// Holds identity plus only those facts that need a `Repository` to work out —
/// the base commit and the totals since it. Branch, status and metadata are
/// deliberately **not** cached here: `refresh_status` re-reads all three every
/// refresh, so a copy taken at click time would make the details panel
/// contradict the row beside it, and would make an edit to the metadata look
/// like it had not been saved. Both readers look the live row up with
/// [`RepoState::worktree_by_key`].
#[derive(Debug)]
pub struct SelectedWorktree {
    /// Its [`crate::shared::worktree_metadata::storage_key`]: the metadata key
    /// and the reconciliation key in one.
    pub storage_key: String,
    /// Git's own name for it, for messages.
    pub worktree_name: String,
    /// The checkout's own directory, reopened on every refresh to re-total it.
    pub path: PathBuf,
    /// `None` when the base could not be resolved — an unborn HEAD, or a
    /// checkout that is no longer on disk. [`Self::load_error`] says which.
    pub base: Option<ReviewBase>,
    pub summary: Option<ReviewSummary>,
    /// Why the base or the totals could not be read.
    ///
    /// Kept apart from an absent summary for the reason [`ChangeSet::load_error`]
    /// is: a worktree whose directory was deleted must read as "could not be
    /// read", not as "nothing changed here".
    pub load_error: Option<String>,
}

impl SelectedWorktree {
    /// Record why this worktree could not be read, and hand the detail back for
    /// the caller to log. The selection itself is kept: a checkout that cannot
    /// be opened is a row the view has to explain, not one to silently drop.
    ///
    /// The two strings are deliberately different. `display` goes in the panel
    /// and has to fit a 300px column in a sentence a person can act on;
    /// `detail` carries the raw git message, which is worth keeping in the log
    /// and is five lines of path and error codes on screen.
    pub fn record_failure(&mut self, display: impl Into<String>, detail: String) -> String {
        self.load_error = Some(display.into());
        detail
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CenterView {
    #[default]
    Diff,
    History,
    Review,
    Agents,
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
    CreateWorktree,
    RemoveWorktree,
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
    pub remote_branches: Vec<String>,
    pub commit_history: Vec<CommitEntry>,
    pub pull_request_prompt: Option<PullRequestPrompt>,
    /// What the user recorded about this repository's worktrees, keyed by
    /// [`storage_key`]. Re-read from disk on every refresh, not cached across
    /// them: several tabs can be showing one repository, and the file is the
    /// only thing they share. Written back one entry at a time on edit.
    pub worktree_metadata: WorktreeMetadataMap,
    /// Every checkout sharing this repository's object store, main tree first.
    ///
    /// Lives on [`RepoState`] rather than beside the file lists because it
    /// describes the repository, not this tab's working tree — and because the
    /// panel has to keep showing the *other* worktrees while this one changes.
    pub linked_worktrees: Vec<LinkedWorktree>,
}

impl RepoState {
    /// The listed worktree with this [`storage_key`], as the last refresh read
    /// it.
    ///
    /// The way every view resolves a remembered selection back to live data,
    /// rather than holding a copy that quietly goes stale.
    pub fn worktree_by_key(&self, key: &str) -> Option<&LinkedWorktree> {
        self.linked_worktrees
            .iter()
            .find(|worktree| storage_key(worktree) == key)
    }
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
    /// Worktree opened in the Review tab, or `None` while nothing is under
    /// review.
    pub selected_review: Option<SelectedReview>,
    /// Worktree the user picked in the Agents view or the sidebar, or `None`
    /// while nothing is picked.
    ///
    /// Independent of [`Self::selected_review`]: picking a worktree to read
    /// about is not the same act as opening its diff, and the two views can
    /// legitimately be pointed at different worktrees.
    pub selected_worktree: Option<SelectedWorktree>,
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

    /// Open (or close) the Review tab's view, on the same terms.
    pub fn set_review(&mut self, review: Option<SelectedReview>) {
        self.selected_review = review;
    }

    /// Pick (or unpick) the worktree the Agents view and the sidebar follow.
    pub fn set_selected_worktree(&mut self, selected: Option<SelectedWorktree>) {
        self.selected_worktree = selected;
    }

    /// The picked worktree's storage key, for the panels that only need to know
    /// which row is highlighted.
    pub fn selected_worktree_key(&self) -> Option<&str> {
        self.selected_worktree
            .as_ref()
            .map(|selected| selected.storage_key.as_str())
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
    pub file_action: FileActionDialogState,
    pub worktree: WorktreeDialogState,
    pub worktree_metadata: WorktreeMetadataDialogState,
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

/// Draft state for the New Worktree form and the removal confirmation.
///
/// The two share a struct because they are one feature's dialogs and are reset
/// together; they are never open at the same time in practice, but nothing
/// depends on that.
#[derive(Default)]
pub struct WorktreeDialogState {
    pub show_new_worktree_dialog: bool,
    pub name: String,
    pub branch: String,
    /// `None` means "branch from the current HEAD".
    pub base_branch: Option<String>,
    pub path: String,
    /// Folder new worktrees are proposed inside. Kept apart from [`Self::path`]
    /// because deriving it back out of the path is lossy: an empty name leaves a
    /// trailing separator, and `Path::parent` then strips the folder itself.
    pub path_parent: String,
    pub focus_name_requested: bool,
    /// Whether [`Self::branch`] and [`Self::path`] still mirror the name, so
    /// typing a name keeps them in step until the user edits them directly.
    /// Filling one field is the common case; overwriting a typed value is never
    /// right.
    pub branch_follows_name: bool,
    pub path_follows_name: bool,
    /// Open exactly while this holds the worktree awaiting removal confirmation.
    pub pending_remove: Option<LinkedWorktree>,
}

impl WorktreeDialogState {
    pub fn request(&self) -> NewWorktreeRequest {
        NewWorktreeRequest {
            name: self.name.trim().to_string(),
            branch: self.branch.trim().to_string(),
            base_branch: self.base_branch.clone(),
            path: PathBuf::from(self.path.trim()),
        }
    }
}

/// Draft state for the worktree metadata form.
///
/// Open exactly while `editing` names the worktree being edited, the way
/// [`FileActionDialogState::pending`] gates its own dialog.
#[derive(Default)]
pub struct WorktreeMetadataDialogState {
    /// The worktree's display name, and what makes the dialog open.
    pub editing: Option<String>,
    /// Its [`crate::shared::worktree_metadata::storage_key`], captured when the
    /// dialog opened so the write cannot drift onto a different worktree.
    pub key: String,
    /// Why the last save failed, shown in the form rather than only the status
    /// bar, so the user can see it while fixing it.
    pub save_error: Option<String>,
    pub task: String,
    pub agent: String,
    pub notes: String,
    /// Shown read-only: the app records this when it creates a worktree and
    /// there is nothing sensible for the user to type here.
    pub base_commit: String,
    /// Shown read-only, for the same reason as [`Self::base_commit`], and
    /// carried through [`WorktreeMetadataDialogState::metadata`] so saving the
    /// form cannot silently forget when the worktree began.
    pub started: i64,
    pub review: ReviewState,
    pub test: TestState,
    pub focus_task_requested: bool,
}

impl WorktreeMetadataDialogState {
    /// Fill the form from a worktree's stored metadata.
    pub fn open(&mut self, name: &str, metadata: &WorktreeMetadata) {
        self.editing = Some(name.to_string());
        self.save_error = None;
        self.task = metadata.task.clone();
        self.agent = metadata.agent.clone();
        self.notes = metadata.notes.clone();
        self.base_commit = metadata.base_commit.clone();
        self.started = metadata.started;
        self.review = metadata.review;
        self.test = metadata.test;
        self.focus_task_requested = true;
    }

    /// What the form currently describes.
    pub fn metadata(&self) -> WorktreeMetadata {
        WorktreeMetadata {
            task: self.task.trim().to_string(),
            agent: self.agent.trim().to_string(),
            // Only the trailing whitespace goes: notes are prose, and the
            // paragraph breaks inside them are the user's.
            notes: self.notes.trim_end().to_string(),
            base_commit: self.base_commit.clone(),
            started: self.started,
            review: self.review,
            test: self.test,
        }
    }
}

/// Open exactly while `pending` holds the file operation awaiting confirmation.
#[derive(Default)]
pub struct FileActionDialogState {
    pub pending: Option<PendingFileAction>,
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
mod worktree_metadata_form_tests {
    use super::*;

    /// The form shows `base_commit` and `started` but cannot edit either, so
    /// [`WorktreeMetadataDialogState::metadata`] has to carry them through.
    /// Rebuilding the struct without them would silently forget where and when
    /// the worktree began, on every save.
    #[test]
    fn a_save_carries_the_fields_the_form_cannot_edit() {
        let stored = WorktreeMetadata {
            task: "Add OAuth login".into(),
            agent: "claude".into(),
            notes: "Waiting on review.".into(),
            base_commit: "a1b2c3d4e5".into(),
            started: 1_700_000_000,
            review: ReviewState::NeedsReview,
            test: TestState::Passing,
        };

        let mut dialog = WorktreeMetadataDialogState::default();
        dialog.open("feature-auth", &stored);
        // The user edits only what the form exposes.
        dialog.review = ReviewState::Approved;

        let saved = dialog.metadata();

        assert_eq!(saved.base_commit, stored.base_commit);
        assert_eq!(saved.started, stored.started);
        assert_eq!(saved.notes, stored.notes);
        assert_eq!(saved.review, ReviewState::Approved);
    }

    /// Notes are prose: the paragraph breaks inside them belong to the user, and
    /// only the trailing whitespace a text box collects is dropped.
    #[test]
    fn saving_notes_keeps_their_internal_line_breaks() {
        let dialog = WorktreeMetadataDialogState {
            notes: "First line.\n\nSecond line.\n\n".into(),
            ..WorktreeMetadataDialogState::default()
        };

        assert_eq!(dialog.metadata().notes, "First line.\n\nSecond line.");
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
