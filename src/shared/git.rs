/// What git reports happened to a file, independent of the words the UI shows
/// for it.
///
/// [`FileEntry::display_status`] is derived from this and never the other way
/// round: the labels are display copy — the same [`FileChangeKind::Added`] reads
/// "untracked" in the unstaged list but "new" in the staged one — so anything
/// that has to *act* on a file, above all the destructive context-menu items,
/// matches on this instead of on the label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileChangeKind {
    /// Git has no committed copy of this path: `WT_NEW` unstaged, `INDEX_NEW`
    /// staged. Such a file can only be deleted, never restored.
    Added,
    Modified,
    Deleted,
    Renamed,
    /// A type change, or any flag combination the file lists do not name.
    TypeChange,
    Conflicted,
}

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub path: String,
    /// Display copy only — badge wording and colour. Never dispatch on this.
    pub display_status: String,
    pub kind: FileChangeKind,
}

impl FileEntry {
    pub fn is_conflicted(&self) -> bool {
        self.kind == FileChangeKind::Conflicted
    }
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
