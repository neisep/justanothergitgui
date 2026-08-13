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

## src/app/dialogs.rs

#### method GitGuiApp.any_dialog_open
- Location: src/app/dialogs.rs:4
- Namespace: crate::app::dialogs
- Anchor: pub(super) fn any_dialog_open(&self) -> bool {

#### method GitGuiApp.close_topmost_dialog
- Location: src/app/dialogs.rs:24
- Namespace: crate::app::dialogs
- Anchor: pub(super) fn close_topmost_dialog(&mut self) -> bool {

#### method GitGuiApp.show_settings_dialog
- Location: src/app/dialogs.rs:100
- Namespace: crate::app::dialogs
- Anchor: pub(super) fn show_settings_dialog(&mut self, ctx: &egui::Context) {

#### method GitGuiApp.show_clone_repo_dialog
- Location: src/app/dialogs.rs:142
- Namespace: crate::app::dialogs
- Anchor: pub(super) fn show_clone_repo_dialog(&mut self, ctx: &egui::Context) {

#### method GitGuiApp.start_clone_repo
- Location: src/app/dialogs.rs:184
- Namespace: crate::app::dialogs
- Anchor: pub(super) fn start_clone_repo(&mut self) {

#### method GitGuiApp.show_publish_repo_dialog
- Location: src/app/dialogs.rs:217
- Namespace: crate::app::dialogs
- Anchor: pub(super) fn show_publish_repo_dialog(&mut self, ctx: &egui::Context) {

#### method GitGuiApp.show_create_branch_dialog
- Location: src/app/dialogs.rs:303
- Namespace: crate::app::dialogs
- Anchor: pub(super) fn show_create_branch_dialog(&mut self, ctx: &egui::Context) {

#### method GitGuiApp.show_create_branch_confirm_dialog
- Location: src/app/dialogs.rs:341
- Namespace: crate::app::dialogs
- Anchor: pub(super) fn show_create_branch_confirm_dialog(&mut self, ctx: &egui::Context) {

#### method GitGuiApp.show_create_tag_dialog
- Location: src/app/dialogs.rs:365
- Namespace: crate::app::dialogs
- Anchor: pub(super) fn show_create_tag_dialog(&mut self, ctx: &egui::Context) {

#### method GitGuiApp.show_cleanup_branches_dialog
- Location: src/app/dialogs.rs:412
- Namespace: crate::app::dialogs
- Anchor: pub(super) fn show_cleanup_branches_dialog(&mut self, ctx: &egui::Context) {

#### method GitGuiApp.show_discard_dialog
- Location: src/app/dialogs.rs:447
- Namespace: crate::app::dialogs
- Anchor: pub(super) fn show_discard_dialog(&mut self, ctx: &egui::Context) {

#### method GitGuiApp.show_github_auth_dialog
- Location: src/app/dialogs.rs:487
- Namespace: crate::app::dialogs
- Anchor: pub(super) fn show_github_auth_dialog(&mut self, ctx: &egui::Context) {

## src/infra/git/repository.rs

### function open_repo
- Location: src/infra/git/repository.rs:7
- Namespace: crate::infra::git::repository
- Anchor: pub fn open_repo(path: &Path) -> Result<Repository, git2::Error> {

### function get_current_branch
- Location: src/infra/git/repository.rs:15
- Namespace: crate::infra::git::repository
- Anchor: pub fn get_current_branch(repo: &Repository) -> Result<String, git2::Error> {

### function get_branches
- Location: src/infra/git/repository.rs:31
- Namespace: crate::infra::git::repository
- Anchor: pub fn get_branches(repo: &Repository) -> Result<Vec<String>, git2::Error> {

### function get_outgoing_commit_count
- Location: src/infra/git/repository.rs:42
- Namespace: crate::infra::git::repository
- Anchor: pub fn get_outgoing_commit_count(repo: &Repository) -> Result<usize, git2::Error> {

### function can_create_tag_on_branch
- Location: src/infra/git/repository.rs:86
- Namespace: crate::infra::git::repository
- Anchor: pub fn can_create_tag_on_branch(branch_name: &str) -> bool {

### function suggest_next_tag
- Location: src/infra/git/repository.rs:90
- Namespace: crate::infra::git::repository
- Anchor: pub fn suggest_next_tag(repo: &Repository) -> String {

### function has_origin_remote
- Location: src/infra/git/repository.rs:121
- Namespace: crate::infra::git::repository
- Anchor: pub fn has_origin_remote(repo: &Repository) -> bool {

### function preview_discard_damage
- Location: src/infra/git/repository.rs:125
- Namespace: crate::infra::git::repository
- Anchor: pub fn preview_discard_damage(repo: &Repository) -> DiscardPreview {

### function preview_create_branch
- Location: src/infra/git/repository.rs:162
- Namespace: crate::infra::git::repository
- Anchor: pub fn preview_create_branch(repo: &Repository, branch_name: &str) -> CreateBranchPreview {

### function switch_branch
- Location: src/infra/git/repository.rs:203
- Namespace: crate::infra::git::repository
- Anchor: pub fn switch_branch(repo: &Repository, branch_name: &str) -> Result<(), git2::Error> {

### function validate_new_branch_name
- Location: src/infra/git/repository.rs:213
- Namespace: crate::infra::git::repository
- Anchor: pub fn validate_new_branch_name(repo: &Repository, name: &str) -> Option<String> {

### function create_branch
- Location: src/infra/git/repository.rs:231
- Namespace: crate::infra::git::repository
- Anchor: pub fn create_branch(repo: &Repository, branch_name: &str) -> Result<(), git2::Error> {

### function list_stale_branches
- Location: src/infra/git/repository.rs:260
- Namespace: crate::infra::git::repository
- Anchor: pub fn list_stale_branches(repo: &Repository) -> Result<Vec<StaleBranch>, git2::Error> {

### function delete_local_branch
- Location: src/infra/git/repository.rs:304
- Namespace: crate::infra::git::repository
- Anchor: pub fn delete_local_branch(repo: &Repository, name: &str) -> Result<(), git2::Error> {

### function get_commit_history
- Location: src/infra/git/repository.rs:314
- Namespace: crate::infra::git::repository
- Anchor: pub fn get_commit_history(

### function resolve_history_head
- Location: src/infra/git/repository.rs:350
- Namespace: crate::infra::git::repository
- Anchor: fn resolve_history_head(repo: &Repository) -> Result<Option<git2::Oid>, git2::Error> {

### function collect_commit_labels
- Location: src/infra/git/repository.rs:365
- Namespace: crate::infra::git::repository
- Anchor: fn collect_commit_labels(repo: &Repository) -> HashMap<git2::Oid, Vec<String>> {

### function build_history_revwalk
- Location: src/infra/git/repository.rs:393
- Namespace: crate::infra::git::repository
- Anchor: fn build_history_revwalk(

### function build_commit_entry
- Location: src/infra/git/repository.rs:403
- Namespace: crate::infra::git::repository
- Anchor: fn build_commit_entry(

### function open_or_init_repo
- Location: src/infra/git/repository.rs:428
- Namespace: crate::infra::git::repository
- Anchor: pub(crate) fn open_or_init_repo(folder_path: &Path) -> Result<Repository, String> {

### function repo_has_changes
- Location: src/infra/git/repository.rs:453
- Namespace: crate::infra::git::repository
- Anchor: pub(crate) fn repo_has_changes(repo: &Repository) -> Result<bool, String> {

### function current_branch_name
- Location: src/infra/git/repository.rs:459
- Namespace: crate::infra::git::repository
- Anchor: pub(crate) fn current_branch_name(repo_path: &Path) -> Result<Option<String>, String> {

### function symbolic_head_branch_name
- Location: src/infra/git/repository.rs:482
- Namespace: crate::infra::git::repository
- Anchor: fn symbolic_head_branch_name(repo: &Repository) -> Option<String> {

### function parse_semver_tag
- Location: src/infra/git/repository.rs:488
- Namespace: crate::infra::git::repository
- Anchor: pub(crate) fn parse_semver_tag(name: &str) -> Option<([u32; 4], bool)> {

### function repo_root_path
- Location: src/infra/git/repository.rs:504
- Namespace: crate::infra::git::repository
- Anchor: pub(crate) fn repo_root_path(repo: &Repository) -> PathBuf {

### function format_relative_time
- Location: src/infra/git/repository.rs:510
- Namespace: crate::infra::git::repository
- Anchor: fn format_relative_time(now: i64, then: i64) -> String {

## src/infra/github/repos.rs

### const GITHUB_REPO_LIST_MAX_PAGES
- Location: src/infra/github/repos.rs:8
- Namespace: crate::infra::github::repos
- Anchor: const GITHUB_REPO_LIST_MAX_PAGES: usize = 5;

### struct GithubRepo
- Location: src/infra/github/repos.rs:11
- Namespace: crate::infra::github::repos
- Anchor: struct GithubRepo {

### struct GithubCreateRepoBody
- Location: src/infra/github/repos.rs:16
- Namespace: crate::infra::github::repos
- Anchor: struct GithubCreateRepoBody<'a> {

### function list_github_repositories
- Location: src/infra/github/repos.rs:21
- Namespace: crate::infra::github::repos
- Anchor: pub fn list_github_repositories(

### function create_repository
- Location: src/infra/github/repos.rs:73
- Namespace: crate::infra::github::repos
- Anchor: pub fn create_repository(

### function repo_name_from_clone_url
- Location: src/infra/github/repos.rs:104
- Namespace: crate::infra::github::repos
- Anchor: pub fn repo_name_from_clone_url(url: &str) -> Option<String> {

### function parse_link_header_next
- Location: src/infra/github/repos.rs:132
- Namespace: crate::infra::github::repos
- Anchor: pub(crate) fn parse_link_header_next(header: &str) -> Option<String> {

### function parse_target_repo_name
- Location: src/infra/github/repos.rs:150
- Namespace: crate::infra::github::repos
- Anchor: fn parse_target_repo_name(

## src/ui/tab_bar.rs

### struct TabBarView
- Location: src/ui/tab_bar.rs:4
- Namespace: crate::ui::tab_bar
- Anchor: pub struct TabBarView<'a> {

### struct TabBarResponse
- Location: src/ui/tab_bar.rs:14
- Namespace: crate::ui::tab_bar
- Anchor: pub struct TabBarResponse {

### const PAD_X
- Location: src/ui/tab_bar.rs:20
- Namespace: crate::ui::tab_bar
- Anchor: const PAD_X: f32 = 10.0;

### const PAD_Y
- Location: src/ui/tab_bar.rs:21
- Namespace: crate::ui::tab_bar
- Anchor: const PAD_Y: f32 = 5.0;

### const LABEL_CLOSE_GAP
- Location: src/ui/tab_bar.rs:22
- Namespace: crate::ui::tab_bar
- Anchor: const LABEL_CLOSE_GAP: f32 = 6.0;

### const CLOSE_SIZE
- Location: src/ui/tab_bar.rs:23
- Namespace: crate::ui::tab_bar
- Anchor: const CLOSE_SIZE: f32 = 14.0;

### const TAB_CORNER
- Location: src/ui/tab_bar.rs:24
- Namespace: crate::ui::tab_bar
- Anchor: const TAB_CORNER: u8 = 6;

### function show
- Location: src/ui/tab_bar.rs:33
- Namespace: crate::ui::tab_bar
- Anchor: pub fn show(ui: &mut egui::Ui, view: &TabBarView) -> TabBarResponse {

### function draw_tab
- Location: src/ui/tab_bar.rs:111
- Namespace: crate::ui::tab_bar
- Anchor: fn draw_tab(

### const NEW_TAB_GAP
- Location: src/ui/tab_bar.rs:25
- Namespace: crate::ui::tab_bar
- Anchor: const NEW_TAB_GAP: f32 = 4.0;

### const NEW_TAB_GLYPH
- Location: src/ui/tab_bar.rs:30
- Namespace: crate::ui::tab_bar
- Anchor: const NEW_TAB_GLYPH: f32 = 16.0;

### function draw_new_tab_button
- Location: src/ui/tab_bar.rs:69
- Namespace: crate::ui::tab_bar
- Anchor: fn draw_new_tab_button(

### const NEW_TAB_CORNER
- Location: src/ui/tab_bar.rs:29
- Namespace: crate::ui::tab_bar
- Anchor: const NEW_TAB_CORNER: u8 = 4;

### const NEW_TAB_WIDTH
- Location: src/ui/tab_bar.rs:28
- Namespace: crate::ui::tab_bar
- Anchor: const NEW_TAB_WIDTH: f32 = 20.0;

## src/app/shell.rs

### struct RepoTabsUiOutput
- Location: src/app/shell.rs:4
- Namespace: crate::app::shell
- Anchor: struct RepoTabsUiOutput {

### struct RepoToolbarModel
- Location: src/app/shell.rs:15
- Namespace: crate::app::shell
- Anchor: struct RepoToolbarModel {

#### method RepoToolbarModel.from_state
- Location: src/app/shell.rs:37
- Namespace: crate::app::shell
- Anchor: fn from_state(

#### method GitGuiApp.refresh_active_tab
- Location: src/app/shell.rs:126
- Namespace: crate::app::shell
- Anchor: pub(super) fn refresh_active_tab(&mut self) {

#### method GitGuiApp.handle_keyboard_shortcuts
- Location: src/app/shell.rs:172
- Namespace: crate::app::shell
- Anchor: pub(super) fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {

#### method GitGuiApp.show_log_viewer_dialog
- Location: src/app/shell.rs:244
- Namespace: crate::app::shell
- Anchor: pub(super) fn show_log_viewer_dialog(&mut self, ctx: &egui::Context) {

#### method GitGuiApp.show_repo_tabs
- Location: src/app/shell.rs:272
- Namespace: crate::app::shell
- Anchor: pub(super) fn show_repo_tabs(&mut self, ui: &mut egui::Ui) {

#### method GitGuiApp.repo_tab_labels
- Location: src/app/shell.rs:294
- Namespace: crate::app::shell
- Anchor: fn repo_tab_labels(&self) -> Vec<(String, Option<String>)> {

#### method GitGuiApp.show_repo_tabs_panel
- Location: src/app/shell.rs:310
- Namespace: crate::app::shell
- Anchor: fn show_repo_tabs_panel(

#### method GitGuiApp.show_repo_menu
- Location: src/app/shell.rs:351
- Namespace: crate::app::shell
- Anchor: fn show_repo_menu(

#### method GitGuiApp.show_repo_toolbar_actions
- Location: src/app/shell.rs:463
- Namespace: crate::app::shell
- Anchor: fn show_repo_toolbar_actions(

#### method GitGuiApp.show_remote_sync_actions
- Location: src/app/shell.rs:475
- Namespace: crate::app::shell
- Anchor: fn show_remote_sync_actions(

#### method GitGuiApp.show_pull_request_action
- Location: src/app/shell.rs:514
- Namespace: crate::app::shell
- Anchor: fn show_pull_request_action(

#### method GitGuiApp.show_branch_controls
- Location: src/app/shell.rs:554
- Namespace: crate::app::shell
- Anchor: fn show_branch_controls(ui: &mut egui::Ui, state: &mut AppState, toolbar: &RepoToolbarModel) {

#### method GitGuiApp.apply_repo_tabs_output
- Location: src/app/shell.rs:585
- Namespace: crate::app::shell
- Anchor: fn apply_repo_tabs_output(&mut self, active_index: usize, output: RepoTabsUiOutput) {

#### method GitGuiApp.show_welcome
- Location: src/app/shell.rs:616
- Namespace: crate::app::shell
- Anchor: pub(super) fn show_welcome(&mut self, ui: &mut egui::Ui) {
