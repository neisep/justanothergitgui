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

pub struct AppState {
    pub repo_path: Option<PathBuf>,
    pub has_origin_remote: bool,
    pub has_github_origin: bool,
    pub has_github_https_origin: bool,
    pub branch: String,
    pub outgoing_commit_count: usize,
    pub branches: Vec<String>,
    pub new_branch_name: String,
    pub focus_new_branch_name_requested: bool,
    pub show_create_branch_dialog: bool,
    pub show_create_branch_confirm: bool,
    pub create_branch_preview: Option<CreateBranchPreview>,
    pub pending_new_branch_name: Option<String>,
    pub new_tag_name: String,
    pub focus_new_tag_name_requested: bool,
    pub show_create_tag_dialog: bool,
    pub stale_branches: Vec<StaleBranch>,
    pub show_cleanup_branches_dialog: bool,
    pub show_discard_dialog: bool,
    pub discard_preview: Option<DiscardPreview>,
    pub discard_clean_untracked: bool,
    pub unstaged: Vec<FileEntry>,
    pub staged: Vec<FileEntry>,
    pub inferred_commit_scopes: Vec<String>,
    pub commit_summary: String,
    pub commit_body: String,
    pub focus_commit_summary_requested: bool,
    pub status_msg: String,
    pub selected_file: Option<SelectedFile>,
    pub diff_content: String,
    pub diff_wrap: bool,
    pub actions: Vec<UiAction>,
    pub center_view: CenterView,
    pub commit_history: Vec<CommitEntry>,
    pub pull_request_prompt: Option<PullRequestPrompt>,
    pub conflict_data: Option<ConflictData>,
    pub dragging: Option<DragFile>,
    pub busy: Option<BusyState>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            repo_path: None,
            has_origin_remote: false,
            has_github_origin: false,
            has_github_https_origin: false,
            branch: String::new(),
            outgoing_commit_count: 0,
            branches: Vec::new(),
            new_branch_name: String::new(),
            focus_new_branch_name_requested: false,
            show_create_branch_dialog: false,
            show_create_branch_confirm: false,
            create_branch_preview: None,
            pending_new_branch_name: None,
            new_tag_name: String::new(),
            focus_new_tag_name_requested: false,
            show_create_tag_dialog: false,
            stale_branches: Vec::new(),
            show_cleanup_branches_dialog: false,
            show_discard_dialog: false,
            discard_preview: None,
            discard_clean_untracked: false,
            unstaged: Vec::new(),
            staged: Vec::new(),
            inferred_commit_scopes: Vec::new(),
            commit_summary: String::new(),
            commit_body: String::new(),
            focus_commit_summary_requested: false,
            status_msg: "No repository open".into(),
            selected_file: None,
            diff_content: String::new(),
            diff_wrap: false,
            actions: Vec::new(),
            center_view: CenterView::default(),
            commit_history: Vec::new(),
            pull_request_prompt: None,
            conflict_data: None,
            dragging: None,
            busy: None,
        }
    }
}
