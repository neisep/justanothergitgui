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

/// How long ago an instant was, in the vocabulary the history list already uses.
///
/// Both arguments are Unix seconds. Lives here rather than beside its first
/// caller because a commit's age and a worktree's age must read the same way,
/// and the two are computed in different layers.
///
/// Deliberately coarse: the app bundles no timezone database, so a wall-clock
/// time of day is not reachable, and "3h ago" is what the reader wanted anyway.
pub fn relative_time(now_secs: i64, then_secs: i64) -> String {
    let diff = now_secs - then_secs;
    if diff < 0 {
        return "in the future".into();
    }
    if diff < 60 {
        return "just now".into();
    }
    if diff < 3600 {
        return format!("{}m ago", diff / 60);
    }
    if diff < 86400 {
        return format!("{}h ago", diff / 3600);
    }
    if diff < 2592000 {
        return format!("{}d ago", diff / 86400);
    }
    format!("{}mo ago", diff / 2592000)
}

#[cfg(test)]
mod tests {
    use super::relative_time;

    /// The thresholds are display copy the history list has always shown; a
    /// worktree's "started" line now shares them.
    #[test]
    fn relative_time_names_the_largest_unit_that_fits() {
        assert_eq!(relative_time(1_000, 1_000), "just now");
        assert_eq!(relative_time(1_059, 1_000), "just now");
        assert_eq!(relative_time(1_060, 1_000), "1m ago");
        assert_eq!(relative_time(1_000 + 3_600, 1_000), "1h ago");
        assert_eq!(relative_time(1_000 + 86_400, 1_000), "1d ago");
        assert_eq!(relative_time(1_000 + 2_592_000, 1_000), "1mo ago");
    }

    /// A clock that went backwards must not underflow into a huge age.
    #[test]
    fn an_instant_in_the_future_says_so() {
        assert_eq!(relative_time(1_000, 2_000), "in the future");
    }
}
