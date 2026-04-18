use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub path: String,
    pub display_status: String,
    pub is_conflicted: bool,
}

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
pub struct CommitEntry {
    pub short_oid: String,
    pub message: String,
    pub author: String,
    pub time: String,
    pub is_merge: bool,
    pub branch_labels: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum PullRequestPrompt {
    Open {
        branch: String,
        number: u64,
        url: String,
    },
    Create {
        branch: String,
        url: String,
    },
}

#[derive(Clone, Debug)]
pub enum ConflictPart {
    Common(String),
    Conflict {
        ours: String,
        theirs: String,
        resolution: ConflictChoice,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum ConflictChoice {
    #[default]
    Unresolved,
    Ours,
    Theirs,
    Both,
}

#[derive(Clone, Debug)]
pub struct ConflictData {
    pub path: String,
    pub sections: Vec<ConflictPart>,
}

#[derive(Clone, Debug)]
pub struct DragFile {
    pub path: String,
    pub from_staged: bool,
}

#[derive(Clone, Debug)]
pub struct StaleBranch {
    pub name: String,
    pub merged_into_head: bool,
    pub selected: bool,
}

#[derive(Clone, Debug, Default)]
pub struct DiscardPreview {
    pub dirty_files: usize,
    pub untracked_files: usize,
    pub local_only_commits: usize,
}

#[derive(Clone, Debug, Default)]
pub struct CreateBranchPreview {
    pub branch_name: String,
    pub dirty_files: usize,
    pub untracked_files: usize,
    pub staged_files: usize,
}

pub enum UiAction {
    StageFile(String),
    UnstageFile(String),
    StageAll,
    UnstageAll,
    Commit,
    Push,
    Pull,
    SelectFile { path: String, staged: bool },
    SwitchBranch(String),
    CreateBranch(String),
    OpenCreateBranchConfirm(String),
    ConfirmCreateBranch,
    CreateTag(String),
    LaunchPullRequest,
    ShowDiff,
    ShowHistory,
    SaveConflictResolution,
    OpenCleanupBranches,
    DeleteStaleBranches(Vec<String>),
    OpenDiscardDialog,
    DiscardAndReset { clean_untracked: bool },
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
    ListGithubRepos,
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
