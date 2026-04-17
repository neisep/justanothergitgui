use std::path::{Path, PathBuf};

use eframe::egui;
use git2::Repository;

use crate::commit_rules::{self, CommitMessageRuleSet};
use crate::git_ops;
use crate::logging::{self, AppLogger};
use crate::settings::{self, AppSettings};
use crate::state::{AppState, CenterView, PullRequestPrompt, SelectedFile, UiAction};
use crate::ui;
use crate::worker::{TaskResult, Worker};

const GITHUB_OAUTH_CLIENT_ID: &str = "Ov23liRh81zsShRFaA4r";

struct RepoTab {
    state: AppState,
    repo: Repository,
    worker: Worker,
}

struct PublishRepoDialogState {
    show: bool,
    folder_path: String,
    repo_name: String,
    commit_message: String,
    visibility: git_ops::GithubRepoVisibility,
    github_authenticated: bool,
    github_status: String,
    operation_status: String,
}

struct SettingsDialogState {
    show: bool,
    status: String,
    custom_scopes_input: String,
}

pub struct GitGuiApp {
    tabs: Vec<RepoTab>,
    active_tab: usize,
    welcome_status: String,
    welcome_worker: Worker,
    publish_dialog: PublishRepoDialogState,
    settings: AppSettings,
    settings_dialog: SettingsDialogState,
    github_auth_session: Option<git_ops::GithubAuthSession>,
    github_auth_prompt: Option<git_ops::GithubAuthPrompt>,
    logger: AppLogger,
    show_log_viewer: bool,
}

impl PublishRepoDialogState {
    fn new(ruleset: CommitMessageRuleSet) -> Self {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut state = Self {
            show: false,
            folder_path: current_dir.display().to_string(),
            repo_name: default_repo_name_for_path(&current_dir),
            commit_message: commit_rules::default_initial_commit_message(ruleset).into(),
            visibility: git_ops::GithubRepoVisibility::Private,
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
        self.commit_message = commit_rules::default_initial_commit_message(ruleset).into();
        self.visibility = git_ops::GithubRepoVisibility::Private;
        self.operation_status.clear();
    }

    fn set_folder(&mut self, path: PathBuf) {
        self.folder_path = path.display().to_string();
        self.repo_name = default_repo_name_for_path(&path);
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
                startup_status = Some(status_message_for_error("Settings", &msg));
                AppSettings::default()
            }
        };
        let github_auth_session = match git_ops::load_github_auth_session() {
            Ok(Some(session)) => Some(session),
            Ok(None) => None,
            Err(msg) => {
                logger.log_error("GitHub sign-in", &msg);
                if startup_status.is_none() {
                    startup_status = Some(status_message_for_error("GitHub sign-in", &msg));
                }
                None
            }
        };
        let settings_custom_scopes_input = settings.commit_message_custom_scopes.join(", ");

        let mut app = Self {
            tabs: Vec::new(),
            active_tab: 0,
            welcome_status: "Open a Git repository to get started.".into(),
            welcome_worker: Worker::new(),
            publish_dialog: PublishRepoDialogState::new(settings.commit_message_ruleset),
            settings,
            settings_dialog: SettingsDialogState {
                show: false,
                status: String::new(),
                custom_scopes_input: settings_custom_scopes_input,
            },
            github_auth_session,
            github_auth_prompt: None,
            logger,
            show_log_viewer: false,
        };

        // Try to open current directory as a repo
        if let Ok(repo) = git_ops::open_repo(Path::new(".")) {
            app.add_repo_tab(repo);
        }

        app.refresh_github_auth_status();
        if let Some(message) = startup_status {
            app.set_status_message(message);
        }

        app
    }

    fn open_repo_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.open_repo(path);
        }
    }

    fn open_repo(&mut self, path: PathBuf) {
        match git_ops::open_repo(&path) {
            Ok(repo) => self.add_repo_tab(repo),
            Err(e) => {
                let detail = e.to_string();
                self.logger.log_error("Open repository", &detail);
                self.set_status_message(status_message_for_error("Open repository", &detail));
            }
        }
    }

    fn add_repo_tab(&mut self, repo: Repository) {
        let repo_path = repo_root_path(&repo);

        if let Some(index) = self
            .tabs
            .iter()
            .position(|tab| tab.state.repo_path.as_ref() == Some(&repo_path))
        {
            self.active_tab = index;
            self.tabs[index].state.status_msg = "Repository already open".into();
            return;
        }

        let mut state = AppState {
            repo_path: Some(repo_path.clone()),
            ..AppState::default()
        };
        reset_repo_view_state(&mut state);
        let refresh_error = refresh_status(&mut state, &repo);
        if let Some(detail) = refresh_error {
            self.logger.log_error("Refresh", &detail);
        } else {
            state.status_msg = format!("Repository loaded: {}", repo_tab_label(Some(&repo_path)));
        }

        self.tabs.push(RepoTab {
            state,
            repo,
            worker: Worker::new(),
        });
        self.active_tab = self.tabs.len() - 1;
        self.welcome_status = "Open a Git repository to get started.".into();
    }

    fn set_status_message(&mut self, message: String) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.state.status_msg = message;
        } else {
            self.welcome_status = message;
        }
    }

    fn open_publish_repo_dialog(&mut self, path: Option<PathBuf>) {
        self.publish_dialog
            .reset_for_path(path, self.settings.commit_message_ruleset);
        self.publish_dialog.show = true;
        self.refresh_github_auth_status();
    }

    fn open_settings_dialog(&mut self) {
        self.settings_dialog.show = true;
        self.settings_dialog.custom_scopes_input =
            self.settings.commit_message_custom_scopes.join(", ");
    }

    fn refresh_github_auth_status(&mut self) {
        if let Some(session) = &self.github_auth_session {
            self.publish_dialog.github_authenticated = true;
            self.publish_dialog.github_status =
                format!("Signed in to GitHub as @{}", session.login);
        } else {
            self.publish_dialog.github_authenticated = false;
            self.publish_dialog.github_status =
                "Not signed in to GitHub. Sign in to create repositories and PRs.".into();
        }
    }

    fn begin_github_sign_in(&mut self, start_message: &str) {
        self.github_auth_prompt = None;
        self.publish_dialog.github_status = start_message.into();
        self.publish_dialog.operation_status.clear();
        self.welcome_status = start_message.into();
        self.set_status_message(start_message.into());
        self.welcome_worker
            .login_github(GITHUB_OAUTH_CLIENT_ID.into());
    }

    fn show_log_viewer_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_log_viewer {
            return;
        }

        let mut keep_open = self.show_log_viewer;
        let log_path = self.logger.path().display().to_string();
        let mut contents = self.logger.read_entries();
        let mut clear_result = None;

        egui::Window::new("Application Logs")
            .id(egui::Id::new("app_logs_dialog"))
            .default_size(egui::vec2(720.0, 420.0))
            .open(&mut keep_open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("Log file: {}", log_path));
                    if ui.button("Clear").clicked() {
                        let result = self.logger.clear_entries();
                        if result.is_ok() {
                            contents = self.logger.read_entries();
                        }
                        clear_result = Some(result);
                    }
                });
                ui.add_space(8.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut contents)
                            .desired_width(f32::INFINITY)
                            .desired_rows(24)
                            .interactive(false)
                            .font(egui::TextStyle::Monospace),
                    );
                });
            });

        if let Some(result) = clear_result {
            match result {
                Ok(()) => self.set_status_message("Logs cleared.".into()),
                Err(error) => {
                    self.set_status_message(status_message_for_error("Clear logs", &error))
                }
            }
        }

        self.show_log_viewer = keep_open;
    }

    fn process_actions(&mut self) {
        if self.tabs.is_empty() {
            return;
        }

        self.active_tab = self.active_tab.min(self.tabs.len() - 1);
        let active_index = self.active_tab;
        let actions: Vec<UiAction> = self.tabs[active_index].state.actions.drain(..).collect();

        for action in actions {
            let mut log_entry: Option<(&str, String)> = None;
            let mut refresh_error = None;
            match action {
                UiAction::StageFile(path) => {
                    let tab = &mut self.tabs[active_index];
                    match git_ops::stage_file(&tab.repo, &path) {
                        Ok(()) => tab.state.status_msg = format!("Staged: {}", path),
                        Err(e) => {
                            let detail = e.to_string();
                            tab.state.status_msg = status_message_for_error("Stage", &detail);
                            log_entry = Some(("Stage", detail));
                        }
                    }
                    refresh_error = refresh_status(&mut tab.state, &tab.repo);
                }

                UiAction::UnstageFile(path) => {
                    let tab = &mut self.tabs[active_index];
                    match git_ops::unstage_file(&tab.repo, &path) {
                        Ok(()) => tab.state.status_msg = format!("Unstaged: {}", path),
                        Err(e) => {
                            let detail = e.to_string();
                            tab.state.status_msg = status_message_for_error("Unstage", &detail);
                            log_entry = Some(("Unstage", detail));
                        }
                    }
                    refresh_error = refresh_status(&mut tab.state, &tab.repo);
                }

                UiAction::StageAll => {
                    let tab = &mut self.tabs[active_index];
                    match git_ops::stage_all(&tab.repo) {
                        Ok(()) => tab.state.status_msg = "Staged all changes".into(),
                        Err(e) => {
                            let detail = e.to_string();
                            tab.state.status_msg = status_message_for_error("Stage all", &detail);
                            log_entry = Some(("Stage all", detail));
                        }
                    }
                    refresh_error = refresh_status(&mut tab.state, &tab.repo);
                }

                UiAction::UnstageAll => {
                    let tab = &mut self.tabs[active_index];
                    match git_ops::unstage_all(&tab.repo) {
                        Ok(()) => tab.state.status_msg = "Unstaged all changes".into(),
                        Err(e) => {
                            let detail = e.to_string();
                            tab.state.status_msg = status_message_for_error("Unstage all", &detail);
                            log_entry = Some(("Unstage all", detail));
                        }
                    }
                    refresh_error = refresh_status(&mut tab.state, &tab.repo);
                }

                UiAction::Commit => {
                    let tab = &mut self.tabs[active_index];
                    let msg = tab.state.commit_msg.trim().to_string();
                    match commit_rules::validate_for_submit(
                        self.settings.commit_message_ruleset,
                        &msg,
                    ) {
                        Ok(()) => match git_ops::create_commit(&tab.repo, &msg) {
                            Ok(oid) => {
                                tab.state.status_msg =
                                    format!("Committed: {}", &oid.to_string()[..8]);
                                tab.state.commit_msg.clear();
                                tab.state.selected_file = None;
                                tab.state.diff_content.clear();
                                tab.state.conflict_data = None;
                            }
                            Err(e) => {
                                let detail = e.to_string();
                                tab.state.status_msg = status_message_for_error("Commit", &detail);
                                log_entry = Some(("Commit", detail));
                            }
                        },
                        Err(detail) => {
                            tab.state.status_msg = detail;
                        }
                    }

                    if tab.state.status_msg.starts_with("Committed:") || log_entry.is_some() {
                        refresh_error = refresh_status(&mut tab.state, &tab.repo);
                    }
                }

                UiAction::Push => {
                    let tab = &mut self.tabs[active_index];
                    if let Some(path) = tab.state.repo_path.clone() {
                        if tab.worker.is_busy() {
                            tab.state.status_msg = "Busy — please wait...".into();
                        } else {
                            tab.state.status_msg = "Pushing...".into();
                            tab.worker.push(path, self.github_auth_session.clone());
                        }
                    }
                }

                UiAction::Pull => {
                    let tab = &mut self.tabs[active_index];
                    if let Some(path) = tab.state.repo_path.clone() {
                        if tab.worker.is_busy() {
                            tab.state.status_msg = "Busy — please wait...".into();
                        } else {
                            tab.state.status_msg = "Pulling...".into();
                            tab.worker.pull(path, self.github_auth_session.clone());
                        }
                    }
                }

                UiAction::SelectFile { path, staged } => {
                    let tab = &mut self.tabs[active_index];
                    tab.state.center_view = CenterView::Diff;
                    load_selected_file(&mut tab.state, &tab.repo, path, staged);
                }

                UiAction::SwitchBranch(branch) => {
                    let tab = &mut self.tabs[active_index];
                    match git_ops::switch_branch(&tab.repo, &branch) {
                        Ok(()) => {
                            tab.state.status_msg = format!("Switched to {}", branch);
                            tab.state.selected_file = None;
                            tab.state.diff_content.clear();
                            tab.state.conflict_data = None;
                        }
                        Err(e) => {
                            let detail = e.to_string();
                            tab.state.status_msg =
                                status_message_for_error("Switch branch", &detail);
                            log_entry = Some(("Switch branch", detail));
                        }
                    }
                    refresh_error = refresh_status(&mut tab.state, &tab.repo);
                }

                UiAction::CreateBranch(branch) => {
                    let tab = &mut self.tabs[active_index];
                    match git_ops::create_branch(&tab.repo, &branch) {
                        Ok(()) => {
                            tab.state.status_msg = format!("Created and switched to {}", branch);
                            tab.state.selected_file = None;
                            tab.state.diff_content.clear();
                            tab.state.conflict_data = None;
                            tab.state.new_branch_name.clear();
                            tab.state.show_create_branch_dialog = false;
                            tab.state.show_create_branch_confirm = false;
                            tab.state.create_branch_preview = None;
                            tab.state.pending_new_branch_name = None;
                        }
                        Err(e) => {
                            let detail = e.to_string();
                            tab.state.status_msg =
                                status_message_for_error("Create branch", &detail);
                            log_entry = Some(("Create branch", detail));
                        }
                    }
                    refresh_error = refresh_status(&mut tab.state, &tab.repo);
                }

                UiAction::OpenCreateBranchConfirm(branch) => {
                    let tab = &mut self.tabs[active_index];
                    let preview = git_ops::preview_create_branch(&tab.repo, &branch);
                    let clean = preview.dirty_files == 0
                        && preview.untracked_files == 0
                        && preview.staged_files == 0;
                    if clean {
                        tab.state.actions.push(UiAction::CreateBranch(branch));
                    } else {
                        tab.state.pending_new_branch_name = Some(branch);
                        tab.state.create_branch_preview = Some(preview);
                        tab.state.show_create_branch_dialog = false;
                        tab.state.show_create_branch_confirm = true;
                    }
                }

                UiAction::ConfirmCreateBranch => {
                    let tab = &mut self.tabs[active_index];
                    if let Some(name) = tab.state.pending_new_branch_name.take() {
                        tab.state.actions.push(UiAction::CreateBranch(name));
                    }
                    tab.state.show_create_branch_confirm = false;
                    tab.state.create_branch_preview = None;
                }

                UiAction::CreateTag(tag_name) => {
                    let tab = &mut self.tabs[active_index];
                    if let Some(path) = tab.state.repo_path.clone() {
                        if tab.worker.is_busy() {
                            tab.state.status_msg = "Busy — please wait...".into();
                        } else if !git_ops::can_create_tag_on_branch(&tab.state.branch) {
                            tab.state.status_msg =
                                "Tags can only be created from the main or master branch.".into();
                        } else if tab.state.has_github_https_origin
                            && self.github_auth_session.is_none()
                        {
                            tab.state.status_msg =
                                "Sign in to GitHub before creating tags for this repository."
                                    .into();
                        } else {
                            tab.state.status_msg = format!("Creating tag {}...", tag_name);
                            tab.state.new_tag_name.clear();
                            tab.state.show_create_tag_dialog = false;
                            tab.worker
                                .create_tag(path, tag_name, self.github_auth_session.clone());
                        }
                    }
                }

                UiAction::LaunchPullRequest => {
                    let tab = &mut self.tabs[active_index];
                    let Some(prompt) = tab.state.pull_request_prompt.clone() else {
                        tab.state.status_msg = "No pull request action available".into();
                        continue;
                    };

                    if tab.worker.is_busy() {
                        tab.state.status_msg = "Busy — please wait...".into();
                        continue;
                    }

                    match prompt {
                        PullRequestPrompt::Open { number, url, .. } => {
                            tab.state.status_msg = format!("Opening pull request #{}...", number);
                            tab.worker.open_pull_request(url);
                        }
                        PullRequestPrompt::Create { branch, url } => {
                            tab.state.status_msg =
                                format!("Opening pull request creation for {}...", branch);
                            tab.worker.create_pull_request(url);
                        }
                    }
                }

                UiAction::ShowDiff => {
                    self.tabs[active_index].state.center_view = CenterView::Diff;
                }

                UiAction::ShowHistory => {
                    self.tabs[active_index].state.center_view = CenterView::History;
                }

                UiAction::OpenCleanupBranches => {
                    let tab = &mut self.tabs[active_index];
                    match git_ops::list_stale_branches(&tab.repo) {
                        Ok(stale) => {
                            tab.state.stale_branches = stale;
                            tab.state.show_cleanup_branches_dialog = true;
                        }
                        Err(e) => {
                            let detail = e.to_string();
                            tab.state.status_msg =
                                status_message_for_error("Cleanup branches", &detail);
                            log_entry = Some(("Cleanup branches", detail));
                        }
                    }
                }

                UiAction::DeleteStaleBranches(names) => {
                    let tab = &mut self.tabs[active_index];
                    let mut deleted: Vec<String> = Vec::new();
                    let mut failures: Vec<String> = Vec::new();
                    for name in &names {
                        match git_ops::delete_local_branch(&tab.repo, name) {
                            Ok(()) => deleted.push(name.clone()),
                            Err(e) => failures.push(format!("{}: {}", name, e)),
                        }
                    }
                    tab.state
                        .stale_branches
                        .retain(|branch| !deleted.contains(&branch.name));
                    if failures.is_empty() {
                        tab.state.status_msg = format!("Deleted {} branch(es)", deleted.len());
                        tab.state.show_cleanup_branches_dialog = false;
                    } else {
                        let detail = failures.join("; ");
                        tab.state.status_msg = status_message_for_error("Delete branch", &detail);
                        log_entry = Some(("Delete branch", detail));
                    }
                    refresh_error = refresh_status(&mut tab.state, &tab.repo);
                }

                UiAction::OpenDiscardDialog => {
                    let tab = &mut self.tabs[active_index];
                    let preview = git_ops::preview_discard_damage(&tab.repo);
                    tab.state.discard_preview = Some(preview);
                    tab.state.discard_clean_untracked = false;
                    tab.state.show_discard_dialog = true;
                }

                UiAction::DiscardAndReset { clean_untracked } => {
                    let tab = &mut self.tabs[active_index];
                    if let Some(path) = tab.state.repo_path.clone() {
                        if tab.worker.is_busy() {
                            tab.state.status_msg = "Busy — please wait...".into();
                        } else {
                            tab.state.status_msg = "Resetting to remote...".into();
                            tab.state.show_discard_dialog = false;
                            tab.state.discard_preview = None;
                            tab.worker.discard_and_reset(
                                path,
                                self.github_auth_session.clone(),
                                clean_untracked,
                            );
                        }
                    }
                }

                UiAction::SaveConflictResolution => {
                    let tab = &mut self.tabs[active_index];
                    if let Some(conflict_data) = tab.state.conflict_data.clone() {
                        let path = conflict_data.path.clone();
                        match git_ops::write_resolved_file(&tab.repo, &conflict_data) {
                            Ok(()) => {
                                tab.state.status_msg = format!("Resolved and staged: {}", path);
                                tab.state.selected_file = Some(SelectedFile { path, staged: true });
                                tab.state.conflict_data = None;
                            }
                            Err(e) => {
                                let detail = e.to_string();
                                tab.state.status_msg =
                                    status_message_for_error("Save resolution", &detail);
                                log_entry = Some(("Save resolution", detail));
                            }
                        }
                    } else {
                        tab.state.status_msg = "No conflict selected".into();
                    }
                    refresh_error = refresh_status(&mut tab.state, &tab.repo);
                }
            }

            if let Some((context, detail)) = log_entry {
                self.logger.log_error(context, &detail);
            }
            if let Some(detail) = refresh_error {
                self.logger.log_error("Refresh", &detail);
            }
        }
    }

    fn poll_workers(&mut self) -> bool {
        let mut refresh_indices = Vec::new();
        let mut any_busy = false;
        let mut tab_logs: Vec<(String, String)> = Vec::new();

        while let Some(result) = self.welcome_worker.try_recv() {
            match result {
                TaskResult::GithubAuthPrompt(prompt) => {
                    let message = format!(
                        "Enter GitHub code {} to finish signing in.",
                        prompt.user_code
                    );
                    self.github_auth_prompt = Some(prompt.clone());
                    self.publish_dialog.github_status = message.clone();
                    self.publish_dialog.operation_status = format!(
                        "If GitHub did not open automatically, visit {}.",
                        prompt.verification_uri
                    );
                    self.welcome_status = message.clone();
                    self.set_status_message(message);
                }
                TaskResult::GithubAuth(Ok(session)) => {
                    let persistence_result = git_ops::save_github_auth_session(&session);
                    let message = match &persistence_result {
                        Ok(()) => format!("GitHub sign-in complete for @{}", session.login),
                        Err(error) => {
                            self.logger.log_error("GitHub sign-in", error);
                            format!(
                                "GitHub sign-in complete for @{}, but {}",
                                session.login,
                                logging::summarize_for_ui(error)
                            )
                        }
                    };
                    self.github_auth_prompt = None;
                    self.github_auth_session = Some(session);
                    self.publish_dialog.github_authenticated = true;
                    self.publish_dialog.github_status = message.clone();
                    self.publish_dialog.operation_status.clear();
                    self.welcome_status = message;
                    self.set_status_message(self.publish_dialog.github_status.clone());
                }
                TaskResult::GithubAuth(Err(msg)) => {
                    self.logger.log_error("GitHub sign-in", &msg);
                    self.github_auth_prompt = None;
                    self.publish_dialog.github_authenticated = self.github_auth_session.is_some();
                    self.publish_dialog.github_status =
                        if let Some(session) = &self.github_auth_session {
                            format!(
                                "Signed in to GitHub as @{} (latest sign-in failed: {})",
                                session.login,
                                logging::summarize_for_ui(&msg)
                            )
                        } else {
                            status_message_for_error("GitHub sign-in", &msg)
                        };
                    self.publish_dialog.operation_status.clear();
                    self.welcome_status = status_message_for_error("GitHub sign-in", &msg);
                    self.set_status_message(self.publish_dialog.github_status.clone());
                }
                TaskResult::CreateGithubRepo(Ok(result)) => {
                    let message = result.message.clone();
                    self.publish_dialog.show = false;
                    self.publish_dialog.operation_status.clear();
                    self.welcome_status = message.clone();
                    self.open_repo(result.folder_path);
                    self.set_status_message(message);
                }
                TaskResult::CreateGithubRepo(Err(msg)) => {
                    self.logger.log_error("Publish to GitHub", &msg);
                    self.publish_dialog.operation_status =
                        status_message_for_error("Publish to GitHub", &msg);
                    self.welcome_status = status_message_for_error("Publish to GitHub", &msg);
                }
                TaskResult::Push(_)
                | TaskResult::Pull(_)
                | TaskResult::CreateTag(_)
                | TaskResult::OpenPullRequest(_)
                | TaskResult::CreatePullRequest(_)
                | TaskResult::DiscardAndReset(_) => {}
            }
        }

        if self.welcome_worker.is_busy() {
            any_busy = true;
        }

        for (index, tab) in self.tabs.iter_mut().enumerate() {
            while let Some(result) = tab.worker.try_recv() {
                match result {
                    TaskResult::Push(Ok(result)) => {
                        let prompt_message = match &result.pull_request_prompt {
                            Some(PullRequestPrompt::Open { number, .. }) => {
                                format!(" Pull request #{} is ready.", number)
                            }
                            Some(PullRequestPrompt::Create { .. }) => {
                                " You can create a pull request now.".into()
                            }
                            None => String::new(),
                        };
                        tab.state.pull_request_prompt = result.pull_request_prompt;
                        tab.state.status_msg =
                            format!("Push: {}{}", result.message, prompt_message);
                        refresh_indices.push(index);
                    }
                    TaskResult::Push(Err(msg)) => {
                        tab.state.status_msg = status_message_for_error("Push", &msg);
                        tab_logs.push(("Push".into(), msg));
                    }
                    TaskResult::Pull(Ok(msg)) => {
                        tab.state.status_msg = format!("Pull: {}", msg);
                        refresh_indices.push(index);
                    }
                    TaskResult::Pull(Err(msg)) => {
                        tab.state.status_msg = status_message_for_error("Pull", &msg);
                        tab_logs.push(("Pull".into(), msg));
                    }
                    TaskResult::CreateTag(Ok(msg)) => {
                        tab.state.status_msg = msg;
                        refresh_indices.push(index);
                    }
                    TaskResult::CreateTag(Err(msg)) => {
                        tab.state.status_msg = status_message_for_error("Create tag", &msg);
                        tab_logs.push(("Create tag".into(), msg));
                        refresh_indices.push(index);
                    }
                    TaskResult::OpenPullRequest(Ok(msg)) => {
                        tab.state.status_msg = msg;
                    }
                    TaskResult::OpenPullRequest(Err(msg)) => {
                        tab.state.status_msg = status_message_for_error("Open PR", &msg);
                        tab_logs.push(("Open PR".into(), msg));
                    }
                    TaskResult::CreatePullRequest(Ok(msg)) => {
                        tab.state.status_msg = msg;
                    }
                    TaskResult::CreatePullRequest(Err(msg)) => {
                        tab.state.status_msg = status_message_for_error("Create PR", &msg);
                        tab_logs.push(("Create PR".into(), msg));
                    }
                    TaskResult::DiscardAndReset(Ok(msg)) => {
                        tab.state.status_msg = format!("Discard: {}", msg);
                        refresh_indices.push(index);
                    }
                    TaskResult::DiscardAndReset(Err(msg)) => {
                        tab.state.status_msg = status_message_for_error("Discard & reset", &msg);
                        tab_logs.push(("Discard & reset".into(), msg));
                        refresh_indices.push(index);
                    }
                    TaskResult::GithubAuthPrompt(_)
                    | TaskResult::GithubAuth(_)
                    | TaskResult::CreateGithubRepo(_) => {}
                }
            }

            if tab.worker.is_busy() {
                any_busy = true;
            }
        }

        for index in refresh_indices {
            let Some(path) = self.tabs[index].state.repo_path.clone() else {
                continue;
            };

            match git_ops::open_repo(&path) {
                Ok(repo) => {
                    if let Some(detail) = refresh_status(&mut self.tabs[index].state, &repo) {
                        tab_logs.push(("Refresh".into(), detail));
                    }
                    self.tabs[index].repo = repo;
                }
                Err(error) => {
                    let detail = error.to_string();
                    self.tabs[index].state.status_msg =
                        status_message_for_error("Refresh", &detail);
                    tab_logs.push(("Refresh".into(), detail));
                }
            }
        }

        for (context, detail) in tab_logs {
            self.logger.log_error(&context, &detail);
        }

        any_busy
    }

    fn show_repo_tabs(&mut self, ui: &mut egui::Ui) {
        let mut next_active = None;
        let mut open_clicked = false;
        let mut settings_clicked = false;
        let active_index = self.active_tab.min(self.tabs.len() - 1);
        let tab_labels: Vec<(String, Option<String>)> = self
            .tabs
            .iter()
            .map(|tab| {
                (
                    repo_tab_label(tab.state.repo_path.as_deref()),
                    tab.state
                        .repo_path
                        .as_ref()
                        .map(|path| path.display().to_string()),
                )
            })
            .collect();

        egui::Panel::top("repo_tabs").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let state = &mut self.tabs[active_index].state;
                let controls_width = if state.branch.is_empty() {
                    520.0
                } else {
                    720.0
                };

                if ui.button("Open...").clicked() {
                    open_clicked = true;
                }

                if ui.button("Settings...").clicked() {
                    settings_clicked = true;
                }

                ui.separator();

                let tabs_width = (ui.available_width() - controls_width).max(120.0);

                egui::ScrollArea::horizontal()
                    .id_salt("repo_tabs_scroll")
                    .max_width(tabs_width)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (index, (label, tooltip)) in tab_labels.iter().enumerate() {
                                let mut response =
                                    ui.selectable_label(index == self.active_tab, label);

                                if let Some(path) = tooltip {
                                    response = response.on_hover_text(path);
                                }

                                if response.clicked() {
                                    next_active = Some(index);
                                }
                            }
                        });
                    });

                ui.separator();

                let has_repo = state.repo_path.is_some();
                let repo_path = state.repo_path.clone();
                let has_origin_remote = state.has_origin_remote;
                let has_github_origin = state.has_github_origin;
                let has_github_https_origin = state.has_github_https_origin;
                let needs_github_sign_in = has_github_origin && self.github_auth_session.is_none();
                let push_label = if state.outgoing_commit_count > 0 {
                    format!("Push({})", state.outgoing_commit_count)
                } else {
                    "Push".into()
                };
                let push_tooltip = if state.outgoing_commit_count > 0 {
                    format!(
                        "Push {} local commit(s) to remote",
                        state.outgoing_commit_count
                    )
                } else {
                    "Push to remote".into()
                };
                let pull_request_prompt = state.pull_request_prompt.clone();
                let mut publish_clicked = false;
                let mut github_sign_in_clicked = false;
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if has_origin_remote {
                        ui.add_enabled_ui(has_repo, |ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button(push_label).on_hover_text(push_tooltip).clicked() {
                                        state.actions.push(UiAction::Push);
                                    }
                                    if ui
                                        .button("Pull")
                                        .on_hover_text("Pull from remote")
                                        .clicked()
                                    {
                                        state.actions.push(UiAction::Pull);
                                    }

                                    let has_branch = !state.branch.is_empty();
                                    let github_auth_ok = !has_github_https_origin
                                        || self.github_auth_session.is_some();
                                    let can_discard = has_branch && github_auth_ok;
                                    let discard_tooltip = if !has_branch {
                                        "Check out a branch to reset it to origin".to_string()
                                    } else if !github_auth_ok {
                                        "Sign in to GitHub to reset to origin".to_string()
                                    } else {
                                        format!(
                                            "Discard local changes and reset '{}' to origin/{}",
                                            state.branch, state.branch
                                        )
                                    };
                                    if ui
                                        .add_enabled(
                                            can_discard,
                                            egui::Button::new("Discard..."),
                                        )
                                        .on_hover_text(discard_tooltip)
                                        .clicked()
                                    {
                                        state.actions.push(UiAction::OpenDiscardDialog);
                                    }
                                },
                            );
                        });
                    } else if ui
                        .add_enabled(has_repo, egui::Button::new("Publish to GitHub..."))
                        .on_hover_text("Create a GitHub repository for this folder and push it")
                        .clicked()
                    {
                        publish_clicked = true;
                    }

                    if needs_github_sign_in
                        && ui
                            .add_enabled(has_repo, egui::Button::new("Sign in to GitHub..."))
                            .on_hover_text("Sign in so the app can check and open pull requests")
                            .clicked()
                    {
                        github_sign_in_clicked = true;
                    }

                    if let Some(prompt) = &pull_request_prompt {
                        let (label, hover) = match prompt {
                            PullRequestPrompt::Open { number, url, .. } => (
                                format!("Open PR #{}", number),
                                format!("Open existing pull request\n{}", url),
                            ),
                            PullRequestPrompt::Create { branch, .. } => (
                                "Create PR...".into(),
                                format!("Open GitHub pull request creation for {}", branch),
                            ),
                        };

                        if ui
                            .add_enabled(has_repo, egui::Button::new(label))
                            .on_hover_text(hover)
                            .clicked()
                        {
                            state.actions.push(UiAction::LaunchPullRequest);
                        }
                    }

                    if ui
                        .add_enabled(has_repo, egui::Button::new("New Branch..."))
                        .on_hover_text("Create and switch to a new local branch")
                        .clicked()
                    {
                        state.show_create_branch_dialog = true;
                    }

                    if ui
                        .add_enabled(has_repo, egui::Button::new("Cleanup..."))
                        .on_hover_text(
                            "Remove local branches whose remote branch was deleted\n(e.g. after a merged PR). Pull first to refresh.",
                        )
                        .clicked()
                    {
                        state.actions.push(UiAction::OpenCleanupBranches);
                    }

                    let can_create_tag = has_repo
                        && git_ops::can_create_tag_on_branch(&state.branch)
                        && (!has_github_https_origin || self.github_auth_session.is_some());
                    if ui
                        .add_enabled(can_create_tag, egui::Button::new("Create Tag..."))
                        .on_hover_text(if can_create_tag {
                            if state.has_origin_remote {
                                "Create a tag from the current HEAD commit and push it to origin"
                            } else {
                                "Create a local tag from the current HEAD commit"
                            }
                        } else if has_github_https_origin && self.github_auth_session.is_none() {
                            "Sign in to GitHub to create and push tags for this repository"
                        } else {
                            "Switch to main or master to create a tag"
                        })
                        .clicked()
                    {
                        state.show_create_tag_dialog = true;
                    }

                    if !state.branch.is_empty() {
                        let prev_branch = state.branch.clone();
                        egui::ComboBox::from_id_salt("branch_selector")
                            .selected_text(&state.branch)
                            .show_ui(ui, |ui| {
                                for branch in &state.branches {
                                    ui.selectable_value(&mut state.branch, branch.clone(), branch);
                                }
                            });

                        if state.branch != prev_branch {
                            let new_branch = state.branch.clone();
                            state.actions.push(UiAction::SwitchBranch(new_branch));
                        }
                    }
                });

                if publish_clicked {
                    self.open_publish_repo_dialog(repo_path);
                }
                if github_sign_in_clicked {
                    self.begin_github_sign_in("Requesting GitHub sign-in code...");
                }
            });
        });

        if let Some(index) = next_active {
            self.active_tab = index;
        }

        if open_clicked {
            self.open_repo_dialog();
        }
        if settings_clicked {
            self.open_settings_dialog();
        }
    }

    fn show_welcome(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 3.0);
            ui.heading("Just Another Git GUI");
            ui.add_space(12.0);
            ui.label("Open a Git repository or publish the current folder to GitHub.");
            ui.add_space(8.0);
            if ui.button("Open Repository...").clicked() {
                self.open_repo_dialog();
            }
            if ui.button("Publish Folder to GitHub...").clicked() {
                self.open_publish_repo_dialog(None);
            }
            if ui.button("Settings...").clicked() {
                self.open_settings_dialog();
            }
            ui.add_space(12.0);
            ui.weak(&self.welcome_status);
            if self.logger.has_entries() && ui.button("View Logs").clicked() {
                self.show_log_viewer = true;
            }
        });
    }

    fn show_settings_dialog(&mut self, ctx: &egui::Context) {
        if !self.settings_dialog.show {
            return;
        }

        let mut keep_open = self.settings_dialog.show;
        let mut close_requested = false;
        let mut selected_ruleset = self.settings.commit_message_ruleset;
        let mut custom_scope_error = None;

        egui::Window::new("Settings")
            .id(egui::Id::new("settings_dialog"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .open(&mut keep_open)
            .show(ctx, |ui| {
                ui.label("Commit message rules");
                egui::ComboBox::from_id_salt("commit_message_ruleset")
                    .selected_text(selected_ruleset.display_name())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut selected_ruleset,
                            CommitMessageRuleSet::Off,
                            CommitMessageRuleSet::Off.display_name(),
                        );
                        ui.selectable_value(
                            &mut selected_ruleset,
                            CommitMessageRuleSet::ConventionalCommits,
                            CommitMessageRuleSet::ConventionalCommits.display_name(),
                        );
                    });

                ui.add_space(8.0);
                if let Some(description) = selected_ruleset.description() {
                    ui.weak(description);
                    ui.add_space(6.0);
                    ui.weak("Type a prefix like `fix` to get scope suggestions.");
                } else {
                    ui.weak("Leave this off to allow any commit message format.");
                }

                ui.add_space(10.0);
                ui.label("Custom commit scopes");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings_dialog.custom_scopes_input)
                        .desired_width(320.0)
                        .hint_text("ui, settings, worker"),
                );

                match commit_rules::parse_custom_scopes(&self.settings_dialog.custom_scopes_input) {
                    Ok(scopes) => {
                        if scopes.is_empty() {
                            ui.weak("Optional. Add comma-separated scopes to keep them available in autocomplete.");
                        } else {
                            ui.weak("Custom scopes stay available alongside inferred scopes.");
                        }
                    }
                    Err(error) => {
                        custom_scope_error = Some(error.clone());
                        ui.colored_label(
                            egui::Color32::from_rgb(220, 120, 120),
                            error,
                        );
                    }
                }

                if !self.settings_dialog.status.is_empty() {
                    ui.add_space(8.0);
                    ui.colored_label(
                        egui::Color32::from_rgb(220, 120, 120),
                        &self.settings_dialog.status,
                    );
                }

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            close_requested = true;
                        }
                    });
                });
            });

        let parsed_custom_scopes =
            commit_rules::parse_custom_scopes(&self.settings_dialog.custom_scopes_input);
        if custom_scope_error.is_none()
            && (selected_ruleset != self.settings.commit_message_ruleset
                || parsed_custom_scopes.as_ref().ok()
                    != Some(&self.settings.commit_message_custom_scopes))
        {
            let mut next_settings = self.settings.clone();
            next_settings.commit_message_ruleset = selected_ruleset;
            next_settings.commit_message_custom_scopes =
                parsed_custom_scopes.unwrap_or_else(|_| unreachable!());
            match settings::save_app_settings(&next_settings) {
                Ok(()) => {
                    self.settings = next_settings;
                    self.settings_dialog.status.clear();
                }
                Err(error) => {
                    self.logger.log_error("Settings", &error);
                    self.settings_dialog.status = status_message_for_error("Settings", &error);
                    self.set_status_message(self.settings_dialog.status.clone());
                }
            }
        }

        if close_requested {
            keep_open = false;
        }

        self.settings_dialog.show = keep_open;
    }

    fn show_publish_repo_dialog(&mut self, ctx: &egui::Context) {
        if !self.publish_dialog.show {
            return;
        }

        let worker_busy = self.welcome_worker.is_busy();
        let mut keep_open = self.publish_dialog.show;
        let mut close_requested = false;
        let mut choose_folder_clicked = false;
        let mut sign_in_clicked = false;
        let mut create_clicked = false;

        egui::Window::new("Publish Folder to GitHub")
            .id(egui::Id::new("publish_repo_dialog"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .open(&mut keep_open)
            .show(ctx, |ui| {
                ui.label("Folder");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.publish_dialog.folder_path)
                            .desired_width(320.0),
                    );
                    if ui.button("Choose...").clicked() {
                        choose_folder_clicked = true;
                    }
                });

                ui.add_space(8.0);
                ui.label("Repository name");
                ui.add(
                    egui::TextEdit::singleline(&mut self.publish_dialog.repo_name)
                        .desired_width(320.0)
                        .hint_text("owner/repository or repository"),
                );

                ui.add_space(8.0);
                ui.label("Initial commit message");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.publish_dialog.commit_message)
                        .desired_width(320.0),
                );
                let inferred_publish_scopes = commit_rules::infer_commit_scopes(
                    folder_path_from_text(&self.publish_dialog.folder_path),
                    std::iter::empty::<&str>(),
                );
                ui::commit_panel::show_prefix_suggestions(
                    ui,
                    &response,
                    &mut self.publish_dialog.commit_message,
                    self.settings.commit_message_ruleset,
                    &inferred_publish_scopes,
                    &self.settings.commit_message_custom_scopes,
                );

                let commit_message_error = commit_rules::validation_error(
                    self.settings.commit_message_ruleset,
                    &self.publish_dialog.commit_message,
                );
                if let Some(error) = &commit_message_error {
                    ui.add_space(4.0);
                    ui.colored_label(egui::Color32::from_rgb(220, 120, 120), error);
                } else if let Some(description) = self.settings.commit_message_ruleset.description()
                {
                    ui.add_space(4.0);
                    ui.weak(description);
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("Visibility");
                    ui.selectable_value(
                        &mut self.publish_dialog.visibility,
                        git_ops::GithubRepoVisibility::Private,
                        "Private",
                    );
                    ui.selectable_value(
                        &mut self.publish_dialog.visibility,
                        git_ops::GithubRepoVisibility::Public,
                        "Public",
                    );
                });

                ui.add_space(8.0);
                let auth_color = if self.publish_dialog.github_authenticated {
                    egui::Color32::from_rgb(100, 200, 100)
                } else {
                    egui::Color32::from_rgb(220, 180, 100)
                };
                ui.colored_label(auth_color, &self.publish_dialog.github_status);

                if !self.publish_dialog.operation_status.is_empty() {
                    ui.add_space(4.0);
                    ui.weak(&self.publish_dialog.operation_status);
                }

                if worker_busy {
                    ui.add_space(8.0);
                    ui.weak("Working...");
                }

                let can_create = !worker_busy
                    && self.publish_dialog.github_authenticated
                    && !self.publish_dialog.folder_path.trim().is_empty()
                    && !self.publish_dialog.repo_name.trim().is_empty()
                    && !self.publish_dialog.commit_message.trim().is_empty()
                    && commit_message_error.is_none();

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!worker_busy, egui::Button::new("Sign In with GitHub"))
                        .clicked()
                    {
                        sign_in_clicked = true;
                    }

                    if ui
                        .add_enabled(can_create, egui::Button::new("Create Repository"))
                        .clicked()
                    {
                        create_clicked = true;
                    }

                    if ui
                        .add_enabled(!worker_busy, egui::Button::new("Cancel"))
                        .clicked()
                    {
                        close_requested = true;
                    }
                });
            });

        if choose_folder_clicked {
            let mut dialog = rfd::FileDialog::new();
            if !self.publish_dialog.folder_path.trim().is_empty() {
                dialog = dialog.set_directory(self.publish_dialog.folder_path.trim());
            }

            if let Some(path) = dialog.pick_folder() {
                self.publish_dialog.set_folder(path);
            }
        }

        if sign_in_clicked {
            self.begin_github_sign_in("Requesting GitHub sign-in code...");
        }

        if create_clicked {
            if let Some(auth) = self.github_auth_session.clone() {
                let commit_message = self.publish_dialog.commit_message.trim().to_string();
                match commit_rules::validate_for_submit(
                    self.settings.commit_message_ruleset,
                    &commit_message,
                ) {
                    Ok(()) => {
                        let folder_path = PathBuf::from(self.publish_dialog.folder_path.trim());
                        self.publish_dialog.operation_status =
                            "Publishing folder to GitHub...".into();
                        self.welcome_worker
                            .create_github_repo(git_ops::CreateGithubRepoRequest {
                                folder_path,
                                repo_name: self.publish_dialog.repo_name.trim().to_string(),
                                commit_message,
                                visibility: self.publish_dialog.visibility,
                                auth,
                            });
                    }
                    Err(error) => {
                        self.publish_dialog.operation_status = error;
                    }
                }
            } else {
                self.publish_dialog.operation_status =
                    "Sign in to GitHub before creating a repository.".into();
            }
        }

        if close_requested {
            keep_open = false;
        }

        self.publish_dialog.show = keep_open;
    }

    fn show_create_branch_dialog(&mut self, ctx: &egui::Context) {
        let active_index = self.active_tab.min(self.tabs.len() - 1);
        let state = &mut self.tabs[active_index].state;

        if !state.show_create_branch_dialog {
            return;
        }

        let mut keep_open = state.show_create_branch_dialog;
        let mut close_requested = false;
        let mut submit_branch = None;

        egui::Window::new("Create Branch")
            .id(egui::Id::new("create_branch_dialog"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .open(&mut keep_open)
            .show(ctx, |ui| {
                ui.label("Branch name");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut state.new_branch_name)
                        .desired_width(260.0)
                        .hint_text("feature/my-branch"),
                );
                let can_create = !state.new_branch_name.trim().is_empty();

                if response.lost_focus()
                    && ui.input(|input| input.key_pressed(egui::Key::Enter))
                    && can_create
                {
                    submit_branch = Some(state.new_branch_name.trim().to_string());
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(can_create, egui::Button::new("Create"))
                            .clicked()
                        {
                            submit_branch = Some(state.new_branch_name.trim().to_string());
                        }

                        if ui.button("Cancel").clicked() {
                            close_requested = true;
                        }
                    });
                });
            });

        if let Some(branch_name) = submit_branch {
            state
                .actions
                .push(UiAction::OpenCreateBranchConfirm(branch_name));
        }

        if close_requested {
            state.new_branch_name.clear();
            state.show_create_branch_dialog = false;
        } else {
            state.show_create_branch_dialog = keep_open;
            if !keep_open {
                state.new_branch_name.clear();
            }
        }
    }

    fn show_create_branch_confirm_dialog(&mut self, ctx: &egui::Context) {
        let active_index = self.active_tab.min(self.tabs.len() - 1);
        let state = &mut self.tabs[active_index].state;

        if !state.show_create_branch_confirm {
            return;
        }

        let mut keep_open = state.show_create_branch_confirm;
        let mut close_requested = false;
        let mut confirm_requested = false;
        let current_branch = state.branch.clone();
        let preview = state.create_branch_preview.clone().unwrap_or_default();

        egui::Window::new("Create branch with uncommitted changes?")
            .id(egui::Id::new("create_branch_confirm_dialog"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .open(&mut keep_open)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!("New branch: {}", preview.branch_name)).strong(),
                );
                ui.add_space(8.0);
                ui.label(format!(
                    "Your uncommitted changes will come with you to the new branch. \
                     Nothing is lost on '{}' — the changes simply ride along.",
                    current_branch
                ));
                ui.add_space(10.0);

                ui.label("Changes traveling with you:");
                ui.indent("create_branch_preview", |ui| {
                    if preview.dirty_files > 0 {
                        ui.label(format!("• {} modified file(s)", preview.dirty_files));
                    }
                    if preview.staged_files > 0 {
                        ui.label(format!("• {} staged file(s)", preview.staged_files));
                    }
                    if preview.untracked_files > 0 {
                        ui.label(format!("• {} untracked file(s)", preview.untracked_files));
                    }
                });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Create branch").clicked() {
                            confirm_requested = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_requested = true;
                        }
                    });
                });
            });

        if confirm_requested {
            state.actions.push(UiAction::ConfirmCreateBranch);
        }

        if close_requested {
            state.show_create_branch_confirm = false;
            state.create_branch_preview = None;
            state.pending_new_branch_name = None;
            state.new_branch_name.clear();
        } else {
            state.show_create_branch_confirm = keep_open;
            if !keep_open {
                state.create_branch_preview = None;
                state.pending_new_branch_name = None;
                state.new_branch_name.clear();
            }
        }
    }

    fn show_create_tag_dialog(&mut self, ctx: &egui::Context) {
        let active_index = self.active_tab.min(self.tabs.len() - 1);
        let state = &mut self.tabs[active_index].state;

        if !state.show_create_tag_dialog {
            return;
        }

        let mut keep_open = state.show_create_tag_dialog;
        let mut close_requested = false;
        let mut submit_tag = None;
        let can_create_tag = git_ops::can_create_tag_on_branch(&state.branch)
            && (!state.has_github_https_origin || self.github_auth_session.is_some());

        egui::Window::new("Create Tag")
            .id(egui::Id::new("create_tag_dialog"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .open(&mut keep_open)
            .show(ctx, |ui| {
                ui.label(format!("Current branch: {}", state.branch));
                ui.add_space(6.0);
                ui.label("Tag name");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut state.new_tag_name)
                        .desired_width(260.0)
                        .hint_text("v1.0.0"),
                );
                let can_submit = can_create_tag && !state.new_tag_name.trim().is_empty();

                if response.lost_focus()
                    && ui.input(|input| input.key_pressed(egui::Key::Enter))
                    && can_submit
                {
                    submit_tag = Some(state.new_tag_name.trim().to_string());
                }

                ui.add_space(8.0);
                if can_create_tag {
                    if state.has_origin_remote {
                        ui.weak("The tag will be pushed to origin after it is created.");
                    } else {
                        ui.weak("No origin remote is configured, so the tag will be local only.");
                    }
                } else if state.has_github_https_origin && self.github_auth_session.is_none() {
                    ui.weak("Sign in to GitHub before creating tags for this repository.");
                } else {
                    ui.weak("Tags can only be created from the main or master branch.");
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(can_submit, egui::Button::new("Create"))
                            .clicked()
                        {
                            submit_tag = Some(state.new_tag_name.trim().to_string());
                        }

                        if ui.button("Cancel").clicked() {
                            close_requested = true;
                        }
                    });
                });
            });

        if let Some(tag_name) = submit_tag {
            state.actions.push(UiAction::CreateTag(tag_name));
        }

        if close_requested {
            state.new_tag_name.clear();
            state.show_create_tag_dialog = false;
        } else {
            state.show_create_tag_dialog = keep_open;
            if !keep_open {
                state.new_tag_name.clear();
            }
        }
    }

    fn show_cleanup_branches_dialog(&mut self, ctx: &egui::Context) {
        let active_index = self.active_tab.min(self.tabs.len() - 1);
        let state = &mut self.tabs[active_index].state;

        if !state.show_cleanup_branches_dialog {
            return;
        }

        let mut keep_open = state.show_cleanup_branches_dialog;
        let mut close_requested = false;
        let mut delete_requested = false;

        egui::Window::new("Clean up branches")
            .id(egui::Id::new("cleanup_branches_dialog"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .open(&mut keep_open)
            .show(ctx, |ui| {
                if state.stale_branches.is_empty() {
                    ui.label("No stale branches to clean up.");
                    ui.add_space(4.0);
                    ui.weak(
                        "A branch is listed here when its upstream has been deleted on the remote.\nPull first to refresh remote tracking.",
                    );
                } else {
                    ui.label("These branches no longer exist on the remote:");
                    ui.add_space(6.0);

                    egui::ScrollArea::vertical()
                        .max_height(260.0)
                        .show(ui, |ui| {
                            for branch in state.stale_branches.iter_mut() {
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut branch.selected, &branch.name);
                                    if branch.merged_into_head {
                                        ui.weak("merged");
                                    } else {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(220, 180, 100),
                                            "unmerged — commits may be lost",
                                        );
                                    }
                                });
                            }
                        });

                    ui.add_space(8.0);
                    let any_selected =
                        state.stale_branches.iter().any(|branch| branch.selected);
                    let any_unmerged_selected = state
                        .stale_branches
                        .iter()
                        .any(|branch| branch.selected && !branch.merged_into_head);

                    if any_unmerged_selected {
                        ui.colored_label(
                            egui::Color32::from_rgb(220, 120, 120),
                            "Warning: deleting an unmerged branch loses its local commits.",
                        );
                        ui.add_space(4.0);
                    }

                    ui.horizontal(|ui| {
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui
                                    .add_enabled(
                                        any_selected,
                                        egui::Button::new("Delete Selected"),
                                    )
                                    .clicked()
                                {
                                    delete_requested = true;
                                }

                                if ui.button("Cancel").clicked() {
                                    close_requested = true;
                                }
                            },
                        );
                    });
                }
            });

        if delete_requested {
            let names: Vec<String> = state
                .stale_branches
                .iter()
                .filter(|branch| branch.selected)
                .map(|branch| branch.name.clone())
                .collect();
            if !names.is_empty() {
                state.actions.push(UiAction::DeleteStaleBranches(names));
            }
        }

        if close_requested {
            state.show_cleanup_branches_dialog = false;
            state.stale_branches.clear();
        } else {
            state.show_cleanup_branches_dialog = keep_open;
            if !keep_open {
                state.stale_branches.clear();
            }
        }
    }

    fn show_discard_dialog(&mut self, ctx: &egui::Context) {
        let active_index = self.active_tab.min(self.tabs.len() - 1);
        let state = &mut self.tabs[active_index].state;

        if !state.show_discard_dialog {
            return;
        }

        let mut keep_open = state.show_discard_dialog;
        let mut close_requested = false;
        let mut confirm_requested = false;
        let branch = state.branch.clone();
        let preview = state.discard_preview.clone().unwrap_or_default();

        egui::Window::new("Discard local changes")
            .id(egui::Id::new("discard_dialog"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .open(&mut keep_open)
            .show(ctx, |ui| {
                ui.label(format!(
                    "This will hard-reset '{}' to origin/{}.",
                    branch, branch
                ));
                ui.add_space(6.0);
                ui.colored_label(
                    egui::Color32::from_rgb(220, 120, 120),
                    "This cannot be undone.",
                );
                ui.add_space(10.0);

                ui.label("You will lose:");
                ui.indent("discard_damage", |ui| {
                    ui.label(format!("• {} modified/staged file(s)", preview.dirty_files));
                    ui.label(format!(
                        "• {} local commit(s) not on origin",
                        preview.local_only_commits
                    ));
                    if state.discard_clean_untracked {
                        ui.label(format!(
                            "• {} untracked file(s)/dir(s)",
                            preview.untracked_files
                        ));
                    } else {
                        ui.weak(format!(
                            "  ({} untracked file(s)/dir(s) will be kept)",
                            preview.untracked_files
                        ));
                    }
                });

                ui.add_space(10.0);
                ui.checkbox(
                    &mut state.discard_clean_untracked,
                    "Also delete untracked files",
                );

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("Discard")
                                        .color(egui::Color32::from_rgb(255, 255, 255)),
                                )
                                .fill(egui::Color32::from_rgb(160, 60, 60)),
                            )
                            .clicked()
                        {
                            confirm_requested = true;
                        }

                        if ui.button("Cancel").clicked() {
                            close_requested = true;
                        }
                    });
                });
            });

        if confirm_requested {
            let clean_untracked = state.discard_clean_untracked;
            state
                .actions
                .push(UiAction::DiscardAndReset { clean_untracked });
        }

        if close_requested {
            state.show_discard_dialog = false;
            state.discard_preview = None;
            state.discard_clean_untracked = false;
        } else {
            state.show_discard_dialog = keep_open;
            if !keep_open {
                state.discard_preview = None;
                state.discard_clean_untracked = false;
            }
        }
    }

    fn show_github_auth_dialog(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.github_auth_prompt.clone() else {
            return;
        };

        egui::Window::new("GitHub Sign In")
            .id(egui::Id::new("github_auth_dialog"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label("Enter this code on GitHub to finish signing in:");
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(&prompt.user_code)
                        .monospace()
                        .size(24.0)
                        .strong(),
                );
                ui.add_space(8.0);
                ui.label("Verification page");
                ui.hyperlink_to(&prompt.verification_uri, &prompt.verification_uri);
                ui.add_space(8.0);
                if ui.button("Open GitHub Again").clicked() {
                    if let Err(error) = webbrowser::open(&prompt.browser_url) {
                        let detail = error.to_string();
                        self.logger.log_error("GitHub sign-in", &detail);
                        self.set_status_message(status_message_for_error(
                            "GitHub sign-in",
                            &detail,
                        ));
                    }
                }
                ui.add_space(4.0);
                ui.weak("This window closes automatically after sign-in completes.");
            });
    }
}

impl eframe::App for GitGuiApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if self.poll_workers() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        if self.tabs.is_empty() {
            self.show_welcome(ui);
            self.show_settings_dialog(&ctx);
            self.show_publish_repo_dialog(&ctx);
            self.show_github_auth_dialog(&ctx);
            self.show_log_viewer_dialog(&ctx);
            return;
        }

        self.show_repo_tabs(ui);
        self.active_tab = self.active_tab.min(self.tabs.len() - 1);
        let has_logs = self.logger.has_entries();

        let open_logs_clicked = {
            let tab = &mut self.tabs[self.active_tab];

            // Render panels (order: top/bottom first, then sides, then center)
            let open_logs = ui::bottom_bar::show(ui, &tab.state, has_logs);
            ui::file_panel::show(ui, &mut tab.state);
            ui::commit_panel::show(
                ui,
                &mut tab.state,
                self.settings.commit_message_ruleset,
                &self.settings.commit_message_custom_scopes,
            );

            egui::CentralPanel::default().show_inside(ui, |ui| {
                ui::diff_panel::show(ui, &mut tab.state);
            });
            open_logs
        };

        if open_logs_clicked {
            self.show_log_viewer = true;
        }

        self.show_publish_repo_dialog(&ctx);
        self.show_settings_dialog(&ctx);
        self.show_create_branch_dialog(&ctx);
        self.show_create_branch_confirm_dialog(&ctx);
        self.show_create_tag_dialog(&ctx);
        self.show_discard_dialog(&ctx);
        self.show_cleanup_branches_dialog(&ctx);
        self.show_github_auth_dialog(&ctx);
        self.show_log_viewer_dialog(&ctx);

        // Process deferred actions
        self.process_actions();
    }
}

fn refresh_status(state: &mut AppState, repo: &Repository) -> Option<String> {
    let mut error_detail = None;
    state.has_origin_remote = git_ops::has_origin_remote(repo);
    state.has_github_origin = git_ops::has_github_origin(repo);
    state.has_github_https_origin = git_ops::has_github_https_origin(repo);
    state.outgoing_commit_count = git_ops::get_outgoing_commit_count(repo).unwrap_or(0);
    match git_ops::get_file_statuses(repo) {
        Ok((unstaged, staged)) => {
            state.unstaged = unstaged;
            state.staged = staged;
            let changed_paths = if state.staged.is_empty() {
                state
                    .unstaged
                    .iter()
                    .map(|file| file.path.as_str())
                    .collect::<Vec<_>>()
            } else {
                state
                    .staged
                    .iter()
                    .map(|file| file.path.as_str())
                    .collect::<Vec<_>>()
            };
            state.inferred_commit_scopes =
                commit_rules::infer_commit_scopes(state.repo_path.as_deref(), changed_paths);
        }
        Err(e) => {
            let detail = e.to_string();
            state.status_msg = status_message_for_error("Refresh", &detail);
            error_detail = Some(detail);
            state.inferred_commit_scopes.clear();
        }
    }
    state.branch = git_ops::get_current_branch(repo).unwrap_or_default();
    state.branches = git_ops::get_branches(repo).unwrap_or_default();
    state.commit_history = git_ops::get_commit_history(repo, 200).unwrap_or_default();
    sync_pull_request_prompt(state);
    sync_selected_file(state, repo);
    error_detail
}

fn reset_repo_view_state(state: &mut AppState) {
    state.has_origin_remote = false;
    state.has_github_origin = false;
    state.has_github_https_origin = false;
    state.branch.clear();
    state.outgoing_commit_count = 0;
    state.branches.clear();
    state.new_branch_name.clear();
    state.show_create_branch_dialog = false;
    state.show_create_branch_confirm = false;
    state.create_branch_preview = None;
    state.pending_new_branch_name = None;
    state.new_tag_name.clear();
    state.show_create_tag_dialog = false;
    state.stale_branches.clear();
    state.show_cleanup_branches_dialog = false;
    state.show_discard_dialog = false;
    state.discard_preview = None;
    state.discard_clean_untracked = false;
    state.unstaged.clear();
    state.staged.clear();
    state.inferred_commit_scopes.clear();
    state.commit_msg.clear();
    state.selected_file = None;
    state.diff_content.clear();
    state.actions.clear();
    state.center_view = CenterView::Diff;
    state.commit_history.clear();
    state.pull_request_prompt = None;
    state.conflict_data = None;
    state.dragging = None;
}

fn folder_path_from_text(folder_path: &str) -> Option<&Path> {
    let trimmed = folder_path.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(Path::new(trimmed))
    }
}

fn sync_selected_file(state: &mut AppState, repo: &Repository) {
    let Some(selected) = state.selected_file.clone() else {
        state.conflict_data = None;
        return;
    };

    let in_unstaged = state.unstaged.iter().any(|file| file.path == selected.path);
    let in_staged = state.staged.iter().any(|file| file.path == selected.path);

    if !in_unstaged && !in_staged {
        state.selected_file = None;
        state.diff_content.clear();
        state.conflict_data = None;
        return;
    }

    let staged = if selected.staged && in_staged {
        true
    } else if !selected.staged && in_unstaged {
        false
    } else {
        in_staged && !in_unstaged
    };

    load_selected_file(state, repo, selected.path, staged);
}

fn load_selected_file(state: &mut AppState, repo: &Repository, path: String, staged: bool) {
    let is_conflicted = state
        .unstaged
        .iter()
        .any(|file| file.path == path && file.is_conflicted);

    if is_conflicted {
        state.selected_file = Some(SelectedFile {
            path: path.clone(),
            staged: false,
        });
        match git_ops::read_conflict_file(repo, &path) {
            Ok(conflict_data) => {
                state.conflict_data = Some(conflict_data);
                state.diff_content.clear();
            }
            Err(e) => {
                state.conflict_data = None;
                state.diff_content = format!("Error loading conflict data: {}", e);
            }
        }
        return;
    }

    state.conflict_data = None;
    match git_ops::get_file_diff(repo, &path, staged) {
        Ok(diff) => state.diff_content = diff,
        Err(e) => state.diff_content = format!("Error loading diff: {}", e),
    }
    state.selected_file = Some(SelectedFile { path, staged });
}

fn repo_root_path(repo: &Repository) -> PathBuf {
    repo.workdir()
        .map(|path| path.to_path_buf())
        .unwrap_or_else(|| repo.path().parent().unwrap_or(repo.path()).to_path_buf())
}

fn repo_tab_label(path: Option<&Path>) -> String {
    path.and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "Repository".into())
}

fn default_repo_name_for_path(path: &Path) -> String {
    repo_tab_label(Some(path))
}

fn sync_pull_request_prompt(state: &mut AppState) {
    let keep_prompt = matches!(
        state.pull_request_prompt.as_ref(),
        Some(PullRequestPrompt::Open { branch, .. } | PullRequestPrompt::Create { branch, .. })
            if branch == &state.branch && state.has_origin_remote
    );

    if !keep_prompt {
        state.pull_request_prompt = None;
    }
}

fn status_message_for_error(context: &str, detail: &str) -> String {
    format!(
        "{} failed: {}. See Logs.",
        context,
        logging::summarize_for_ui(detail)
    )
}
