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
    OpenCleanupBranches,
    DeleteStaleBranches(Vec<String>),
    OpenDiscardDialog,
    DiscardAndReset { clean_untracked: bool },
    UndoLastCommit,
    SaveConflictResolution,
}

impl UiAction {
    pub fn stage_file(path: impl Into<String>) -> Self {
        Self::StageFile(path.into())
    }

    pub fn unstage_file(path: impl Into<String>) -> Self {
        Self::UnstageFile(path.into())
    }

    pub fn stage_all() -> Self {
        Self::StageAll
    }

    pub fn unstage_all() -> Self {
        Self::UnstageAll
    }

    pub fn commit() -> Self {
        Self::Commit
    }

    pub fn push() -> Self {
        Self::Push
    }

    pub fn pull() -> Self {
        Self::Pull
    }

    pub fn select_file(path: impl Into<String>, staged: bool) -> Self {
        Self::SelectFile {
            path: path.into(),
            staged,
        }
    }

    pub fn switch_branch(branch: impl Into<String>) -> Self {
        Self::SwitchBranch(branch.into())
    }

    pub fn create_branch(branch: impl Into<String>) -> Self {
        Self::CreateBranch(branch.into())
    }

    pub fn open_create_branch_confirm(branch: impl Into<String>) -> Self {
        Self::OpenCreateBranchConfirm(branch.into())
    }

    pub fn confirm_create_branch() -> Self {
        Self::ConfirmCreateBranch
    }

    pub fn create_tag(tag_name: impl Into<String>) -> Self {
        Self::CreateTag(tag_name.into())
    }

    pub fn launch_pull_request() -> Self {
        Self::LaunchPullRequest
    }

    pub fn show_diff() -> Self {
        Self::ShowDiff
    }

    pub fn show_history() -> Self {
        Self::ShowHistory
    }

    pub fn open_cleanup_branches() -> Self {
        Self::OpenCleanupBranches
    }

    pub fn delete_stale_branches(names: Vec<String>) -> Self {
        Self::DeleteStaleBranches(names)
    }

    pub fn open_discard_dialog() -> Self {
        Self::OpenDiscardDialog
    }

    pub fn discard_and_reset(clean_untracked: bool) -> Self {
        Self::DiscardAndReset { clean_untracked }
    }

    pub fn undo_last_commit() -> Self {
        Self::UndoLastCommit
    }

    pub fn save_conflict_resolution() -> Self {
        Self::SaveConflictResolution
    }
}
