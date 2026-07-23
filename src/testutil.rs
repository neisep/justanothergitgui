//! Shared test-support helpers.
//!
//! Compiled only under `cfg(test)`. Owns the temporary-git-repository fixture and
//! commit/signature helpers that were previously duplicated across the infra and
//! `git_ops` test modules, so every test builds its repositories the same way.
#![allow(dead_code)]

use git2::{Repository, RepositoryInitOptions, Signature, Time};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Fixed timestamp shared by test commits so history ordering is deterministic.
pub const FIXED_TIMESTAMP: i64 = 1_700_000_000;

/// A temporary directory holding a git repository, removed on drop.
pub struct TestRepoDir {
    path: PathBuf,
}

impl TestRepoDir {
    fn make_dir(area: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "justanothergitgui-{}-test-{}-{}",
            area,
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&path).expect("create temp repo dir");
        path
    }

    /// Initialize a repository with `main` as the initial branch.
    pub fn init() -> Self {
        let path = Self::make_dir("repo");
        let mut options = RepositoryInitOptions::new();
        options.initial_head("main");
        Repository::init_opts(&path, &options).expect("init temp repo");
        Self { path }
    }

    /// Initialize a repository and add an `origin` remote pointing at `origin_url`.
    pub fn init_with_origin(origin_url: &str) -> Self {
        let repo_dir = Self::init();
        let repo = Repository::open(&repo_dir.path).expect("open temp repo");
        repo.remote("origin", origin_url)
            .expect("add origin remote");
        repo_dir
    }

    /// Create the temporary directory without initializing a repository.
    pub fn empty() -> Self {
        Self {
            path: Self::make_dir("repo"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Open the repository living in this directory.
    pub fn open(&self) -> Repository {
        Repository::open(&self.path).expect("open temp repo")
    }

    /// Write `contents` to `rel` inside the repository, creating parent dirs.
    pub fn write(&self, rel: &str, contents: &str) {
        let full = self.path.join(rel);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).expect("create parent dirs");
        }
        std::fs::write(full, contents).expect("write file");
    }
}

impl Drop for TestRepoDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Deterministic signature for test commits.
pub fn signature() -> Signature<'static> {
    signature_named("Test User", FIXED_TIMESTAMP)
}

/// Signature with an explicit author name and timestamp (for history-ordering tests).
pub fn signature_named(name: &str, timestamp: i64) -> Signature<'static> {
    Signature::new(name, "tester@example.com", &Time::new(timestamp, 0)).expect("signature")
}

/// The empty tree written from the repository's current (empty) index.
pub fn empty_tree(repo: &Repository) -> git2::Tree<'_> {
    let tree_id = {
        let mut index = repo.index().expect("index");
        index.write_tree().expect("write tree")
    };
    repo.find_tree(tree_id).expect("find tree")
}

/// Stage every change and commit it to HEAD, returning the new commit id.
pub fn commit_all(repo: &Repository, message: &str) -> git2::Oid {
    {
        let mut index = repo.index().expect("index");
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .expect("add all");
        index.update_all(["*"], None).expect("update all");
        index.write().expect("write index");
    }
    let tree_id = {
        let mut index = repo.index().expect("index");
        index.write_tree().expect("write tree")
    };
    let tree = repo.find_tree(tree_id).expect("find tree");
    let sig = signature();
    let parents = repo
        .head()
        .ok()
        .and_then(|head| head.peel_to_commit().ok())
        .into_iter()
        .collect::<Vec<_>>();
    let parent_refs = parents.iter().collect::<Vec<_>>();
    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
        .expect("commit")
}
