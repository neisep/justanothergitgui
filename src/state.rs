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
    CreateTag(String),
    LaunchPullRequest,
    ShowDiff,
    ShowHistory,
    SaveConflictResolution,
}

pub struct AppState {
    pub repo_path: Option<PathBuf>,
    pub has_origin_remote: bool,
    pub has_github_origin: bool,
    pub branch: String,
    pub branches: Vec<String>,
    pub new_branch_name: String,
    pub show_create_branch_dialog: bool,
    pub new_tag_name: String,
    pub show_create_tag_dialog: bool,
    pub unstaged: Vec<FileEntry>,
    pub staged: Vec<FileEntry>,
    pub commit_msg: String,
    pub status_msg: String,
    pub selected_file: Option<SelectedFile>,
    pub diff_content: String,
    pub actions: Vec<UiAction>,
    pub center_view: CenterView,
    pub commit_history: Vec<CommitEntry>,
    pub pull_request_prompt: Option<PullRequestPrompt>,
    pub conflict_data: Option<ConflictData>,
    pub dragging: Option<DragFile>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            repo_path: None,
            has_origin_remote: false,
            has_github_origin: false,
            branch: String::new(),
            branches: Vec::new(),
            new_branch_name: String::new(),
            show_create_branch_dialog: false,
            new_tag_name: String::new(),
            show_create_tag_dialog: false,
            unstaged: Vec::new(),
            staged: Vec::new(),
            commit_msg: String::new(),
            status_msg: "No repository open".into(),
            selected_file: None,
            diff_content: String::new(),
            actions: Vec::new(),
            center_view: CenterView::default(),
            commit_history: Vec::new(),
            pull_request_prompt: None,
            conflict_data: None,
            dragging: None,
        }
    }
}
