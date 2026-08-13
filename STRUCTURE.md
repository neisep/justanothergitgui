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
- Location: src/app/dialogs.rs:145
- Namespace: crate::app::dialogs
- Anchor: pub(super) fn show_clone_repo_dialog(&mut self, ctx: &egui::Context) {

#### method GitGuiApp.start_clone_repo
- Location: src/app/dialogs.rs:187
- Namespace: crate::app::dialogs
- Anchor: pub(super) fn start_clone_repo(&mut self) {

#### method GitGuiApp.show_publish_repo_dialog
- Location: src/app/dialogs.rs:220
- Namespace: crate::app::dialogs
- Anchor: pub(super) fn show_publish_repo_dialog(&mut self, ctx: &egui::Context) {

#### method GitGuiApp.show_create_branch_dialog
- Location: src/app/dialogs.rs:306
- Namespace: crate::app::dialogs
- Anchor: pub(super) fn show_create_branch_dialog(&mut self, ctx: &egui::Context) {

#### method GitGuiApp.show_create_branch_confirm_dialog
- Location: src/app/dialogs.rs:344
- Namespace: crate::app::dialogs
- Anchor: pub(super) fn show_create_branch_confirm_dialog(&mut self, ctx: &egui::Context) {

#### method GitGuiApp.show_create_tag_dialog
- Location: src/app/dialogs.rs:368
- Namespace: crate::app::dialogs
- Anchor: pub(super) fn show_create_tag_dialog(&mut self, ctx: &egui::Context) {

#### method GitGuiApp.show_cleanup_branches_dialog
- Location: src/app/dialogs.rs:415
- Namespace: crate::app::dialogs
- Anchor: pub(super) fn show_cleanup_branches_dialog(&mut self, ctx: &egui::Context) {

#### method GitGuiApp.show_discard_dialog
- Location: src/app/dialogs.rs:450
- Namespace: crate::app::dialogs
- Anchor: pub(super) fn show_discard_dialog(&mut self, ctx: &egui::Context) {

#### method GitGuiApp.show_github_auth_dialog
- Location: src/app/dialogs.rs:490
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
- Location: src/app/shell.rs:16
- Namespace: crate::app::shell
- Anchor: struct RepoToolbarModel {

#### method RepoToolbarModel.from_state
- Location: src/app/shell.rs:38
- Namespace: crate::app::shell
- Anchor: fn from_state(

#### method GitGuiApp.refresh_active_tab
- Location: src/app/shell.rs:129
- Namespace: crate::app::shell
- Anchor: pub(super) fn refresh_active_tab(&mut self) {

#### method GitGuiApp.handle_keyboard_shortcuts
- Location: src/app/shell.rs:215
- Namespace: crate::app::shell
- Anchor: pub(super) fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {

#### method GitGuiApp.show_log_viewer_dialog
- Location: src/app/shell.rs:287
- Namespace: crate::app::shell
- Anchor: pub(super) fn show_log_viewer_dialog(&mut self, ctx: &egui::Context) {

#### method GitGuiApp.show_repo_tabs
- Location: src/app/shell.rs:315
- Namespace: crate::app::shell
- Anchor: pub(super) fn show_repo_tabs(&mut self, ui: &mut egui::Ui) {

#### method GitGuiApp.repo_tab_labels
- Location: src/app/shell.rs:337
- Namespace: crate::app::shell
- Anchor: fn repo_tab_labels(&self) -> Vec<(String, Option<String>)> {

#### method GitGuiApp.show_repo_tabs_panel
- Location: src/app/shell.rs:353
- Namespace: crate::app::shell
- Anchor: fn show_repo_tabs_panel(

#### method GitGuiApp.show_repo_menu
- Location: src/app/shell.rs:394
- Namespace: crate::app::shell
- Anchor: fn show_repo_menu(

#### method GitGuiApp.show_repo_toolbar_actions
- Location: src/app/shell.rs:506
- Namespace: crate::app::shell
- Anchor: fn show_repo_toolbar_actions(

#### method GitGuiApp.show_remote_sync_actions
- Location: src/app/shell.rs:542
- Namespace: crate::app::shell
- Anchor: fn show_remote_sync_actions(

#### method GitGuiApp.show_pull_request_action
- Location: src/app/shell.rs:581
- Namespace: crate::app::shell
- Anchor: fn show_pull_request_action(

#### method GitGuiApp.show_branch_controls
- Location: src/app/shell.rs:621
- Namespace: crate::app::shell
- Anchor: fn show_branch_controls(ui: &mut egui::Ui, state: &mut AppState, toolbar: &RepoToolbarModel) {

#### method GitGuiApp.apply_repo_tabs_output
- Location: src/app/shell.rs:652
- Namespace: crate::app::shell
- Anchor: fn apply_repo_tabs_output(&mut self, active_index: usize, output: RepoTabsUiOutput) {

#### method GitGuiApp.show_welcome
- Location: src/app/shell.rs:686
- Namespace: crate::app::shell
- Anchor: pub(super) fn show_welcome(&mut self, ui: &mut egui::Ui) {

#### method GitGuiApp.refresh_active_tab_quiet
- Location: src/app/shell.rs:139
- Namespace: crate::app::shell
- Anchor: pub(super) fn refresh_active_tab_quiet(&mut self) {

#### method GitGuiApp.refresh_active_tab_inner
- Location: src/app/shell.rs:143
- Namespace: crate::app::shell
- Anchor: fn refresh_active_tab_inner(&mut self, announce: bool) {

#### method GitGuiApp.show_refresh_action
- Location: src/app/shell.rs:525
- Namespace: crate::app::shell
- Anchor: fn show_refresh_action(

#### method GitGuiApp.handle_window_focus
- Location: src/app/shell.rs:198
- Namespace: crate::app::shell
- Anchor: pub(super) fn handle_window_focus(&mut self, ctx: &egui::Context) {

## src/app.rs

### const GITHUB_OAUTH_CLIENT_ID
- Location: src/app.rs:30
- Namespace: crate::app
- Anchor: const GITHUB_OAUTH_CLIENT_ID: &str = "Ov23liRh81zsShRFaA4r";

### const SHORTCUT_STAGE_SELECTED_FILE
- Location: src/app.rs:31
- Namespace: crate::app
- Anchor: const SHORTCUT_STAGE_SELECTED_FILE: egui::KeyboardShortcut =

### const SHORTCUT_COMMIT
- Location: src/app.rs:33
- Namespace: crate::app
- Anchor: const SHORTCUT_COMMIT: egui::KeyboardShortcut =

### const SHORTCUT_REFRESH
- Location: src/app.rs:35
- Namespace: crate::app
- Anchor: const SHORTCUT_REFRESH: egui::KeyboardShortcut =

### const SHORTCUT_REFRESH_F5
- Location: src/app.rs:37
- Namespace: crate::app
- Anchor: const SHORTCUT_REFRESH_F5: egui::KeyboardShortcut =

### const SHORTCUT_FOCUS_COMMIT
- Location: src/app.rs:39
- Namespace: crate::app
- Anchor: const SHORTCUT_FOCUS_COMMIT: egui::KeyboardShortcut =

### struct RepoTab
- Location: src/app.rs:42
- Namespace: crate::app
- Anchor: struct RepoTab {

### struct PublishRepoDialogState
- Location: src/app.rs:49
- Namespace: crate::app
- Anchor: pub(crate) struct PublishRepoDialogState {

### struct CloneRepoDialogState
- Location: src/app.rs:61
- Namespace: crate::app
- Anchor: pub(crate) struct CloneRepoDialogState {

#### method CloneRepoDialogState.new
- Location: src/app.rs:74
- Namespace: crate::app
- Anchor: fn new() -> Self {

#### method CloneRepoDialogState.reset
- Location: src/app.rs:89
- Namespace: crate::app
- Anchor: fn reset(&mut self) {

### struct SettingsDialogState
- Location: src/app.rs:101
- Namespace: crate::app
- Anchor: pub(crate) struct SettingsDialogState {

### struct GitGuiApp
- Location: src/app.rs:108
- Namespace: crate::app
- Anchor: pub struct GitGuiApp {

#### method PublishRepoDialogState.new
- Location: src/app.rs:128
- Namespace: crate::app
- Anchor: fn new(ruleset: CommitMessageRuleSet) -> Self {

#### method PublishRepoDialogState.reset_for_path
- Location: src/app.rs:145
- Namespace: crate::app
- Anchor: fn reset_for_path(&mut self, path: Option<PathBuf>, ruleset: CommitMessageRuleSet) {

#### method PublishRepoDialogState.set_folder
- Location: src/app.rs:154
- Namespace: crate::app
- Anchor: fn set_folder(&mut self, path: PathBuf) {

#### method GitGuiApp.new
- Location: src/app.rs:162
- Namespace: crate::app
- Anchor: pub fn new(cc: &eframe::CreationContext<'_>) -> Self {

#### method GitGuiApp.active_tab_index
- Location: src/app.rs:246
- Namespace: crate::app
- Anchor: fn active_tab_index(&self) -> Option<usize> {

#### method GitGuiApp.log_target
- Location: src/app.rs:255
- Namespace: crate::app
- Anchor: fn log_target(&self, tab_index: Option<usize>) -> &AppLogger {

#### method GitGuiApp.normalize_active_tab
- Location: src/app.rs:262
- Namespace: crate::app
- Anchor: fn normalize_active_tab(&mut self) -> Option<usize> {

#### method GitGuiApp.ui
- Location: src/app.rs:270
- Namespace: crate::app
- Anchor: fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {

## src/ui/diff_panel.rs

### struct DiffPanelState
- Location: src/ui/diff_panel.rs:11
- Namespace: crate::ui::diff_panel
- Anchor: pub struct DiffPanelState<'a> {

### function show
- Location: src/ui/diff_panel.rs:18
- Namespace: crate::ui::diff_panel
- Anchor: pub fn show(ui: &mut egui::Ui, mut state: DiffPanelState<'_>) {

### function show_history
- Location: src/ui/diff_panel.rs:46
- Namespace: crate::ui::diff_panel
- Anchor: fn show_history(ui: &mut egui::Ui, state: &mut DiffPanelState<'_>) {

### function show_diff_or_conflict
- Location: src/ui/diff_panel.rs:69
- Namespace: crate::ui::diff_panel
- Anchor: fn show_diff_or_conflict(ui: &mut egui::Ui, state: &mut DiffPanelState<'_>) {

### function show_diff_empty_state
- Location: src/ui/diff_panel.rs:79
- Namespace: crate::ui::diff_panel
- Anchor: fn show_diff_empty_state(ui: &mut egui::Ui, state: &DiffPanelState<'_>) {

### function show_diff_view
- Location: src/ui/diff_panel.rs:106
- Namespace: crate::ui::diff_panel
- Anchor: fn show_diff_view(ui: &mut egui::Ui, state: &mut DiffPanelState<'_>) {

### enum MergeSide
- Location: src/ui/diff_panel.rs:139
- Namespace: crate::ui::diff_panel
- Anchor: enum MergeSide {

### const CURRENT_TEXT
- Location: src/ui/diff_panel.rs:146
- Namespace: crate::ui::diff_panel
- Anchor: const CURRENT_TEXT: egui::Color32 = egui::Color32::from_rgb(120, 220, 130);

### const INCOMING_TEXT
- Location: src/ui/diff_panel.rs:147
- Namespace: crate::ui::diff_panel
- Anchor: const INCOMING_TEXT: egui::Color32 = egui::Color32::from_rgb(125, 180, 255);

### const CUSTOM_ACCENT
- Location: src/ui/diff_panel.rs:148
- Namespace: crate::ui::diff_panel
- Anchor: const CUSTOM_ACCENT: egui::Color32 = egui::Color32::from_rgb(190, 190, 190);

### const UNRESOLVED_ACCENT
- Location: src/ui/diff_panel.rs:149
- Namespace: crate::ui::diff_panel
- Anchor: const UNRESOLVED_ACCENT: egui::Color32 = egui::Color32::from_rgb(240, 180, 70);

### const EDIT_ACCENT
- Location: src/ui/diff_panel.rs:150
- Namespace: crate::ui::diff_panel
- Anchor: const EDIT_ACCENT: egui::Color32 = egui::Color32::from_rgb(230, 160, 230);

### function show_conflict_view
- Location: src/ui/diff_panel.rs:152
- Namespace: crate::ui::diff_panel
- Anchor: fn show_conflict_view(ui: &mut egui::Ui, state: &mut DiffPanelState<'_>) {

### function apply_choice
- Location: src/ui/diff_panel.rs:255
- Namespace: crate::ui::diff_panel
- Anchor: fn apply_choice(

### enum ResultAction
- Location: src/ui/diff_panel.rs:267
- Namespace: crate::ui::diff_panel
- Anchor: enum ResultAction {

### function apply_result_action
- Location: src/ui/diff_panel.rs:273
- Namespace: crate::ui::diff_panel
- Anchor: fn apply_result_action(data: &mut ConflictData, action: Option<ResultAction>, ctx: &egui::Context) {

### function render_result_document
- Location: src/ui/diff_panel.rs:288
- Namespace: crate::ui::diff_panel
- Anchor: fn render_result_document(

### function render_result_conflict
- Location: src/ui/diff_panel.rs:344
- Namespace: crate::ui::diff_panel
- Anchor: fn render_result_conflict(ui: &mut egui::Ui, data: &ConflictData, index: usize) {

### function line_checkbox
- Location: src/ui/diff_panel.rs:378
- Namespace: crate::ui::diff_panel
- Anchor: fn line_checkbox(ui: &mut egui::Ui, keep: bool, text: &str, kept_color: egui::Color32) -> bool {

### function render_custom_zone
- Location: src/ui/diff_panel.rs:391
- Namespace: crate::ui::diff_panel
- Anchor: fn render_custom_zone(

### function render_edit_zone
- Location: src/ui/diff_panel.rs:434
- Namespace: crate::ui::diff_panel
- Anchor: fn render_edit_zone(

### function line_or_space
- Location: src/ui/diff_panel.rs:471
- Namespace: crate::ui::diff_panel
- Anchor: fn line_or_space(line: &str) -> &str {

### function edit_buffer
- Location: src/ui/diff_panel.rs:481
- Namespace: crate::ui::diff_panel
- Anchor: fn edit_buffer(text: &str) -> String {

### function conflict_zone_frame
- Location: src/ui/diff_panel.rs:486
- Namespace: crate::ui::diff_panel
- Anchor: fn conflict_zone_frame(

### function render_input_pane
- Location: src/ui/diff_panel.rs:502
- Namespace: crate::ui::diff_panel
- Anchor: fn render_input_pane(

### function render_input_conflict_lines
- Location: src/ui/diff_panel.rs:573
- Namespace: crate::ui::diff_panel
- Anchor: fn render_input_conflict_lines(

### function render_input_buttons
- Location: src/ui/diff_panel.rs:612
- Namespace: crate::ui::diff_panel
- Anchor: fn render_input_buttons(

### function render_plain_lines
- Location: src/ui/diff_panel.rs:663
- Namespace: crate::ui::diff_panel
- Anchor: fn render_plain_lines(ui: &mut egui::Ui, text: &str, color: egui::Color32) {

## src/ui/file_panel.rs

### const STATUS_COL_WIDTH
- Location: src/ui/file_panel.rs:11
- Namespace: crate::ui::file_panel
- Anchor: const STATUS_COL_WIDTH: f32 = 24.0;

### const ACTION_COL_WIDTH
- Location: src/ui/file_panel.rs:12
- Namespace: crate::ui::file_panel
- Anchor: const ACTION_COL_WIDTH: f32 = 72.0;

### struct FilePanelState
- Location: src/ui/file_panel.rs:24
- Namespace: crate::ui::file_panel
- Anchor: pub struct FilePanelState<'a> {

### function show
- Location: src/ui/file_panel.rs:30
- Namespace: crate::ui::file_panel
- Anchor: pub fn show(ui: &mut egui::Ui, mut state: FilePanelState<'_>) {

### function show_file_list
- Location: src/ui/file_panel.rs:223
- Namespace: crate::ui::file_panel
- Anchor: fn show_file_list(

### struct FileTable
- Location: src/ui/file_panel.rs:249
- Namespace: crate::ui::file_panel
- Anchor: struct FileTable<'a, 'f> {

### function render_file_table
- Location: src/ui/file_panel.rs:255
- Namespace: crate::ui::file_panel
- Anchor: fn render_file_table(

### function render_empty_section
- Location: src/ui/file_panel.rs:432
- Namespace: crate::ui::file_panel
- Anchor: fn render_empty_section(

### function drag_handle
- Location: src/ui/file_panel.rs:500
- Namespace: crate::ui::file_panel
- Anchor: fn drag_handle(ui: &mut egui::Ui) -> egui::Response {

### function handle_drop
- Location: src/ui/file_panel.rs:526
- Namespace: crate::ui::file_panel
- Anchor: fn handle_drop(

### function show_drag_ghost
- Location: src/ui/file_panel.rs:578
- Namespace: crate::ui::file_panel
- Anchor: fn show_drag_ghost(ctx: &egui::Context, state: &FilePanelState<'_>) {

### const PANEL_DEFAULT_WIDTH
- Location: src/ui/file_panel.rs:13
- Namespace: crate::ui::file_panel
- Anchor: const PANEL_DEFAULT_WIDTH: f32 = 300.0;

### const PANEL_MIN_WIDTH
- Location: src/ui/file_panel.rs:14
- Namespace: crate::ui::file_panel
- Anchor: const PANEL_MIN_WIDTH: f32 = 220.0;

### const MIN_SECTION_HEIGHT
- Location: src/ui/file_panel.rs:16
- Namespace: crate::ui::file_panel
- Anchor: const MIN_SECTION_HEIGHT: f32 = 110.0;

### const MAX_UNSTAGED_FRACTION
- Location: src/ui/file_panel.rs:19
- Namespace: crate::ui::file_panel
- Anchor: const MAX_UNSTAGED_FRACTION: f32 = 0.65;

### const SECTION_CHROME
- Location: src/ui/file_panel.rs:21
- Namespace: crate::ui::file_panel
- Anchor: const SECTION_CHROME: f32 = 32.0;

### const CONFLICT_TEXT
- Location: src/ui/file_panel.rs:22
- Namespace: crate::ui::file_panel
- Anchor: const CONFLICT_TEXT: egui::Color32 = egui::Color32::from_rgb(255, 170, 80);

### function row_height
- Location: src/ui/file_panel.rs:104
- Namespace: crate::ui::file_panel
- Anchor: fn row_height(ui: &egui::Ui) -> f32 {

### function matches_filter
- Location: src/ui/file_panel.rs:110
- Namespace: crate::ui::file_panel
- Anchor: fn matches_filter(path: &str, filter: &str) -> bool {

### function filtered
- Location: src/ui/file_panel.rs:119
- Namespace: crate::ui::file_panel
- Anchor: fn filtered<'a>(files: &'a [FileEntry], filter: &str) -> Vec<&'a FileEntry> {

### function split_display_path
- Location: src/ui/file_panel.rs:131
- Namespace: crate::ui::file_panel
- Anchor: fn split_display_path(path: &str) -> (&str, &str) {

### function preferred_unstaged_height
- Location: src/ui/file_panel.rs:146
- Namespace: crate::ui::file_panel
- Anchor: fn preferred_unstaged_height(rows: usize, row_height: f32, available: f32) -> f32 {

### function show_filter_row
- Location: src/ui/file_panel.rs:155
- Namespace: crate::ui::file_panel
- Anchor: fn show_filter_row(ui: &mut egui::Ui, inspector: &mut InspectorState) {

### struct SectionHeader
- Location: src/ui/file_panel.rs:177
- Namespace: crate::ui::file_panel
- Anchor: struct SectionHeader<'a> {

### function show_section_header
- Location: src/ui/file_panel.rs:185
- Namespace: crate::ui::file_panel
- Anchor: fn show_section_header(

### function render_path
- Location: src/ui/file_panel.rs:406
- Namespace: crate::ui::file_panel
- Anchor: fn render_path(ui: &mut egui::Ui, file: &FileEntry, is_selected: bool) {

## src/ui/mod.rs

### function prepare_clickable_rows
- Location: src/ui/mod.rs:20
- Namespace: crate::ui
- Anchor: pub fn prepare_clickable_rows(ui: &mut egui::Ui) {

### struct HoveredRow
- Location: src/ui/mod.rs:47
- Namespace: crate::ui
- Anchor: pub struct HoveredRow {

#### method HoveredRow.load
- Location: src/ui/mod.rs:55
- Namespace: crate::ui
- Anchor: pub fn load(ui: &egui::Ui, salt: &str) -> Self {

#### method HoveredRow.is_hovered
- Location: src/ui/mod.rs:66
- Namespace: crate::ui
- Anchor: pub fn is_hovered(&self, index: usize) -> bool {

#### method HoveredRow.observe
- Location: src/ui/mod.rs:71
- Namespace: crate::ui
- Anchor: pub fn observe(&mut self, index: usize, response: &egui::Response) {

#### method HoveredRow.store
- Location: src/ui/mod.rs:79
- Namespace: crate::ui
- Anchor: pub fn store(self, ui: &egui::Ui) {

### function show_inline_busy
- Location: src/ui/mod.rs:95
- Namespace: crate::ui
- Anchor: pub fn show_inline_busy(ui: &mut egui::Ui, label: &str) {

## src/ui/commit_panel.rs

### function show
- Location: src/ui/commit_panel.rs:7
- Namespace: crate::ui::commit_panel
- Anchor: pub fn show(

### function show_prefix_suggestions
- Location: src/ui/commit_panel.rs:92
- Namespace: crate::ui::commit_panel
- Anchor: pub fn show_prefix_suggestions(

### function move_text_cursor_to_subject_end
- Location: src/ui/commit_panel.rs:134
- Namespace: crate::ui::commit_panel
- Anchor: fn move_text_cursor_to_subject_end(ctx: &egui::Context, text_edit_id: egui::Id, message: &str) {

## src/ui/history_panel.rs

### const GRAPH_COL_WIDTH
- Location: src/ui/history_panel.rs:12
- Namespace: crate::ui::history_panel
- Anchor: const GRAPH_COL_WIDTH: f32 = 24.0;

### const OID_COL_WIDTH
- Location: src/ui/history_panel.rs:13
- Namespace: crate::ui::history_panel
- Anchor: const OID_COL_WIDTH: f32 = 76.0;

### const META_COL_WIDTH
- Location: src/ui/history_panel.rs:14
- Namespace: crate::ui::history_panel
- Anchor: const META_COL_WIDTH: f32 = 200.0;

### struct HistoryPanelView
- Location: src/ui/history_panel.rs:16
- Namespace: crate::ui::history_panel
- Anchor: pub struct HistoryPanelView<'a> {

### function show
- Location: src/ui/history_panel.rs:22
- Namespace: crate::ui::history_panel
- Anchor: pub fn show(ui: &mut egui::Ui, view: HistoryPanelView<'_>) {

### function draw_graph_lane
- Location: src/ui/history_panel.rs:132
- Namespace: crate::ui::history_panel
- Anchor: fn draw_graph_lane(

## src/state.rs

### struct SelectedFile
- Location: src/state.rs:12
- Namespace: crate::state
- Anchor: pub struct SelectedFile {

### struct ParsedDiff
- Location: src/state.rs:24
- Namespace: crate::state
- Anchor: pub struct ParsedDiff {

#### method ParsedDiff.from_patch
- Location: src/state.rs:31
- Namespace: crate::state
- Anchor: fn from_patch(content: &str) -> Self {

### struct SelectedCommit
- Location: src/state.rs:58
- Namespace: crate::state
- Anchor: pub struct SelectedCommit {

### enum CenterView
- Location: src/state.rs:87
- Namespace: crate::state
- Anchor: pub enum CenterView {

### struct DragFile
- Location: src/state.rs:94
- Namespace: crate::state
- Anchor: pub struct DragFile {

### enum BusyAction
- Location: src/state.rs:151
- Namespace: crate::state
- Anchor: pub enum BusyAction {

### struct BusyState
- Location: src/state.rs:165
- Namespace: crate::state
- Anchor: pub struct BusyState {

#### method BusyState.new
- Location: src/state.rs:171
- Namespace: crate::state
- Anchor: pub fn new(action: BusyAction, label: impl Into<String>) -> Self {

### struct AppState
- Location: src/state.rs:180
- Namespace: crate::state
- Anchor: pub struct AppState {

#### method AppState.refresh_parts_mut
- Location: src/state.rs:190
- Namespace: crate::state
- Anchor: pub fn refresh_parts_mut(

### struct RepoState
- Location: src/state.rs:212
- Namespace: crate::state
- Anchor: pub struct RepoState {

### struct WorktreeState
- Location: src/state.rs:225
- Namespace: crate::state
- Anchor: pub struct WorktreeState {

### struct InspectorState
- Location: src/state.rs:231
- Namespace: crate::state
- Anchor: pub struct InspectorState {

### struct ConflictEdit
- Location: src/state.rs:255
- Namespace: crate::state
- Anchor: pub struct ConflictEdit {

#### method InspectorState.set_diff
- Location: src/state.rs:263
- Namespace: crate::state
- Anchor: pub fn set_diff(&mut self, content: String) {

#### method InspectorState.clear_diff
- Location: src/state.rs:268
- Namespace: crate::state
- Anchor: pub fn clear_diff(&mut self) {

#### method InspectorState.set_conflict
- Location: src/state.rs:275
- Namespace: crate::state
- Anchor: pub fn set_conflict(&mut self, data: Option<ConflictData>) {

#### method InspectorState.set_commit
- Location: src/state.rs:283
- Namespace: crate::state
- Anchor: pub fn set_commit(&mut self, commit: Option<SelectedCommit>) {

### struct CommitState
- Location: src/state.rs:289
- Namespace: crate::state
- Anchor: pub struct CommitState {

### struct DialogState
- Location: src/state.rs:297
- Namespace: crate::state
- Anchor: pub struct DialogState {

### struct BranchDialogState
- Location: src/state.rs:305
- Namespace: crate::state
- Anchor: pub struct BranchDialogState {

### struct TagDialogState
- Location: src/state.rs:315
- Namespace: crate::state
- Anchor: pub struct TagDialogState {

### struct CleanupBranchesDialogState
- Location: src/state.rs:322
- Namespace: crate::state
- Anchor: pub struct CleanupBranchesDialogState {

### struct DiscardDialogState
- Location: src/state.rs:328
- Namespace: crate::state
- Anchor: pub struct DiscardDialogState {

### struct UiState
- Location: src/state.rs:334
- Namespace: crate::state
- Anchor: pub struct UiState {

#### method UiState.default
- Location: src/state.rs:341
- Namespace: crate::state
- Anchor: fn default() -> Self {

### enum StatusLevel
- Location: src/state.rs:104
- Namespace: crate::state
- Anchor: pub enum StatusLevel {

### struct StatusMessage
- Location: src/state.rs:114
- Namespace: crate::state
- Anchor: pub struct StatusMessage {

#### method StatusMessage.info
- Location: src/state.rs:120
- Namespace: crate::state
- Anchor: pub fn info(text: impl Into<String>) -> Self {

#### method StatusMessage.success
- Location: src/state.rs:127
- Namespace: crate::state
- Anchor: pub fn success(text: impl Into<String>) -> Self {

#### method StatusMessage.error
- Location: src/state.rs:134
- Namespace: crate::state
- Anchor: pub fn error(text: impl Into<String>) -> Self {

#### method StatusMessage.text
- Location: src/state.rs:141
- Namespace: crate::state
- Anchor: pub fn text(&self) -> &str {

#### method StatusMessage.level
- Location: src/state.rs:145
- Namespace: crate::state
- Anchor: pub fn level(&self) -> StatusLevel {

#### method StatusMessage.is_empty
- Location: src/state.rs:149
- Namespace: crate::state
- Anchor: pub fn is_empty(&self) -> bool {

## src/ui/dialogs/settings.rs

### struct SettingsDialogOutput
- Location: src/ui/dialogs/settings.rs:6
- Namespace: crate::ui::dialogs::settings
- Anchor: pub struct SettingsDialogOutput {

### function show
- Location: src/ui/dialogs/settings.rs:13
- Namespace: crate::ui::dialogs::settings
- Anchor: pub fn show(

## src/app/helpers.rs

### function refresh_status
- Location: src/app/helpers.rs:10
- Namespace: crate::app::helpers
- Anchor: pub(super) fn refresh_status(

### function reset_repo_state
- Location: src/app/helpers.rs:87
- Namespace: crate::app::helpers
- Anchor: pub(super) fn reset_repo_state(repo_state: &mut RepoState) {

### function reset_worktree_state
- Location: src/app/helpers.rs:98
- Namespace: crate::app::helpers
- Anchor: pub(super) fn reset_worktree_state(worktree_state: &mut WorktreeState) {

### function reset_commit_state
- Location: src/app/helpers.rs:103
- Namespace: crate::app::helpers
- Anchor: pub(super) fn reset_commit_state(commit_state: &mut CommitState) {

### function reset_inspector_state
- Location: src/app/helpers.rs:110
- Namespace: crate::app::helpers
- Anchor: pub(super) fn reset_inspector_state(inspector_state: &mut InspectorState) {

### function reset_dialog_state
- Location: src/app/helpers.rs:121
- Namespace: crate::app::helpers
- Anchor: pub(super) fn reset_dialog_state(dialog_state: &mut DialogState) {

### function reset_ui_state
- Location: src/app/helpers.rs:128
- Namespace: crate::app::helpers
- Anchor: pub(super) fn reset_ui_state(ui_state: &mut UiState) {

### function reset_branch_dialog_state
- Location: src/app/helpers.rs:133
- Namespace: crate::app::helpers
- Anchor: fn reset_branch_dialog_state(dialog_state: &mut BranchDialogState) {

### function reset_tag_dialog_state
- Location: src/app/helpers.rs:142
- Namespace: crate::app::helpers
- Anchor: fn reset_tag_dialog_state(dialog_state: &mut TagDialogState) {

### function reset_cleanup_dialog_state
- Location: src/app/helpers.rs:148
- Namespace: crate::app::helpers
- Anchor: fn reset_cleanup_dialog_state(dialog_state: &mut CleanupBranchesDialogState) {

### function reset_discard_dialog_state
- Location: src/app/helpers.rs:153
- Namespace: crate::app::helpers
- Anchor: fn reset_discard_dialog_state(dialog_state: &mut DiscardDialogState) {

### function load_selected_file
- Location: src/app/helpers.rs:159
- Namespace: crate::app::helpers
- Anchor: pub(super) fn load_selected_file(

### function load_selected_commit
- Location: src/app/helpers.rs:204
- Namespace: crate::app::helpers
- Anchor: pub(super) fn load_selected_commit(

### function load_commit_file_diff
- Location: src/app/helpers.rs:265
- Namespace: crate::app::helpers
- Anchor: pub(super) fn load_commit_file_diff(

### function repo_root_path
- Location: src/app/helpers.rs:298
- Namespace: crate::app::helpers
- Anchor: pub(super) fn repo_root_path(repo: &Repository) -> PathBuf {

### function repo_tab_label
- Location: src/app/helpers.rs:304
- Namespace: crate::app::helpers
- Anchor: pub(super) fn repo_tab_label(path: Option<&Path>) -> String {

### function default_repo_name_for_path
- Location: src/app/helpers.rs:312
- Namespace: crate::app::helpers
- Anchor: pub(super) fn default_repo_name_for_path(path: &Path) -> String {

### function status_message_for_error
- Location: src/app/helpers.rs:316
- Namespace: crate::app::helpers
- Anchor: pub(super) fn status_message_for_error(context: &str, detail: &str) -> StatusMessage {

### const WORKER_DISPATCH_ERROR_DETAIL
- Location: src/app/helpers.rs:324
- Namespace: crate::app::helpers
- Anchor: pub(super) const WORKER_DISPATCH_ERROR_DETAIL: &str = "worker rejected task dispatch";

### function status_message_for_worker_dispatch
- Location: src/app/helpers.rs:326
- Namespace: crate::app::helpers
- Anchor: pub(super) fn status_message_for_worker_dispatch(context: &str) -> StatusMessage {

### function sync_pull_request_prompt
- Location: src/app/helpers.rs:330
- Namespace: crate::app::helpers
- Anchor: fn sync_pull_request_prompt(repo_state: &mut RepoState) {

### function sync_selected_commit
- Location: src/app/helpers.rs:344
- Namespace: crate::app::helpers
- Anchor: fn sync_selected_commit(repo_state: &RepoState, inspector_state: &mut InspectorState) {

### function sync_selected_file
- Location: src/app/helpers.rs:358
- Namespace: crate::app::helpers
- Anchor: fn sync_selected_file(

## src/app/worker_events.rs

### struct WelcomeWorkerContext
- Location: src/app/worker_events.rs:9
- Namespace: crate::app::worker_events
- Anchor: pub(crate) struct WelcomeWorkerContext<'a> {

### struct RepoWorkerContext
- Location: src/app/worker_events.rs:13
- Namespace: crate::app::worker_events
- Anchor: pub(crate) struct RepoWorkerContext<'a> {

#### method WelcomeWorkerContext.new
- Location: src/app/worker_events.rs:19
- Namespace: crate::app::worker_events
- Anchor: fn new(app: &'a mut GitGuiApp) -> Self {

#### method RepoWorkerContext.request_refresh
- Location: src/app/worker_events.rs:25
- Namespace: crate::app::worker_events
- Anchor: fn request_refresh(&mut self) {

#### method RepoWorkerContext.log_error
- Location: src/app/worker_events.rs:29
- Namespace: crate::app::worker_events
- Anchor: fn log_error(&mut self, context: &str, detail: &str) {

#### method GithubAuthPromptResult.apply
- Location: src/app/worker_events.rs:35
- Namespace: crate::app::worker_events
- Anchor: fn apply(self: Box<Self>, ctx: &mut WelcomeWorkerContext<'_>) {

#### method GithubAuthResult.apply
- Location: src/app/worker_events.rs:52
- Namespace: crate::app::worker_events
- Anchor: fn apply(self: Box<Self>, ctx: &mut WelcomeWorkerContext<'_>) {

#### method CreateGithubRepoResult.apply
- Location: src/app/worker_events.rs:106
- Namespace: crate::app::worker_events
- Anchor: fn apply(self: Box<Self>, ctx: &mut WelcomeWorkerContext<'_>) {

#### method ListGithubReposResult.apply
- Location: src/app/worker_events.rs:129
- Namespace: crate::app::worker_events
- Anchor: fn apply(self: Box<Self>, ctx: &mut WelcomeWorkerContext<'_>) {

#### method CloneRepoResult.apply
- Location: src/app/worker_events.rs:146
- Namespace: crate::app::worker_events
- Anchor: fn apply(self: Box<Self>, ctx: &mut WelcomeWorkerContext<'_>) {

#### method PushResult.apply
- Location: src/app/worker_events.rs:170
- Namespace: crate::app::worker_events
- Anchor: fn apply(self: Box<Self>, ctx: &mut RepoWorkerContext<'_>) {

#### method PullResult.apply
- Location: src/app/worker_events.rs:198
- Namespace: crate::app::worker_events
- Anchor: fn apply(self: Box<Self>, ctx: &mut RepoWorkerContext<'_>) {

#### method CreateTagResult.apply
- Location: src/app/worker_events.rs:215
- Namespace: crate::app::worker_events
- Anchor: fn apply(self: Box<Self>, ctx: &mut RepoWorkerContext<'_>) {

#### method OpenPullRequestResult.apply
- Location: src/app/worker_events.rs:236
- Namespace: crate::app::worker_events
- Anchor: fn apply(self: Box<Self>, ctx: &mut RepoWorkerContext<'_>) {

#### method CreatePullRequestResult.apply
- Location: src/app/worker_events.rs:252
- Namespace: crate::app::worker_events
- Anchor: fn apply(self: Box<Self>, ctx: &mut RepoWorkerContext<'_>) {

#### method DiscardAndResetResult.apply
- Location: src/app/worker_events.rs:268
- Namespace: crate::app::worker_events
- Anchor: fn apply(self: Box<Self>, ctx: &mut RepoWorkerContext<'_>) {

#### method UndoLastCommitResult.apply
- Location: src/app/worker_events.rs:290
- Namespace: crate::app::worker_events
- Anchor: fn apply(self: Box<Self>, ctx: &mut RepoWorkerContext<'_>) {

#### method GitGuiApp.poll_workers
- Location: src/app/worker_events.rs:310
- Namespace: crate::app::worker_events
- Anchor: pub(super) fn poll_workers(&mut self) -> bool {

### function refresh_repo_tab
- Location: src/app/worker_events.rs:342
- Namespace: crate::app::worker_events
- Anchor: fn refresh_repo_tab(tab: &mut RepoTab) {

## src/app/actions.rs

### struct TabActionContext
- Location: src/app/actions.rs:4
- Namespace: crate::app::actions
- Anchor: struct TabActionContext<'a> {

#### method UiAction.apply
- Location: src/app/actions.rs:11
- Namespace: crate::app::actions
- Anchor: fn apply(self, ctx: &mut TabActionContext<'_>) {

### function stage_file
- Location: src/app/actions.rs:42
- Namespace: crate::app::actions
- Anchor: fn stage_file(ctx: &mut TabActionContext<'_>, path: String) {

### function unstage_file
- Location: src/app/actions.rs:50
- Namespace: crate::app::actions
- Anchor: fn unstage_file(ctx: &mut TabActionContext<'_>, path: String) {

### function stage_all
- Location: src/app/actions.rs:58
- Namespace: crate::app::actions
- Anchor: fn stage_all(ctx: &mut TabActionContext<'_>) {

### function unstage_all
- Location: src/app/actions.rs:66
- Namespace: crate::app::actions
- Anchor: fn unstage_all(ctx: &mut TabActionContext<'_>) {

### function commit
- Location: src/app/actions.rs:74
- Namespace: crate::app::actions
- Anchor: fn commit(ctx: &mut TabActionContext<'_>) {

### function push
- Location: src/app/actions.rs:106
- Namespace: crate::app::actions
- Anchor: fn push(ctx: &mut TabActionContext<'_>) {

### function pull
- Location: src/app/actions.rs:122
- Namespace: crate::app::actions
- Anchor: fn pull(ctx: &mut TabActionContext<'_>) {

### function select_file
- Location: src/app/actions.rs:138
- Namespace: crate::app::actions
- Anchor: fn select_file(ctx: &mut TabActionContext<'_>, path: String, staged: bool) {

### function switch_branch
- Location: src/app/actions.rs:150
- Namespace: crate::app::actions
- Anchor: fn switch_branch(ctx: &mut TabActionContext<'_>, branch: String) {

### function create_branch
- Location: src/app/actions.rs:161
- Namespace: crate::app::actions
- Anchor: fn create_branch(ctx: &mut TabActionContext<'_>, branch: String) {

### function open_create_branch_confirm
- Location: src/app/actions.rs:177
- Namespace: crate::app::actions
- Anchor: fn open_create_branch_confirm(ctx: &mut TabActionContext<'_>, branch: String) {

### function confirm_create_branch
- Location: src/app/actions.rs:196
- Namespace: crate::app::actions
- Anchor: fn confirm_create_branch(ctx: &mut TabActionContext<'_>) {

### function create_tag
- Location: src/app/actions.rs:204
- Namespace: crate::app::actions
- Anchor: fn create_tag(ctx: &mut TabActionContext<'_>, tag_name: String) {

### function launch_pull_request
- Location: src/app/actions.rs:230
- Namespace: crate::app::actions
- Anchor: fn launch_pull_request(ctx: &mut TabActionContext<'_>) {

### function show_diff
- Location: src/app/actions.rs:269
- Namespace: crate::app::actions
- Anchor: fn show_diff(ctx: &mut TabActionContext<'_>) {

### function show_history
- Location: src/app/actions.rs:273
- Namespace: crate::app::actions
- Anchor: fn show_history(ctx: &mut TabActionContext<'_>) {

### function select_commit
- Location: src/app/actions.rs:277
- Namespace: crate::app::actions
- Anchor: fn select_commit(ctx: &mut TabActionContext<'_>, oid: String) {

### function select_commit_file
- Location: src/app/actions.rs:288
- Namespace: crate::app::actions
- Anchor: fn select_commit_file(ctx: &mut TabActionContext<'_>, path: String) {

### function close_commit
- Location: src/app/actions.rs:293
- Namespace: crate::app::actions
- Anchor: fn close_commit(ctx: &mut TabActionContext<'_>) {

### function open_cleanup_branches
- Location: src/app/actions.rs:297
- Namespace: crate::app::actions
- Anchor: fn open_cleanup_branches(ctx: &mut TabActionContext<'_>) {

### function delete_stale_branches
- Location: src/app/actions.rs:307
- Namespace: crate::app::actions
- Anchor: fn delete_stale_branches(ctx: &mut TabActionContext<'_>, names: Vec<String>) {

### function open_discard_dialog
- Location: src/app/actions.rs:335
- Namespace: crate::app::actions
- Anchor: fn open_discard_dialog(ctx: &mut TabActionContext<'_>) {

### function discard_and_reset
- Location: src/app/actions.rs:342
- Namespace: crate::app::actions
- Anchor: fn discard_and_reset(ctx: &mut TabActionContext<'_>, clean_untracked: bool) {

### function undo_last_commit
- Location: src/app/actions.rs:362
- Namespace: crate::app::actions
- Anchor: fn undo_last_commit(ctx: &mut TabActionContext<'_>) {

### function save_conflict_resolution
- Location: src/app/actions.rs:378
- Namespace: crate::app::actions
- Anchor: fn save_conflict_resolution(ctx: &mut TabActionContext<'_>) {

#### method GitGuiApp.process_actions
- Location: src/app/actions.rs:411
- Namespace: crate::app::actions
- Anchor: pub(super) fn process_actions(&mut self) {

### function clear_repo_selection
- Location: src/app/actions.rs:434
- Namespace: crate::app::actions
- Anchor: fn clear_repo_selection(inspector_state: &mut InspectorState) {

### function log_action_error
- Location: src/app/actions.rs:441
- Namespace: crate::app::actions
- Anchor: fn log_action_error(ctx: &mut TabActionContext<'_>, context: &str, detail: String) {

### function log_worker_dispatch_error
- Location: src/app/actions.rs:446
- Namespace: crate::app::actions
- Anchor: fn log_worker_dispatch_error(ctx: &mut TabActionContext<'_>, context: &str) {

### function refresh_tab
- Location: src/app/actions.rs:453
- Namespace: crate::app::actions
- Anchor: fn refresh_tab(ctx: &mut TabActionContext<'_>) {

## src/ui/bottom_bar.rs

### const ERROR_TEXT
- Location: src/ui/bottom_bar.rs:7
- Namespace: crate::ui::bottom_bar
- Anchor: const ERROR_TEXT: egui::Color32 = egui::Color32::from_rgb(230, 120, 120);

### const SUCCESS_TEXT
- Location: src/ui/bottom_bar.rs:8
- Namespace: crate::ui::bottom_bar
- Anchor: const SUCCESS_TEXT: egui::Color32 = egui::Color32::from_rgb(120, 200, 130);

### struct BottomBarView
- Location: src/ui/bottom_bar.rs:10
- Namespace: crate::ui::bottom_bar
- Anchor: pub struct BottomBarView<'a> {

### function show
- Location: src/ui/bottom_bar.rs:15
- Namespace: crate::ui::bottom_bar
- Anchor: pub fn show(ui: &mut egui::Ui, view: BottomBarView<'_>, has_logs: bool) -> bool {

### function show_status
- Location: src/ui/bottom_bar.rs:39
- Namespace: crate::ui::bottom_bar
- Anchor: fn show_status(ui: &mut egui::Ui, status: &StatusMessage) {

## src/app/repo.rs

#### method GitGuiApp.open_repo_dialog
- Location: src/app/repo.rs:4
- Namespace: crate::app::repo
- Anchor: pub(super) fn open_repo_dialog(&mut self) {

#### method GitGuiApp.open_repo
- Location: src/app/repo.rs:10
- Namespace: crate::app::repo
- Anchor: pub(super) fn open_repo(&mut self, path: PathBuf) {

#### method GitGuiApp.add_repo_tab
- Location: src/app/repo.rs:24
- Namespace: crate::app::repo
- Anchor: pub(super) fn add_repo_tab(&mut self, repo: Repository) {

#### method GitGuiApp.restore_previous_session
- Location: src/app/repo.rs:88
- Namespace: crate::app::repo
- Anchor: pub(super) fn restore_previous_session(&mut self) {

#### method GitGuiApp.persist_session
- Location: src/app/repo.rs:126
- Namespace: crate::app::repo
- Anchor: pub(super) fn persist_session(&mut self) {

#### method GitGuiApp.close_repo_tab
- Location: src/app/repo.rs:150
- Namespace: crate::app::repo
- Anchor: pub(super) fn close_repo_tab(&mut self, index: usize) {

#### method GitGuiApp.set_status_message
- Location: src/app/repo.rs:175
- Namespace: crate::app::repo
- Anchor: pub(super) fn set_status_message(&mut self, message: StatusMessage) {

#### method GitGuiApp.open_publish_repo_dialog
- Location: src/app/repo.rs:183
- Namespace: crate::app::repo
- Anchor: pub(super) fn open_publish_repo_dialog(&mut self, path: Option<PathBuf>) {

#### method GitGuiApp.open_clone_repo_dialog
- Location: src/app/repo.rs:191
- Namespace: crate::app::repo
- Anchor: pub(super) fn open_clone_repo_dialog(&mut self) {

#### method GitGuiApp.open_settings_dialog
- Location: src/app/repo.rs:211
- Namespace: crate::app::repo
- Anchor: pub(super) fn open_settings_dialog(&mut self) {

#### method GitGuiApp.refresh_github_auth_status
- Location: src/app/repo.rs:218
- Namespace: crate::app::repo
- Anchor: pub(super) fn refresh_github_auth_status(&mut self) {

#### method GitGuiApp.begin_github_sign_in
- Location: src/app/repo.rs:230
- Namespace: crate::app::repo
- Anchor: pub(super) fn begin_github_sign_in(&mut self, start_message: &str) {

## src/settings.rs

### struct AppSettings
- Location: src/settings.rs:10
- Namespace: crate::settings
- Anchor: pub struct AppSettings {

### function auto_refresh_on_focus_default
- Location: src/settings.rs:24
- Namespace: crate::settings
- Anchor: fn auto_refresh_on_focus_default() -> bool {

#### method AppSettings.default
- Location: src/settings.rs:29
- Namespace: crate::settings
- Anchor: fn default() -> Self {

### function load_app_settings
- Location: src/settings.rs:38
- Namespace: crate::settings
- Anchor: pub fn load_app_settings() -> Result<AppSettings, String> {

### function save_app_settings
- Location: src/settings.rs:56
- Namespace: crate::settings
- Anchor: pub fn save_app_settings(settings: &AppSettings) -> Result<(), String> {

### function settings_path
- Location: src/settings.rs:80
- Namespace: crate::settings
- Anchor: fn settings_path() -> PathBuf {

### function config_dir
- Location: src/settings.rs:85
- Namespace: crate::settings
- Anchor: pub fn config_dir() -> PathBuf {

### function settings_dir
- Location: src/settings.rs:89
- Namespace: crate::settings
- Anchor: fn settings_dir() -> PathBuf {

## src/ui/diff_view.rs

### const LINE_NUMBER_WIDTH
- Location: src/ui/diff_view.rs:10
- Namespace: crate::ui::diff_view
- Anchor: const LINE_NUMBER_WIDTH: f32 = 44.0;

### const BADGE_VERTICAL_MARGIN
- Location: src/ui/diff_view.rs:12
- Namespace: crate::ui::diff_view
- Anchor: const BADGE_VERTICAL_MARGIN: f32 = 2.0;

### function show_diff_table
- Location: src/ui/diff_view.rs:20
- Namespace: crate::ui::diff_view
- Anchor: pub(crate) fn show_diff_table(ui: &mut egui::Ui, rows: &[ParsedDiffLine], wrap_lines: bool) {

### function diff_row_height
- Location: src/ui/diff_view.rs:70
- Namespace: crate::ui::diff_view
- Anchor: fn diff_row_height(ui: &egui::Ui) -> f32 {

### function render_diff_rows
- Location: src/ui/diff_view.rs:76
- Namespace: crate::ui::diff_view
- Anchor: fn render_diff_rows(

### struct SideBySideView
- Location: src/ui/diff_view.rs:105
- Namespace: crate::ui::diff_view
- Anchor: pub(crate) struct SideBySideView<'a> {

### function show_side_by_side
- Location: src/ui/diff_view.rs:112
- Namespace: crate::ui::diff_view
- Anchor: pub(crate) fn show_side_by_side(ui: &mut egui::Ui, view: SideBySideView<'_>) -> f32 {

### enum Side
- Location: src/ui/diff_view.rs:131
- Namespace: crate::ui::diff_view
- Anchor: enum Side {

#### method Side.title
- Location: src/ui/diff_view.rs:137
- Namespace: crate::ui::diff_view
- Anchor: fn title(self) -> &'static str {

#### method Side.scroll_id
- Location: src/ui/diff_view.rs:144
- Namespace: crate::ui::diff_view
- Anchor: fn scroll_id(self) -> &'static str {

#### method Side.grid_id
- Location: src/ui/diff_view.rs:151
- Namespace: crate::ui::diff_view
- Anchor: fn grid_id(self) -> &'static str {

#### method Side.cell
- Location: src/ui/diff_view.rs:158
- Namespace: crate::ui::diff_view
- Anchor: fn cell(self, entry: &SideBySideEntry) -> Option<&SideCell> {

### function render_pane
- Location: src/ui/diff_view.rs:175
- Namespace: crate::ui::diff_view
- Anchor: fn render_pane(ui: &mut egui::Ui, entries: &[SideBySideEntry], scroll: f32, side: Side) -> f32 {

### function render_header_line
- Location: src/ui/diff_view.rs:227
- Namespace: crate::ui::diff_view
- Anchor: fn render_header_line(ui: &mut egui::Ui, entry: &SideBySideEntry) {

### function render_line_number
- Location: src/ui/diff_view.rs:235
- Namespace: crate::ui::diff_view
- Anchor: pub(crate) fn render_line_number(ui: &mut egui::Ui, line_number: Option<usize>) {

### function render_diff_badge
- Location: src/ui/diff_view.rs:244
- Namespace: crate::ui::diff_view
- Anchor: pub(crate) fn render_diff_badge(ui: &mut egui::Ui, kind: DiffLineKind) {

### function render_diff_content
- Location: src/ui/diff_view.rs:292
- Namespace: crate::ui::diff_view
- Anchor: pub(crate) fn render_diff_content(

### function diff_line_color
- Location: src/ui/diff_view.rs:312
- Namespace: crate::ui::diff_view
- Anchor: pub(crate) fn diff_line_color(kind: DiffLineKind, ui: &egui::Ui) -> egui::Color32 {

### function render_status_badge
- Location: src/ui/diff_view.rs:341
- Namespace: crate::ui::diff_view
- Anchor: pub(crate) fn render_status_badge(ui: &mut egui::Ui, display_status: &str, is_conflicted: bool) {

### function status_style
- Location: src/ui/diff_view.rs:325
- Namespace: crate::ui::diff_view
- Anchor: pub(crate) fn status_style(display_status: &str, is_conflicted: bool) -> (egui::Color32, &str) {

### function render_status_chip
- Location: src/ui/diff_view.rs:362
- Namespace: crate::ui::diff_view
- Anchor: pub(crate) fn render_status_chip(ui: &mut egui::Ui, display_status: &str, is_conflicted: bool) {

### const CHIP_SIZE
- Location: src/ui/diff_view.rs:382
- Namespace: crate::ui::diff_view
- Anchor: const CHIP_SIZE: f32 = 18.0;
