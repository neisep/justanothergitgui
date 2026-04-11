use std::path::{Path, PathBuf};

use eframe::egui;
use git2::Repository;

use crate::git_ops;
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

pub struct GitGuiApp {
    tabs: Vec<RepoTab>,
    active_tab: usize,
    welcome_status: String,
    welcome_worker: Worker,
    publish_dialog: PublishRepoDialogState,
    github_auth_session: Option<git_ops::GithubAuthSession>,
    github_auth_prompt: Option<git_ops::GithubAuthPrompt>,
}

impl PublishRepoDialogState {
    fn new() -> Self {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut state = Self {
            show: false,
            folder_path: current_dir.display().to_string(),
            repo_name: default_repo_name_for_path(&current_dir),
            commit_message: "Initial commit".into(),
            visibility: git_ops::GithubRepoVisibility::Private,
            github_authenticated: false,
            github_status: String::new(),
            operation_status: String::new(),
        };
        state.set_folder(current_dir);
        state
    }

    fn reset_for_path(&mut self, path: Option<PathBuf>) {
        let path =
            path.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        self.set_folder(path);
        self.commit_message = "Initial commit".into();
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
        let mut startup_status = None;
        let github_auth_session = match git_ops::load_github_auth_session() {
            Ok(Some(session)) => Some(session),
            Ok(None) => None,
            Err(msg) => {
                startup_status = Some(msg);
                None
            }
        };

        let mut app = Self {
            tabs: Vec::new(),
            active_tab: 0,
            welcome_status: "Open a Git repository to get started.".into(),
            welcome_worker: Worker::new(),
            publish_dialog: PublishRepoDialogState::new(),
            github_auth_session,
            github_auth_prompt: None,
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
            Err(e) => self.set_status_message(format!("Failed to open: {}", e)),
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
        refresh_status(&mut state, &repo);
        state.status_msg = format!("Repository loaded: {}", repo_tab_label(Some(&repo_path)));

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
        self.publish_dialog.reset_for_path(path);
        self.publish_dialog.show = true;
        self.refresh_github_auth_status();
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

    fn process_actions(&mut self) {
        if self.tabs.is_empty() {
            return;
        }

        self.active_tab = self.active_tab.min(self.tabs.len() - 1);
        let active_index = self.active_tab;
        let actions: Vec<UiAction> = self.tabs[active_index].state.actions.drain(..).collect();

        for action in actions {
            match action {
                UiAction::StageFile(path) => {
                    let tab = &mut self.tabs[active_index];
                    match git_ops::stage_file(&tab.repo, &path) {
                        Ok(()) => tab.state.status_msg = format!("Staged: {}", path),
                        Err(e) => tab.state.status_msg = format!("Stage error: {}", e),
                    }
                    refresh_status(&mut tab.state, &tab.repo);
                }

                UiAction::UnstageFile(path) => {
                    let tab = &mut self.tabs[active_index];
                    match git_ops::unstage_file(&tab.repo, &path) {
                        Ok(()) => tab.state.status_msg = format!("Unstaged: {}", path),
                        Err(e) => tab.state.status_msg = format!("Unstage error: {}", e),
                    }
                    refresh_status(&mut tab.state, &tab.repo);
                }

                UiAction::StageAll => {
                    let tab = &mut self.tabs[active_index];
                    match git_ops::stage_all(&tab.repo) {
                        Ok(()) => tab.state.status_msg = "Staged all changes".into(),
                        Err(e) => tab.state.status_msg = format!("Stage all error: {}", e),
                    }
                    refresh_status(&mut tab.state, &tab.repo);
                }

                UiAction::UnstageAll => {
                    let tab = &mut self.tabs[active_index];
                    match git_ops::unstage_all(&tab.repo) {
                        Ok(()) => tab.state.status_msg = "Unstaged all changes".into(),
                        Err(e) => tab.state.status_msg = format!("Unstage all error: {}", e),
                    }
                    refresh_status(&mut tab.state, &tab.repo);
                }

                UiAction::Commit => {
                    let tab = &mut self.tabs[active_index];
                    let msg = tab.state.commit_msg.trim().to_string();
                    match git_ops::create_commit(&tab.repo, &msg) {
                        Ok(oid) => {
                            tab.state.status_msg = format!("Committed: {}", &oid.to_string()[..8]);
                            tab.state.commit_msg.clear();
                            tab.state.selected_file = None;
                            tab.state.diff_content.clear();
                            tab.state.conflict_data = None;
                        }
                        Err(e) => {
                            tab.state.status_msg = format!("Commit error: {}", e);
                        }
                    }
                    refresh_status(&mut tab.state, &tab.repo);
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
                            tab.worker.pull(path);
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
                            tab.state.status_msg = format!("Switch error: {}", e);
                        }
                    }
                    refresh_status(&mut tab.state, &tab.repo);
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
                        }
                        Err(e) => {
                            tab.state.status_msg = format!("Create branch error: {}", e);
                        }
                    }
                    refresh_status(&mut tab.state, &tab.repo);
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
                                tab.state.status_msg = format!("Save resolution error: {}", e);
                            }
                        }
                    } else {
                        tab.state.status_msg = "No conflict selected".into();
                    }
                    refresh_status(&mut tab.state, &tab.repo);
                }
            }
        }
    }

    fn poll_workers(&mut self) -> bool {
        let mut refresh_indices = Vec::new();
        let mut any_busy = false;

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
                        Err(error) => format!(
                            "GitHub sign-in complete for @{}, but {}",
                            session.login, error
                        ),
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
                    self.github_auth_prompt = None;
                    self.publish_dialog.github_authenticated = self.github_auth_session.is_some();
                    self.publish_dialog.github_status =
                        if let Some(session) = &self.github_auth_session {
                            format!(
                                "Signed in to GitHub as @{} (latest sign-in failed: {})",
                                session.login, msg
                            )
                        } else {
                            format!("GitHub sign-in failed: {}", msg)
                        };
                    self.publish_dialog.operation_status.clear();
                    self.welcome_status = format!("GitHub sign-in failed: {}", msg);
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
                    self.publish_dialog.operation_status = msg.clone();
                    self.welcome_status = format!("Publish failed: {}", msg);
                }
                TaskResult::Push(_)
                | TaskResult::Pull(_)
                | TaskResult::OpenPullRequest(_)
                | TaskResult::CreatePullRequest(_) => {}
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
                    }
                    TaskResult::Push(Err(msg)) => {
                        tab.state.status_msg = format!("Push failed: {}", msg)
                    }
                    TaskResult::Pull(Ok(msg)) => {
                        tab.state.status_msg = format!("Pull: {}", msg);
                        refresh_indices.push(index);
                    }
                    TaskResult::Pull(Err(msg)) => {
                        tab.state.status_msg = format!("Pull failed: {}", msg)
                    }
                    TaskResult::OpenPullRequest(Ok(msg)) => {
                        tab.state.status_msg = msg;
                    }
                    TaskResult::OpenPullRequest(Err(msg)) => {
                        tab.state.status_msg = format!("Open PR failed: {}", msg);
                    }
                    TaskResult::CreatePullRequest(Ok(msg)) => {
                        tab.state.status_msg = msg;
                    }
                    TaskResult::CreatePullRequest(Err(msg)) => {
                        tab.state.status_msg = format!("Create PR failed: {}", msg);
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

            if let Ok(repo) = git_ops::open_repo(&path) {
                refresh_status(&mut self.tabs[index].state, &repo);
                self.tabs[index].repo = repo;
            }
        }

        any_busy
    }

    fn show_repo_tabs(&mut self, ui: &mut egui::Ui) {
        let mut next_active = None;
        let mut open_clicked = false;
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
                    320.0
                } else {
                    460.0
                };

                if ui.button("Open...").clicked() {
                    open_clicked = true;
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
                let needs_github_sign_in = has_github_origin && self.github_auth_session.is_none();
                let pull_request_prompt = state.pull_request_prompt.clone();
                let mut publish_clicked = false;
                let mut github_sign_in_clicked = false;
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if has_origin_remote {
                        ui.add_enabled_ui(has_repo, |ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("Push").on_hover_text("Push to remote").clicked() {
                                        state.actions.push(UiAction::Push);
                                    }
                                    if ui
                                        .button("Pull")
                                        .on_hover_text("Pull from remote")
                                        .clicked()
                                    {
                                        state.actions.push(UiAction::Pull);
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
            ui.add_space(12.0);
            ui.weak(&self.welcome_status);
        });
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
                ui.add(
                    egui::TextEdit::singleline(&mut self.publish_dialog.commit_message)
                        .desired_width(320.0),
                );

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
                    && !self.publish_dialog.commit_message.trim().is_empty();

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
                let folder_path = PathBuf::from(self.publish_dialog.folder_path.trim());
                self.publish_dialog.operation_status = "Publishing folder to GitHub...".into();
                self.welcome_worker
                    .create_github_repo(git_ops::CreateGithubRepoRequest {
                        folder_path,
                        repo_name: self.publish_dialog.repo_name.trim().to_string(),
                        commit_message: self.publish_dialog.commit_message.trim().to_string(),
                        visibility: self.publish_dialog.visibility,
                        auth,
                    });
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
            state.actions.push(UiAction::CreateBranch(branch_name));
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
                    let _ = webbrowser::open(&prompt.browser_url);
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
            self.show_publish_repo_dialog(&ctx);
            self.show_github_auth_dialog(&ctx);
            return;
        }

        self.show_repo_tabs(ui);
        self.active_tab = self.active_tab.min(self.tabs.len() - 1);

        {
            let tab = &mut self.tabs[self.active_tab];

            // Render panels (order: top/bottom first, then sides, then center)
            ui::bottom_bar::show(ui, &tab.state);
            ui::file_panel::show(ui, &mut tab.state);
            ui::commit_panel::show(ui, &mut tab.state);

            egui::CentralPanel::default().show_inside(ui, |ui| {
                ui::diff_panel::show(ui, &mut tab.state);
            });
        }

        self.show_publish_repo_dialog(&ctx);
        self.show_create_branch_dialog(&ctx);
        self.show_github_auth_dialog(&ctx);

        // Process deferred actions
        self.process_actions();
    }
}

fn refresh_status(state: &mut AppState, repo: &Repository) {
    state.has_origin_remote = git_ops::has_origin_remote(repo);
    state.has_github_origin = git_ops::has_github_origin(repo);
    match git_ops::get_file_statuses(repo) {
        Ok((unstaged, staged)) => {
            state.unstaged = unstaged;
            state.staged = staged;
        }
        Err(e) => {
            state.status_msg = format!("Error refreshing: {}", e);
        }
    }
    state.branch = git_ops::get_current_branch(repo).unwrap_or_default();
    state.branches = git_ops::get_branches(repo).unwrap_or_default();
    state.commit_history = git_ops::get_commit_history(repo, 200).unwrap_or_default();
    sync_pull_request_prompt(state);
    sync_selected_file(state, repo);
}

fn reset_repo_view_state(state: &mut AppState) {
    state.has_origin_remote = false;
    state.has_github_origin = false;
    state.branch.clear();
    state.branches.clear();
    state.new_branch_name.clear();
    state.show_create_branch_dialog = false;
    state.unstaged.clear();
    state.staged.clear();
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
