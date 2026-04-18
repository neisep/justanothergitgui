mod actions;
mod dialogs;
mod helpers;
mod ports;
mod repo;
mod shell;
mod worker_events;

pub(crate) use actions::TabActionContext;
use ports::{AppGitHubAuth, AppRepoRead, AppRepoWrite};
pub(crate) use ports::{AppRepoWorkerOps, AppWelcomeWorkerOps};
pub(crate) use worker_events::{RepoWorkerContext, WelcomeWorkerContext};

use std::path::{Path, PathBuf};

use eframe::egui;
use git2::Repository;

use crate::commit_rules::{self, CommitMessageRuleSet};
use crate::logging::{self, AppLogger};
use crate::settings::{self, AppSettings};
use crate::shared::actions::UiAction;
use crate::shared::github::{
    CreateGithubRepoRequest, GithubAuthCheck, GithubAuthPrompt, GithubAuthSession,
    GithubRepoSummary, GithubRepoVisibility, PullRequestPrompt,
};
use crate::state::{AppState, BusyAction, BusyState};
use crate::ui;
use crate::worker::{RepoWorker, WelcomeWorker};

const GITHUB_OAUTH_CLIENT_ID: &str = "Ov23liRh81zsShRFaA4r";
const SHORTCUT_STAGE_SELECTED_FILE: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::S);
const SHORTCUT_COMMIT: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Enter);
const SHORTCUT_REFRESH: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::R);
const SHORTCUT_REFRESH_F5: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::F5);
const SHORTCUT_FOCUS_COMMIT: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::L);

struct RepoTab {
    state: AppState,
    repo: Repository,
    worker: RepoWorker,
}

pub(crate) struct PublishRepoDialogState {
    pub(crate) show: bool,
    pub(crate) folder_path: String,
    pub(crate) repo_name: String,
    pub(crate) commit_message: String,
    pub(crate) focus_folder_requested: bool,
    pub(crate) visibility: GithubRepoVisibility,
    pub(crate) github_authenticated: bool,
    pub(crate) github_status: String,
    pub(crate) operation_status: String,
}

pub(crate) struct CloneRepoDialogState {
    pub(crate) show: bool,
    pub(crate) url: String,
    pub(crate) parent_folder: String,
    pub(crate) focus_url_requested: bool,
    pub(crate) status: String,
    pub(crate) github_repos: Vec<GithubRepoSummary>,
    pub(crate) github_repos_loading: bool,
    pub(crate) github_repos_error: Option<String>,
    pub(crate) filter_text: String,
}

impl CloneRepoDialogState {
    fn new() -> Self {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            show: false,
            url: String::new(),
            parent_folder: current_dir.display().to_string(),
            focus_url_requested: false,
            status: String::new(),
            github_repos: Vec::new(),
            github_repos_loading: false,
            github_repos_error: None,
            filter_text: String::new(),
        }
    }

    fn reset(&mut self) {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        self.url.clear();
        self.parent_folder = current_dir.display().to_string();
        self.status.clear();
        self.github_repos.clear();
        self.github_repos_loading = false;
        self.github_repos_error = None;
        self.filter_text.clear();
    }
}

pub(crate) struct SettingsDialogState {
    pub(crate) show: bool,
    pub(crate) status: String,
    pub(crate) custom_scopes_input: String,
    pub(crate) focus_custom_scopes_requested: bool,
}

pub struct GitGuiApp {
    tabs: Vec<RepoTab>,
    active_tab: usize,
    welcome_status: String,
    welcome_worker: WelcomeWorker,
    welcome_busy: Option<BusyState>,
    publish_dialog: PublishRepoDialogState,
    clone_dialog: CloneRepoDialogState,
    settings: AppSettings,
    settings_dialog: SettingsDialogState,
    github_auth_session: Option<GithubAuthSession>,
    github_auth_prompt: Option<GithubAuthPrompt>,
    logger: AppLogger,
    show_log_viewer: bool,
}

impl PublishRepoDialogState {
    fn new(ruleset: CommitMessageRuleSet) -> Self {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut state = Self {
            show: false,
            folder_path: current_dir.display().to_string(),
            repo_name: helpers::default_repo_name_for_path(&current_dir),
            commit_message: commit_rules::default_initial_commit_summary(ruleset).into(),
            focus_folder_requested: false,
            visibility: GithubRepoVisibility::Private,
            github_authenticated: false,
            github_status: String::new(),
            operation_status: String::new(),
        };
        state.set_folder(current_dir);
        state
    }

    fn reset_for_path(&mut self, path: Option<PathBuf>, ruleset: CommitMessageRuleSet) {
        let path =
            path.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        self.set_folder(path);
        self.commit_message = commit_rules::default_initial_commit_summary(ruleset).into();
        self.visibility = GithubRepoVisibility::Private;
        self.operation_status.clear();
    }

    fn set_folder(&mut self, path: PathBuf) {
        self.folder_path = path.display().to_string();
        self.repo_name = helpers::default_repo_name_for_path(&path);
        self.operation_status.clear();
    }
}

impl GitGuiApp {
    pub fn new() -> Self {
        let logger = AppLogger::new();
        let mut startup_status = None;
        let settings = match settings::load_app_settings() {
            Ok(settings) => settings,
            Err(msg) => {
                logger.log_error("Settings", &msg);
                startup_status = Some(helpers::status_message_for_error("Settings", &msg));
                AppSettings::default()
            }
        };
        let github_auth_session = match AppGitHubAuth::load_session() {
            Ok(Some(session)) => match AppGitHubAuth::verify_session(&session) {
                Ok(GithubAuthCheck::Valid) => Some(session),
                Ok(GithubAuthCheck::Revoked) => {
                    if let Err(clear_err) = AppGitHubAuth::clear_session() {
                        logger.log_error("GitHub sign-in", &clear_err);
                    }
                    logger.log_error(
                        "GitHub sign-in",
                        "Saved GitHub token was revoked or expired; please sign in again.",
                    );
                    if startup_status.is_none() {
                        startup_status = Some(
                            "GitHub sign-in expired. Use 'Sign in to GitHub...' to reconnect."
                                .into(),
                        );
                    }
                    None
                }
                Err(check_err) => {
                    logger.log_error("GitHub sign-in", &check_err);
                    Some(session)
                }
            },
            Ok(None) => None,
            Err(msg) => {
                logger.log_error("GitHub sign-in", &msg);
                if startup_status.is_none() {
                    startup_status =
                        Some(helpers::status_message_for_error("GitHub sign-in", &msg));
                }
                None
            }
        };
        let settings_custom_scopes_input = settings.commit_message_custom_scopes.join(", ");

        let mut app = Self {
            tabs: Vec::new(),
            active_tab: 0,
            welcome_status: "Open a Git repository to get started.".into(),
            welcome_worker: WelcomeWorker::new(),
            welcome_busy: None,
            publish_dialog: PublishRepoDialogState::new(settings.commit_message_ruleset),
            clone_dialog: CloneRepoDialogState::new(),
            settings,
            settings_dialog: SettingsDialogState {
                show: false,
                status: String::new(),
                custom_scopes_input: settings_custom_scopes_input,
                focus_custom_scopes_requested: false,
            },
            github_auth_session,
            github_auth_prompt: None,
            logger,
            show_log_viewer: false,
        };

        if let Ok(repo) = AppRepoRead::open(Path::new(".")) {
            app.add_repo_tab(repo);
        }

        app.refresh_github_auth_status();
        if let Some(message) = startup_status {
            app.set_status_message(message);
        }

        app
    }

    fn active_tab_index(&self) -> Option<usize> {
        self.tabs
            .len()
            .checked_sub(1)
            .map(|last_index| self.active_tab.min(last_index))
    }

    fn normalize_active_tab(&mut self) -> Option<usize> {
        let active_index = self.active_tab_index()?;
        self.active_tab = active_index;
        Some(active_index)
    }
}

impl eframe::App for GitGuiApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if self.poll_workers() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        self.handle_keyboard_shortcuts(&ctx);

        if self.tabs.is_empty() {
            self.show_welcome(ui);
            self.show_settings_dialog(&ctx);
            self.show_publish_repo_dialog(&ctx);
            self.show_clone_repo_dialog(&ctx);
            self.show_github_auth_dialog(&ctx);
            self.show_log_viewer_dialog(&ctx);
            return;
        }

        self.show_repo_tabs(ui);
        let Some(active_index) = self.normalize_active_tab() else {
            self.show_publish_repo_dialog(&ctx);
            self.show_clone_repo_dialog(&ctx);
            self.show_settings_dialog(&ctx);
            self.show_create_branch_dialog(&ctx);
            self.show_create_branch_confirm_dialog(&ctx);
            self.show_create_tag_dialog(&ctx);
            self.show_discard_dialog(&ctx);
            self.show_cleanup_branches_dialog(&ctx);
            self.show_github_auth_dialog(&ctx);
            self.show_log_viewer_dialog(&ctx);
            self.process_actions();
            return;
        };
        let has_logs = self.logger.has_entries();
        let commit_message_ruleset = self.settings.commit_message_ruleset;
        let commit_message_custom_scopes = &self.settings.commit_message_custom_scopes;

        let open_logs_clicked = {
            let tab = &mut self.tabs[active_index];

            let open_logs = ui::bottom_bar::show(
                ui,
                ui::bottom_bar::BottomBarView {
                    repo_path: tab.state.repo.path.as_deref(),
                    status_msg: &tab.state.ui.status_msg,
                },
                has_logs,
            );
            ui::file_panel::show(
                ui,
                ui::file_panel::FilePanelState {
                    worktree: &tab.state.worktree,
                    inspector: &mut tab.state.inspector,
                    ui_state: &mut tab.state.ui,
                },
            );
            ui::commit_panel::show(
                ui,
                &mut tab.state,
                commit_message_ruleset,
                commit_message_custom_scopes,
            );

            egui::CentralPanel::default().show_inside(ui, |ui| {
                ui::diff_panel::show(
                    ui,
                    ui::diff_panel::DiffPanelState {
                        repo: &tab.state.repo,
                        worktree: &tab.state.worktree,
                        inspector: &mut tab.state.inspector,
                        ui_state: &mut tab.state.ui,
                    },
                );
            });
            open_logs
        };

        if open_logs_clicked {
            self.show_log_viewer = true;
        }

        self.show_publish_repo_dialog(&ctx);
        self.show_clone_repo_dialog(&ctx);
        self.show_settings_dialog(&ctx);
        self.show_create_branch_dialog(&ctx);
        self.show_create_branch_confirm_dialog(&ctx);
        self.show_create_tag_dialog(&ctx);
        self.show_discard_dialog(&ctx);
        self.show_cleanup_branches_dialog(&ctx);
        self.show_github_auth_dialog(&ctx);
        self.show_log_viewer_dialog(&ctx);

        self.process_actions();
    }
}
