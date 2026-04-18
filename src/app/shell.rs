use super::{helpers, *};

#[derive(Default)]
struct RepoTabsUiOutput {
    next_active: Option<usize>,
    open_clicked: bool,
    settings_clicked: bool,
    show_logs_clicked: bool,
    publish_clicked: bool,
    github_sign_in_clicked: bool,
    create_tag_clicked: bool,
}

struct RepoToolbarModel {
    welcome_busy: Option<BusyState>,
    repo_busy: Option<BusyState>,
    has_logs: bool,
    publish_dialog_open: bool,
    welcome_worker_busy: bool,
    has_repo: bool,
    has_origin_remote: bool,
    needs_github_sign_in: bool,
    repo_worker_busy: bool,
    controls_width: f32,
    can_discard: bool,
    discard_tooltip: String,
    can_undo_last_commit: bool,
    undo_last_commit_tooltip: String,
    can_create_tag: bool,
    create_tag_tooltip: String,
    push_label: String,
    push_tooltip: String,
    pull_request_prompt: Option<PullRequestPrompt>,
}

impl RepoToolbarModel {
    fn from_state(
        state: &AppState,
        welcome_busy: Option<BusyState>,
        welcome_worker_busy: bool,
        github_auth_available: bool,
        publish_dialog_open: bool,
        has_logs: bool,
    ) -> Self {
        let welcome_worker_busy = welcome_busy.is_some() || welcome_worker_busy;
        let repo_busy = state.ui.busy.clone();
        let has_repo = state.repo.path.is_some();
        let has_origin_remote = state.repo.has_origin_remote;
        let has_github_https_origin = state.repo.has_github_https_origin;
        let needs_github_sign_in = state.repo.has_github_origin && !github_auth_available;
        let has_branch = !state.repo.branch.is_empty();
        let github_auth_ok = !has_github_https_origin || github_auth_available;
        let can_discard = has_origin_remote && has_branch && github_auth_ok;
        let discard_tooltip = if !has_origin_remote {
            "Add or fetch an origin remote before resetting to origin".to_string()
        } else if !has_branch {
            "Check out a branch to reset it to origin".to_string()
        } else if !github_auth_ok {
            "Sign in to GitHub to reset to origin".to_string()
        } else {
            format!(
                "Discard local changes and reset '{}' to origin/{}",
                state.repo.branch, state.repo.branch
            )
        };
        let can_create_tag = has_repo
            && AppRepoRead::can_create_tag_on_branch(&state.repo.branch)
            && (!has_github_https_origin || github_auth_available);
        let create_tag_tooltip = if can_create_tag {
            if has_origin_remote {
                "Create a tag from the current HEAD commit and push it to origin".to_string()
            } else {
                "Create a local tag from the current HEAD commit".to_string()
            }
        } else if has_github_https_origin && !github_auth_available {
            "Sign in to GitHub to create and push tags for this repository".to_string()
        } else {
            "Switch to main or master to create a tag".to_string()
        };
        let can_undo_last_commit = has_repo && state.repo.outgoing_commit_count > 0;
        let undo_last_commit_tooltip = if !has_repo {
            "Open a repository before undoing commits".to_string()
        } else if state.repo.outgoing_commit_count == 0 {
            "No local-only commits to undo".to_string()
        } else {
            "Remove the most recent local-only commit and keep its changes staged".to_string()
        };
        let push_label = if state.repo.outgoing_commit_count > 0 {
            format!("Push({})", state.repo.outgoing_commit_count)
        } else {
            "Push".into()
        };
        let push_tooltip = if state.repo.outgoing_commit_count > 0 {
            format!(
                "Push {} local commit(s) to remote",
                state.repo.outgoing_commit_count
            )
        } else {
            "Push to remote".into()
        };

        Self {
            welcome_busy,
            repo_busy: repo_busy.clone(),
            has_logs,
            publish_dialog_open,
            welcome_worker_busy,
            has_repo,
            has_origin_remote,
            needs_github_sign_in,
            repo_worker_busy: repo_busy.is_some(),
            controls_width: if state.repo.branch.is_empty() {
                380.0
            } else {
                560.0
            },
            can_discard,
            discard_tooltip,
            can_undo_last_commit,
            undo_last_commit_tooltip,
            can_create_tag,
            create_tag_tooltip,
            push_label,
            push_tooltip,
            pull_request_prompt: state.repo.pull_request_prompt.clone(),
        }
    }
}

impl GitGuiApp {
    pub(super) fn refresh_active_tab(&mut self) {
        let Some(active_index) = self.normalize_active_tab() else {
            return;
        };
        let Some(path) = self.tabs[active_index].state.repo.path.clone() else {
            return;
        };

        if self.tabs[active_index].worker.is_busy() {
            self.tabs[active_index].state.ui.status_msg = "Busy — please wait...".into();
            return;
        }

        match AppRepoRead::open(&path) {
            Ok(repo) => {
                let refresh_result = {
                    let (repo_state, worktree_state, commit_state, inspector_state, ui_state) =
                        self.tabs[active_index].state.refresh_parts_mut();
                    helpers::refresh_status(
                        repo_state,
                        worktree_state,
                        commit_state,
                        inspector_state,
                        ui_state,
                        &repo,
                    )
                };
                if let Some(detail) = refresh_result {
                    self.tabs[active_index].state.ui.status_msg =
                        helpers::status_message_for_error("Refresh", &detail);
                    self.logger.log_error("Refresh", &detail);
                } else {
                    self.tabs[active_index].state.ui.status_msg =
                        "Refreshed repository status".into();
                }
                self.tabs[active_index].repo = repo;
            }
            Err(error) => {
                let detail = error.to_string();
                self.tabs[active_index].state.ui.status_msg =
                    helpers::status_message_for_error("Refresh", &detail);
                self.logger.log_error("Refresh", &detail);
            }
        }
    }

    pub(super) fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        if self.any_dialog_open() {
            if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                self.close_topmost_dialog();
            }
            return;
        }

        let Some(active_index) = self.normalize_active_tab() else {
            return;
        };

        if ctx.input_mut(|input| input.consume_shortcut(&SHORTCUT_FOCUS_COMMIT)) {
            self.tabs[active_index]
                .state
                .commit
                .focus_commit_summary_requested = true;
        }

        if ctx.input_mut(|input| input.consume_shortcut(&SHORTCUT_STAGE_SELECTED_FILE)) {
            let selected_file = self.tabs[active_index]
                .state
                .inspector
                .selected_file
                .clone();
            match selected_file {
                Some(selected) if selected.staged => self.tabs[active_index]
                    .state
                    .ui
                    .actions
                    .push(UiAction::unstage_file(selected.path)),
                Some(selected) => self.tabs[active_index]
                    .state
                    .ui
                    .actions
                    .push(UiAction::stage_file(selected.path)),
                None => {
                    self.tabs[active_index].state.ui.status_msg =
                        "Select a file to stage or unstage first".into();
                }
            }
        }

        if ctx.input_mut(|input| input.consume_shortcut(&SHORTCUT_COMMIT)) {
            let tab = &mut self.tabs[active_index];
            let message = commit_rules::build_message(
                &tab.state.commit.commit_summary,
                &tab.state.commit.commit_body,
            );
            let validation_error =
                commit_rules::validation_error(self.settings.commit_message_ruleset, &message);

            if tab.state.worktree.staged.is_empty() {
                tab.state.ui.status_msg = "Stage files first".into();
            } else if tab.state.commit.commit_summary.trim().is_empty() {
                tab.state.ui.status_msg = "Enter a commit summary".into();
                tab.state.commit.focus_commit_summary_requested = true;
            } else if let Some(error) = validation_error {
                tab.state.ui.status_msg = error;
                tab.state.commit.focus_commit_summary_requested = true;
            } else {
                tab.state.ui.actions.push(UiAction::commit());
            }
        }

        if ctx.input_mut(|input| input.consume_shortcut(&SHORTCUT_REFRESH))
            || ctx.input_mut(|input| input.consume_shortcut(&SHORTCUT_REFRESH_F5))
        {
            self.refresh_active_tab();
        }
    }

    pub(super) fn show_log_viewer_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_log_viewer {
            return;
        }

        let log_path = self.logger.path().display().to_string();
        let mut contents = self.logger.read_entries();
        let output =
            ui::dialogs::log_viewer::show(ctx, self.show_log_viewer, &log_path, &mut contents);

        if output.clear_clicked {
            let result = self.logger.clear_entries();
            match result {
                Ok(()) => self.set_status_message("Logs cleared.".into()),
                Err(error) => {
                    self.set_status_message(helpers::status_message_for_error("Clear logs", &error))
                }
            }
        }

        self.show_log_viewer = output.keep_open;
    }

    pub(super) fn show_repo_tabs(&mut self, ui: &mut egui::Ui) {
        let Some(active_index) = self.normalize_active_tab() else {
            return;
        };
        let tab_labels = self.repo_tab_labels();
        let output = {
            let state = &mut self.tabs[active_index].state;
            let toolbar = RepoToolbarModel::from_state(
                state,
                self.welcome_busy.clone(),
                self.welcome_worker.is_busy(),
                self.github_auth_session.is_some(),
                self.publish_dialog.show,
                self.logger.has_entries(),
            );
            Self::show_repo_tabs_panel(ui, active_index, state, &tab_labels, &toolbar)
        };

        self.apply_repo_tabs_output(active_index, output);
    }

    fn repo_tab_labels(&self) -> Vec<(String, Option<String>)> {
        self.tabs
            .iter()
            .map(|tab| {
                (
                    helpers::repo_tab_label(tab.state.repo.path.as_deref()),
                    tab.state
                        .repo
                        .path
                        .as_ref()
                        .map(|path| path.display().to_string()),
                )
            })
            .collect()
    }

    fn show_repo_tabs_panel(
        ui: &mut egui::Ui,
        active_index: usize,
        state: &mut AppState,
        tab_labels: &[(String, Option<String>)],
        toolbar: &RepoToolbarModel,
    ) -> RepoTabsUiOutput {
        let mut output = RepoTabsUiOutput::default();

        egui::Panel::top("repo_tabs").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                Self::show_repo_menu(ui, state, toolbar, &mut output);

                if !toolbar.publish_dialog_open {
                    if let Some(busy) = &toolbar.welcome_busy {
                        ui::show_inline_busy(ui, &busy.label);
                    }
                }

                ui.separator();
                Self::show_repo_tab_strip(ui, active_index, tab_labels, toolbar, &mut output);
                ui.separator();
                Self::show_repo_toolbar_actions(ui, state, toolbar);
            });
        });

        output
    }

    fn show_repo_menu(
        ui: &mut egui::Ui,
        state: &mut AppState,
        toolbar: &RepoToolbarModel,
        output: &mut RepoTabsUiOutput,
    ) {
        ui.menu_button("More", |ui| {
            if ui.button("Open Repository...").clicked() {
                output.open_clicked = true;
                ui.close();
            }

            ui.separator();

            if ui
                .add_enabled(
                    toolbar.has_repo && !toolbar.has_origin_remote && !toolbar.welcome_worker_busy,
                    egui::Button::new("Publish to GitHub..."),
                )
                .on_hover_text("Create a GitHub repository for this folder and push it")
                .clicked()
            {
                output.publish_clicked = true;
                ui.close();
            }

            if toolbar.needs_github_sign_in
                && ui
                    .add_enabled(
                        toolbar.has_repo && !toolbar.welcome_worker_busy,
                        egui::Button::new("Sign in to GitHub..."),
                    )
                    .on_hover_text(
                        "Sign in so the app can check pull requests and reuse GitHub auth",
                    )
                    .clicked()
            {
                output.github_sign_in_clicked = true;
                ui.close();
            }

            if ui
                .add_enabled(
                    toolbar.can_create_tag && !toolbar.repo_worker_busy,
                    egui::Button::new("Create Tag..."),
                )
                .on_hover_text(&toolbar.create_tag_tooltip)
                .clicked()
            {
                output.create_tag_clicked = true;
                ui.close();
            }

            if ui
                .add_enabled(
                    toolbar.has_repo && !toolbar.repo_worker_busy,
                    egui::Button::new("Cleanup..."),
                )
                .on_hover_text(
                    "Remove local branches whose remote branch was deleted\n(e.g. after a merged PR). Pull first to refresh.",
                )
                .clicked()
            {
                state.ui.actions.push(UiAction::open_cleanup_branches());
                ui.close();
            }

            if ui
                .add_enabled(
                    toolbar.can_undo_last_commit && !toolbar.repo_worker_busy,
                    egui::Button::new("Undo Last Commit"),
                )
                .on_hover_text(&toolbar.undo_last_commit_tooltip)
                .clicked()
            {
                state.ui.actions.push(UiAction::undo_last_commit());
                ui.close();
            }

            ui.separator();
            ui.add_space(4.0);

            if ui
                .add_enabled(
                    toolbar.can_discard && !toolbar.repo_worker_busy,
                    egui::Button::new(
                        egui::RichText::new("Discard...")
                            .color(egui::Color32::from_rgb(200, 80, 80)),
                    ),
                )
                .on_hover_text(&toolbar.discard_tooltip)
                .clicked()
            {
                state.ui.actions.push(UiAction::open_discard_dialog());
                ui.close();
            }

            ui.add_space(4.0);
            ui.separator();

            if ui.button("Settings...").clicked() {
                output.settings_clicked = true;
                ui.close();
            }

            if toolbar.has_logs && ui.button("View Logs").clicked() {
                output.show_logs_clicked = true;
                ui.close();
            }
        });
    }

    fn show_repo_tab_strip(
        ui: &mut egui::Ui,
        active_index: usize,
        tab_labels: &[(String, Option<String>)],
        toolbar: &RepoToolbarModel,
        output: &mut RepoTabsUiOutput,
    ) {
        let tabs_width = (ui.available_width() - toolbar.controls_width).max(120.0);

        egui::ScrollArea::horizontal()
            .id_salt("repo_tabs_scroll")
            .max_width(tabs_width)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (index, (label, tooltip)) in tab_labels.iter().enumerate() {
                        let mut response = ui.selectable_label(index == active_index, label);

                        if let Some(path) = tooltip {
                            response = response.on_hover_text(path);
                        }

                        if response.clicked() {
                            output.next_active = Some(index);
                        }
                    }
                });
            });
    }

    fn show_repo_toolbar_actions(
        ui: &mut egui::Ui,
        state: &mut AppState,
        toolbar: &RepoToolbarModel,
    ) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            Self::show_remote_sync_actions(ui, state, toolbar);
            Self::show_pull_request_action(ui, state, toolbar);
            Self::show_branch_controls(ui, state, toolbar);
        });
    }

    fn show_remote_sync_actions(
        ui: &mut egui::Ui,
        state: &mut AppState,
        toolbar: &RepoToolbarModel,
    ) {
        if !toolbar.has_origin_remote {
            return;
        }

        ui.add_enabled_ui(toolbar.has_repo, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        !toolbar.repo_worker_busy,
                        egui::Button::new(&toolbar.push_label),
                    )
                    .on_hover_text(&toolbar.push_tooltip)
                    .clicked()
                {
                    state.ui.actions.push(UiAction::push());
                }
                if ui
                    .add_enabled(!toolbar.repo_worker_busy, egui::Button::new("Pull"))
                    .on_hover_text("Pull from remote")
                    .clicked()
                {
                    state.ui.actions.push(UiAction::pull());
                }
                if let Some(busy) = toolbar
                    .repo_busy
                    .as_ref()
                    .filter(|busy| matches!(busy.action, BusyAction::Push | BusyAction::Pull))
                {
                    ui::show_inline_busy(ui, &busy.label);
                }
            });
        });
    }

    fn show_pull_request_action(
        ui: &mut egui::Ui,
        state: &mut AppState,
        toolbar: &RepoToolbarModel,
    ) {
        let Some(prompt) = &toolbar.pull_request_prompt else {
            return;
        };

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
            .add_enabled(
                toolbar.has_repo && !toolbar.repo_worker_busy,
                egui::Button::new(label),
            )
            .on_hover_text(hover)
            .clicked()
        {
            state.ui.actions.push(UiAction::launch_pull_request());
        }
        if let Some(busy) = toolbar.repo_busy.as_ref().filter(|busy| {
            matches!(
                busy.action,
                BusyAction::OpenPullRequest | BusyAction::CreatePullRequest
            )
        }) {
            ui::show_inline_busy(ui, &busy.label);
        }
    }

    fn show_branch_controls(ui: &mut egui::Ui, state: &mut AppState, toolbar: &RepoToolbarModel) {
        ui.add_enabled_ui(!toolbar.repo_worker_busy, |ui| {
            if ui
                .add_enabled(toolbar.has_repo, egui::Button::new("New Branch..."))
                .on_hover_text("Create and switch to a new local branch")
                .clicked()
            {
                state.dialogs.branch.show_create_branch_dialog = true;
                state.dialogs.branch.focus_new_branch_name_requested = true;
            }

            if !state.repo.branch.is_empty() {
                let prev_branch = state.repo.branch.clone();
                egui::ComboBox::from_id_salt("branch_selector")
                    .selected_text(&state.repo.branch)
                    .show_ui(ui, |ui| {
                        for branch in &state.repo.branches {
                            ui.selectable_value(&mut state.repo.branch, branch.clone(), branch);
                        }
                    });

                if state.repo.branch != prev_branch {
                    state
                        .ui
                        .actions
                        .push(UiAction::switch_branch(state.repo.branch.clone()));
                }
            }
        });
    }

    fn apply_repo_tabs_output(&mut self, active_index: usize, output: RepoTabsUiOutput) {
        if let Some(index) = output.next_active {
            self.active_tab = index;
        }
        if output.open_clicked {
            self.open_repo_dialog();
        }
        if output.settings_clicked {
            self.open_settings_dialog();
        }
        if output.publish_clicked {
            self.open_publish_repo_dialog(self.tabs[active_index].state.repo.path.clone());
        }
        if output.github_sign_in_clicked {
            self.begin_github_sign_in("Requesting GitHub sign-in code...");
        }
        if output.create_tag_clicked {
            let tab = &mut self.tabs[active_index];
            tab.state.dialogs.tag.new_tag_name = AppRepoRead::suggest_next_tag(&tab.repo);
            tab.state.dialogs.tag.show_create_tag_dialog = true;
            tab.state.dialogs.tag.focus_new_tag_name_requested = true;
        }
        if output.show_logs_clicked {
            self.show_log_viewer = true;
        }
    }

    pub(super) fn show_welcome(&mut self, ui: &mut egui::Ui) {
        let welcome_busy = self.welcome_busy.clone();
        let worker_busy = welcome_busy.is_some();
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 3.0);
            ui.heading("Just Another Git GUI");
            ui.add_space(12.0);
            ui.label("Open a Git repository or publish the current folder to GitHub.");
            ui.add_space(8.0);
            if ui.button("Open Repository...").clicked() {
                self.open_repo_dialog();
            }
            if ui
                .add_enabled(!worker_busy, egui::Button::new("Clone Repository..."))
                .clicked()
            {
                self.open_clone_repo_dialog();
            }
            if ui
                .add_enabled(
                    !worker_busy,
                    egui::Button::new("Publish Folder to GitHub..."),
                )
                .clicked()
            {
                self.open_publish_repo_dialog(None);
            }
            if let Some(session) = &self.github_auth_session {
                ui.weak(format!("Signed in to GitHub as @{}", session.login));
            } else if ui
                .add_enabled(!worker_busy, egui::Button::new("Sign in to GitHub..."))
                .clicked()
            {
                self.begin_github_sign_in("Requesting GitHub sign-in code...");
            }
            if let Some(busy) = &welcome_busy {
                ui::show_inline_busy(ui, &busy.label);
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
}
