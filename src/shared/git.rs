#[derive(Clone, Debug)]
pub struct FileEntry {
    pub path: String,
    pub display_status: String,
    pub is_conflicted: bool,
}

/// One file touched by a commit, relative to that commit's first parent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitFileChange {
    pub path: String,
    /// Same vocabulary as [`FileEntry::display_status`] so both lists can share
    /// one status badge renderer.
    pub display_status: String,
}

#[derive(Clone, Debug)]
pub struct CommitEntry {
    /// Full 40-hex object id; the stable handle used to look the commit back up.
    pub oid: String,
    pub short_oid: String,
    pub message: String,
    pub author: String,
    pub time: String,
    pub is_merge: bool,
    pub branch_labels: Vec<String>,
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
