# Code Structure Index

## src/core/ports.rs

### enum GitRemoteAuth
- Location: src/core/ports.rs:5
- Namespace: crate::core::ports
- Anchor: pub enum GitRemoteAuth<'a> {

### trait GitBranchReadPort
- Location: src/core/ports.rs:10
- Namespace: crate::core::ports
- Anchor: pub trait GitBranchReadPort {

#### method GitBranchReadPort.current_branch_name
- Location: src/core/ports.rs:11
- Namespace: crate::core::ports
- Anchor: fn current_branch_name(&self, repo_path: &Path) -> Result<Option<String>, String>;

### trait GitRemoteSyncPort
- Location: src/core/ports.rs:14
- Namespace: crate::core::ports
- Anchor: pub trait GitRemoteSyncPort {

#### method GitRemoteSyncPort.push
- Location: src/core/ports.rs:15
- Namespace: crate::core::ports
- Anchor: fn push(

#### method GitRemoteSyncPort.pull
- Location: src/core/ports.rs:21
- Namespace: crate::core::ports
- Anchor: fn pull(

#### method GitRemoteSyncPort.reset_to_remote
- Location: src/core/ports.rs:27
- Namespace: crate::core::ports
- Anchor: fn reset_to_remote(

### trait GitRemoteInfoPort
- Location: src/core/ports.rs:35
- Namespace: crate::core::ports
- Anchor: pub trait GitRemoteInfoPort {

#### method GitRemoteInfoPort.has_origin_remote
- Location: src/core/ports.rs:36
- Namespace: crate::core::ports
- Anchor: fn has_origin_remote(&self, repo_path: &Path) -> Result<bool, String>;

### trait GitTagPort
- Location: src/core/ports.rs:39
- Namespace: crate::core::ports
- Anchor: pub trait GitTagPort {

#### method GitTagPort.can_create_tag_on_branch
- Location: src/core/ports.rs:40
- Namespace: crate::core::ports
- Anchor: fn can_create_tag_on_branch(&self, branch_name: &str) -> bool;

#### method GitTagPort.create_tag
- Location: src/core/ports.rs:41
- Namespace: crate::core::ports
- Anchor: fn create_tag(&self, repo_path: &Path, tag_name: &str) -> Result<(), String>;

#### method GitTagPort.push_tag
- Location: src/core/ports.rs:42
- Namespace: crate::core::ports
- Anchor: fn push_tag(

#### method GitTagPort.rollback_tag
- Location: src/core/ports.rs:48
- Namespace: crate::core::ports
- Anchor: fn rollback_tag(&self, repo_path: &Path, tag_name: &str) -> Result<(), String>;

### trait GitRepoBootstrapPort
- Location: src/core/ports.rs:51
- Namespace: crate::core::ports
- Anchor: pub trait GitRepoBootstrapPort {

#### method GitRepoBootstrapPort.open_or_init_repo
- Location: src/core/ports.rs:52
- Namespace: crate::core::ports
- Anchor: fn open_or_init_repo(&self, repo_path: &Path) -> Result<(), String>;

#### method GitRepoBootstrapPort.add_remote
- Location: src/core/ports.rs:53
- Namespace: crate::core::ports
- Anchor: fn add_remote(&self, repo_path: &Path, name: &str, url: &str) -> Result<(), String>;

### trait GitWorktreeCommitPort
- Location: src/core/ports.rs:56
- Namespace: crate::core::ports
- Anchor: pub trait GitWorktreeCommitPort {

#### method GitWorktreeCommitPort.repo_has_changes
- Location: src/core/ports.rs:57
- Namespace: crate::core::ports
- Anchor: fn repo_has_changes(&self, repo_path: &Path) -> Result<bool, String>;

#### method GitWorktreeCommitPort.head_exists
- Location: src/core/ports.rs:58
- Namespace: crate::core::ports
- Anchor: fn head_exists(&self, repo_path: &Path) -> Result<bool, String>;

#### method GitWorktreeCommitPort.stage_all
- Location: src/core/ports.rs:59
- Namespace: crate::core::ports
- Anchor: fn stage_all(&self, repo_path: &Path) -> Result<(), String>;

#### method GitWorktreeCommitPort.create_commit
- Location: src/core/ports.rs:60
- Namespace: crate::core::ports
- Anchor: fn create_commit(&self, repo_path: &Path, message: &str) -> Result<(), String>;

### trait GitUndoCommitPort
- Location: src/core/ports.rs:63
- Namespace: crate::core::ports
- Anchor: pub trait GitUndoCommitPort {

#### method GitUndoCommitPort.outgoing_commit_count
- Location: src/core/ports.rs:64
- Namespace: crate::core::ports
- Anchor: fn outgoing_commit_count(&self, repo_path: &Path) -> Result<usize, String>;

#### method GitUndoCommitPort.undo_last_commit
- Location: src/core/ports.rs:65
- Namespace: crate::core::ports
- Anchor: fn undo_last_commit(&self, repo_path: &Path) -> Result<String, String>;

### trait GitPort
- Location: src/core/ports.rs:69
- Namespace: crate::core::ports
- Anchor: pub trait GitPort:

### trait GitHubRemoteInfoPort
- Location: src/core/ports.rs:92
- Namespace: crate::core::ports
- Anchor: pub trait GitHubRemoteInfoPort {

#### method GitHubRemoteInfoPort.is_github_https_origin
- Location: src/core/ports.rs:93
- Namespace: crate::core::ports
- Anchor: fn is_github_https_origin(&self, repo_path: &Path) -> bool;

#### method GitHubRemoteInfoPort.detect_pull_request_prompt
- Location: src/core/ports.rs:94
- Namespace: crate::core::ports
- Anchor: fn detect_pull_request_prompt(

### trait GitHubRepoCreationPort
- Location: src/core/ports.rs:102
- Namespace: crate::core::ports
- Anchor: pub trait GitHubRepoCreationPort {

#### method GitHubRepoCreationPort.create_repository
- Location: src/core/ports.rs:103
- Namespace: crate::core::ports
- Anchor: fn create_repository(

### trait GitHubPort
- Location: src/core/ports.rs:112
- Namespace: crate::core::ports
- Anchor: pub trait GitHubPort: GitHubRemoteInfoPort + GitHubRepoCreationPort {}
