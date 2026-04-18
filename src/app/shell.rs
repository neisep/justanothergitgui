use super::{helpers, *};

impl GitGuiApp {
    pub(super) fn refresh_active_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }

        let active_index = self.active_tab.min(self.tabs.len() - 1);
        let Some(path) = self.tabs[active_index].state.repo_path.clone() else {
            return;
        };

        if self.tabs[active_index].worker.is_busy() {
            self.tabs[active_index].state.status_msg = "Busy — please wait...".into();
            return;
        }

        match git_ops::open_repo(&path) {
            Ok(repo) => {
                if let Some(detail) =
                    helpers::refresh_status(&mut self.tabs[active_index].state, &repo)
                {
                    self.tabs[active_index].state.status_msg =
                        helpers::status_message_for_error("Refresh", &detail);
                    self.logger.log_error("Refresh", &detail);
                } else {
                    self.tabs[active_index].state.status_msg = "Refreshed repository status".into();
                }
                self.tabs[active_index].repo = repo;
            }
            Err(error) => {
                let detail = error.to_string();
                self.tabs[active_index].state.status_msg =
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

        if self.tabs.is_empty() {
            return;
        }

        let active_index = self.active_tab.min(self.tabs.len() - 1);

        if ctx.input_mut(|input| input.consume_shortcut(&SHORTCUT_FOCUS_COMMIT)) {
            self.tabs[active_index].state.focus_commit_summary_requested = true;
        }

        if ctx.input_mut(|input| input.consume_shortcut(&SHORTCUT_STAGE_SELECTED_FILE)) {
            let selected_file = self.tabs[active_index].state.selected_file.clone();
            match selected_file {
                Some(selected) if selected.staged => self.tabs[active_index]
                    .state
                    .actions
                    .push(UiAction::UnstageFile(selected.path)),
                Some(selected) => self.tabs[active_index]
                    .state
                    .actions
                    .push(UiAction::StageFile(selected.path)),
                None => {
                    self.tabs[active_index].state.status_msg =
                        "Select a file to stage or unstage first".into();
                }
            }
        }

        if ctx.input_mut(|input| input.consume_shortcut(&SHORTCUT_COMMIT)) {
            let tab = &mut self.tabs[active_index];
            let message =
                commit_rules::build_message(&tab.state.commit_summary, &tab.state.commit_body);
            let validation_error =
                commit_rules::validation_error(self.settings.commit_message_ruleset, &message);

            if tab.state.staged.is_empty() {
                tab.state.status_msg = "Stage files first".into();
            } else if tab.state.commit_summary.trim().is_empty() {
                tab.state.status_msg = "Enter a commit summary".into();
                tab.state.focus_commit_summary_requested = true;
            } else if let Some(error) = validation_error {
                tab.state.status_msg = error;
                tab.state.focus_commit_summary_requested = true;
            } else {
                tab.state.actions.push(UiAction::Commit);
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
        let mut next_active = None;
        let mut open_clicked = false;
        let mut settings_clicked = false;
        let mut show_logs_clicked = false;
        let active_index = self.active_tab.min(self.tabs.len() - 1);
        let has_logs = self.logger.has_entries();
        let tab_labels: Vec<(String, Option<String>)> = self
            .tabs
            .iter()
            .map(|tab| {
                (
                    helpers::repo_tab_label(tab.state.repo_path.as_deref()),
                    tab.state
                        .repo_path
                        .as_ref()
                        .map(|path| path.display().to_string()),
                )
            })
            .collect();

        egui::Panel::top("repo_tabs").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let welcome_busy = self.welcome_busy.clone();
                let state = &mut self.tabs[active_index].state;
                let repo_busy = state.busy.clone();
                let repo_worker_busy = repo_busy.is_some();
                let welcome_worker_busy = welcome_busy.is_some() || self.welcome_worker.is_busy();
                let controls_width = if state.branch.is_empty() { 380.0 } else { 560.0 };
                let has_repo = state.repo_path.is_some();
                let repo_path = state.repo_path.clone();
                let has_origin_remote = state.has_origin_remote;
                let has_github_origin = state.has_github_origin;
                let has_github_https_origin = state.has_github_https_origin;
                let needs_github_sign_in = has_github_origin && self.github_auth_session.is_none();
                let has_branch = !state.branch.is_empty();
                let github_auth_ok = !has_github_https_origin || self.github_auth_session.is_some();
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
                        state.branch, state.branch
                    )
                };
                let can_create_tag = has_repo
                    && git_ops::can_create_tag_on_branch(&state.branch)
                    && (!has_github_https_origin || self.github_auth_session.is_some());
                let create_tag_tooltip = if can_create_tag {
                    if state.has_origin_remote {
                        "Create a tag from the current HEAD commit and push it to origin"
                    } else {
                        "Create a local tag from the current HEAD commit"
                    }
                } else if has_github_https_origin && self.github_auth_session.is_none() {
                    "Sign in to GitHub to create and push tags for this repository"
                } else {
                    "Switch to main or master to create a tag"
                };
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
                let mut create_tag_clicked = false;

                ui.menu_button("More", |ui| {
                    if ui.button("Open Repository...").clicked() {
                        open_clicked = true;
                        ui.close();
                    }

                    ui.separator();

                    if ui
                        .add_enabled(
                            has_repo && !has_origin_remote && !welcome_worker_busy,
                            egui::Button::new("Publish to GitHub..."),
                        )
                        .on_hover_text("Create a GitHub repository for this folder and push it")
                        .clicked()
                    {
                        publish_clicked = true;
                        ui.close();
                    }

                    if needs_github_sign_in
                        && ui
                            .add_enabled(
                                has_repo && !welcome_worker_busy,
                                egui::Button::new("Sign in to GitHub..."),
                            )
                            .on_hover_text(
                                "Sign in so the app can check pull requests and reuse GitHub auth",
                            )
                            .clicked()
                    {
                        github_sign_in_clicked = true;
                        ui.close();
                    }

                    if ui
                        .add_enabled(
                            can_create_tag && !repo_worker_busy,
                            egui::Button::new("Create Tag..."),
                        )
                        .on_hover_text(create_tag_tooltip)
                        .clicked()
                    {
                        create_tag_clicked = true;
                        ui.close();
                    }

                    if ui
                        .add_enabled(has_repo && !repo_worker_busy, egui::Button::new("Cleanup..."))
                        .on_hover_text(
                            "Remove local branches whose remote branch was deleted\n(e.g. after a merged PR). Pull first to refresh.",
                        )
                        .clicked()
                    {
                        state.actions.push(UiAction::OpenCleanupBranches);
                        ui.close();
                    }

                    ui.separator();
                    ui.add_space(4.0);

                    if ui
                        .add_enabled(
                            can_discard && !repo_worker_busy,
                            egui::Button::new(
                                egui::RichText::new("Discard...")
                                    .color(egui::Color32::from_rgb(200, 80, 80)),
                            ),
                        )
                        .on_hover_text(discard_tooltip)
                        .clicked()
                    {
                        state.actions.push(UiAction::OpenDiscardDialog);
                        ui.close();
                    }

                    ui.add_space(4.0);
                    ui.separator();

                    if ui.button("Settings...").clicked() {
                        settings_clicked = true;
                        ui.close();
                    }

                    if has_logs && ui.button("View Logs").clicked() {
                        show_logs_clicked = true;
                        ui.close();
                    }
                });
                if !self.publish_dialog.show {
                    if let Some(busy) = &welcome_busy {
                        ui::show_inline_busy(ui, &busy.label);
                    }
                }

                ui.separator();

                let tabs_width = (ui.available_width() - controls_width).max(120.0);

                egui::ScrollArea::horizontal()
                    .id_salt("repo_tabs_scroll")
                    .max_width(tabs_width)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (index, (label, tooltip)) in tab_labels.iter().enumerate() {
                                let mut response = ui.selectable_label(index == self.active_tab, label);

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

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if has_origin_remote {
                        ui.add_enabled_ui(has_repo, |ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add_enabled(!repo_worker_busy, egui::Button::new(push_label))
                                        .on_hover_text(push_tooltip)
                                        .clicked()
                                    {
                                        state.actions.push(UiAction::Push);
                                    }
                                    if ui
                                        .add_enabled(!repo_worker_busy, egui::Button::new("Pull"))
                                        .on_hover_text("Pull from remote")
                                        .clicked()
                                    {
                                        state.actions.push(UiAction::Pull);
                                    }
                                    if let Some(busy) = repo_busy.as_ref().filter(|busy| {
                                        matches!(busy.action, BusyAction::Push | BusyAction::Pull)
                                    }) {
                                        ui::show_inline_busy(ui, &busy.label);
                                    }
                                },
                            );
                        });
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
                            .add_enabled(has_repo && !repo_worker_busy, egui::Button::new(label))
                            .on_hover_text(hover)
                            .clicked()
                        {
                            state.actions.push(UiAction::LaunchPullRequest);
                        }
                        if let Some(busy) = repo_busy.as_ref().filter(|busy| {
                            matches!(
                                busy.action,
                                BusyAction::OpenPullRequest | BusyAction::CreatePullRequest
                            )
                        }) {
                            ui::show_inline_busy(ui, &busy.label);
                        }
                    }

                    ui.add_enabled_ui(!repo_worker_busy, |ui| {
                        if ui
                            .add_enabled(has_repo, egui::Button::new("New Branch..."))
                            .on_hover_text("Create and switch to a new local branch")
                            .clicked()
                        {
                            state.show_create_branch_dialog = true;
                            state.focus_new_branch_name_requested = true;
                        }

                        if !state.branch.is_empty() {
                            let prev_branch = state.branch.clone();
                            egui::ComboBox::from_id_salt("branch_selector")
                                .selected_text(&state.branch)
                                .show_ui(ui, |ui| {
                                    for branch in &state.branches {
                                        ui.selectable_value(
                                            &mut state.branch,
                                            branch.clone(),
                                            branch,
                                        );
                                    }
                                });

                            if state.branch != prev_branch {
                                let new_branch = state.branch.clone();
                                state.actions.push(UiAction::SwitchBranch(new_branch));
                            }
                        }
                    });
                });

                if publish_clicked {
                    self.open_publish_repo_dialog(repo_path);
                }
                if github_sign_in_clicked {
                    self.begin_github_sign_in("Requesting GitHub sign-in code...");
                }
                if create_tag_clicked {
                    let tab = &mut self.tabs[active_index];
                    tab.state.new_tag_name = git_ops::suggest_next_tag(&tab.repo);
                    tab.state.show_create_tag_dialog = true;
                    tab.state.focus_new_tag_name_requested = true;
                }
                if show_logs_clicked {
                    self.show_log_viewer = true;
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
