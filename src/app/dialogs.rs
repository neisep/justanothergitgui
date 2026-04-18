use super::{helpers, *};

impl GitGuiApp {
    pub(super) fn any_dialog_open(&self) -> bool {
        if self.publish_dialog.show
            || self.settings_dialog.show
            || self.github_auth_prompt.is_some()
            || self.show_log_viewer
        {
            return true;
        }

        let Some(active_index) = self.active_tab_index() else {
            return false;
        };
        let state = &self.tabs[active_index].state;
        state.dialogs.branch.show_create_branch_dialog
            || state.dialogs.branch.show_create_branch_confirm
            || state.dialogs.tag.show_create_tag_dialog
            || state.dialogs.cleanup.show_cleanup_branches_dialog
            || state.dialogs.discard.show_discard_dialog
    }

    pub(super) fn close_topmost_dialog(&mut self) -> bool {
        if self.show_log_viewer {
            self.show_log_viewer = false;
            return true;
        }

        if self.github_auth_prompt.is_some() {
            self.github_auth_prompt = None;
            return true;
        }

        if let Some(active_index) = self.normalize_active_tab() {
            let state = &mut self.tabs[active_index].state;
            let create_tag_busy = state
                .ui
                .busy
                .as_ref()
                .is_some_and(|busy| busy.action == BusyAction::CreateTag);
            let discard_busy = state
                .ui
                .busy
                .as_ref()
                .is_some_and(|busy| busy.action == BusyAction::DiscardAndReset);

            if state.dialogs.cleanup.show_cleanup_branches_dialog {
                state.dialogs.cleanup.show_cleanup_branches_dialog = false;
                state.dialogs.cleanup.stale_branches.clear();
                return true;
            }

            if state.dialogs.discard.show_discard_dialog && !discard_busy {
                state.dialogs.discard.show_discard_dialog = false;
                state.dialogs.discard.discard_preview = None;
                state.dialogs.discard.discard_clean_untracked = false;
                return true;
            }

            if state.dialogs.tag.show_create_tag_dialog && !create_tag_busy {
                state.dialogs.tag.show_create_tag_dialog = false;
                state.dialogs.tag.new_tag_name.clear();
                state.dialogs.tag.focus_new_tag_name_requested = false;
                return true;
            }

            if state.dialogs.branch.show_create_branch_confirm {
                state.dialogs.branch.show_create_branch_confirm = false;
                state.dialogs.branch.create_branch_preview = None;
                state.dialogs.branch.pending_new_branch_name = None;
                state.dialogs.branch.new_branch_name.clear();
                state.dialogs.branch.focus_new_branch_name_requested = false;
                return true;
            }

            if state.dialogs.branch.show_create_branch_dialog {
                state.dialogs.branch.show_create_branch_dialog = false;
                state.dialogs.branch.new_branch_name.clear();
                state.dialogs.branch.focus_new_branch_name_requested = false;
                return true;
            }
        }

        if self.settings_dialog.show {
            self.settings_dialog.show = false;
            self.settings_dialog.focus_custom_scopes_requested = false;
            return true;
        }

        if self.publish_dialog.show && self.welcome_busy.is_none() {
            self.publish_dialog.show = false;
            self.publish_dialog.focus_folder_requested = false;
            return true;
        }

        false
    }

    pub(super) fn show_settings_dialog(&mut self, ctx: &egui::Context) {
        if !self.settings_dialog.show {
            return;
        }

        let output = ui::dialogs::settings::show(
            ctx,
            &mut self.settings_dialog,
            self.settings.commit_message_ruleset,
        );

        let parsed_custom_scopes =
            commit_rules::parse_custom_scopes(&self.settings_dialog.custom_scopes_input);
        if output.custom_scope_error.is_none()
            && (output.selected_ruleset != self.settings.commit_message_ruleset
                || parsed_custom_scopes.as_ref().ok()
                    != Some(&self.settings.commit_message_custom_scopes))
        {
            let mut next_settings = self.settings.clone();
            next_settings.commit_message_ruleset = output.selected_ruleset;
            next_settings.commit_message_custom_scopes = parsed_custom_scopes.unwrap_or_default();
            match settings::save_app_settings(&next_settings) {
                Ok(()) => {
                    self.settings = next_settings;
                    self.settings_dialog.status.clear();
                }
                Err(error) => {
                    self.logger.log_error("Settings", &error);
                    self.settings_dialog.status =
                        helpers::status_message_for_error("Settings", &error);
                    self.set_status_message(self.settings_dialog.status.clone());
                }
            }
        }

        if !output.keep_open {
            self.settings_dialog.focus_custom_scopes_requested = false;
        }

        self.settings_dialog.show = output.keep_open;
    }

    pub(super) fn show_clone_repo_dialog(&mut self, ctx: &egui::Context) {
        if !self.clone_dialog.show {
            return;
        }

        let welcome_busy = self.welcome_busy.clone();
        let worker_busy = welcome_busy.is_some();
        let worker_dispatch_busy = worker_busy || self.welcome_worker.is_busy();
        let signed_in = self.github_auth_session.is_some();
        let signed_in_login = self
            .github_auth_session
            .as_ref()
            .map(|session| session.login.clone())
            .unwrap_or_default();
        let output = ui::dialogs::clone_repo::show(
            ctx,
            &mut self.clone_dialog,
            worker_busy,
            worker_dispatch_busy,
            welcome_busy.as_ref().map(|busy| busy.label.as_str()),
            signed_in,
            &signed_in_login,
        );

        if output.choose_folder_clicked
            && let Some(path) = rfd::FileDialog::new().pick_folder()
        {
            self.clone_dialog.parent_folder = path.display().to_string();
        }

        if output.clone_clicked {
            self.start_clone_repo();
            return;
        }

        self.clone_dialog.show = if worker_busy { true } else { output.keep_open };
    }

    pub(super) fn start_clone_repo(&mut self) {
        if self.welcome_worker.is_busy() {
            self.clone_dialog.status = "Busy — please wait...".into();
            return;
        }
        let url = self.clone_dialog.url.trim().to_string();
        let parent = self.clone_dialog.parent_folder.trim().to_string();
        if url.is_empty() || parent.is_empty() {
            self.clone_dialog.status = "URL and destination folder are required.".into();
            return;
        }
        let Some(repo_name) = AppRepoRead::repo_name_from_clone_url(&url) else {
            self.clone_dialog.status = "Could not derive a repository name from this URL.".into();
            return;
        };
        let dest = PathBuf::from(&parent).join(&repo_name);
        let message = format!("Cloning into {}...", dest.display());
        if self
            .welcome_worker
            .clone_repo(url, dest, self.github_auth_session.clone())
        {
            self.clone_dialog.status = message;
            self.welcome_busy = Some(BusyState::new(BusyAction::CloneRepository, "Cloning..."));
        } else {
            let dispatch_message = helpers::status_message_for_worker_dispatch("Clone");
            self.clone_dialog.status = dispatch_message.clone();
            self.logger
                .log_error("Clone", helpers::WORKER_DISPATCH_ERROR_DETAIL);
            self.welcome_status = dispatch_message.clone();
            self.set_status_message(dispatch_message);
        }
    }

    pub(super) fn show_publish_repo_dialog(&mut self, ctx: &egui::Context) {
        if !self.publish_dialog.show {
            return;
        }

        let welcome_busy = self.welcome_busy.clone();
        let worker_busy = welcome_busy.is_some();
        let worker_dispatch_busy = worker_busy || self.welcome_worker.is_busy();
        let output = ui::dialogs::publish_repo::show(
            ctx,
            &mut self.publish_dialog,
            worker_busy,
            worker_dispatch_busy,
            welcome_busy.as_ref().map(|busy| busy.label.as_str()),
            self.settings.commit_message_ruleset,
            &self.settings.commit_message_custom_scopes,
        );

        if output.choose_folder_clicked {
            let mut dialog = rfd::FileDialog::new();
            if !self.publish_dialog.folder_path.trim().is_empty() {
                dialog = dialog.set_directory(self.publish_dialog.folder_path.trim());
            }

            if let Some(path) = dialog.pick_folder() {
                self.publish_dialog.set_folder(path);
            }
        }

        if output.sign_in_clicked {
            self.begin_github_sign_in("Requesting GitHub sign-in code...");
        }

        if output.create_clicked {
            if let Some(auth) = self.github_auth_session.clone() {
                let commit_message = self.publish_dialog.commit_message.trim().to_string();
                match commit_rules::validate_for_submit(
                    self.settings.commit_message_ruleset,
                    &commit_message,
                ) {
                    Ok(()) => {
                        let folder_path = PathBuf::from(self.publish_dialog.folder_path.trim());
                        if self
                            .welcome_worker
                            .create_github_repo(CreateGithubRepoRequest {
                                folder_path,
                                repo_name: self.publish_dialog.repo_name.trim().to_string(),
                                commit_message,
                                visibility: self.publish_dialog.visibility,
                                auth,
                            })
                        {
                            self.publish_dialog.operation_status =
                                "Publishing folder to GitHub...".into();
                            self.welcome_busy = Some(BusyState::new(
                                BusyAction::PublishRepository,
                                "Publishing repository...",
                            ));
                        } else {
                            let message =
                                helpers::status_message_for_worker_dispatch("Publish to GitHub");
                            self.publish_dialog.operation_status = message.clone();
                            self.logger.log_error(
                                "Publish to GitHub",
                                helpers::WORKER_DISPATCH_ERROR_DETAIL,
                            );
                            self.welcome_status = message.clone();
                            self.set_status_message(message);
                        }
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

        self.publish_dialog.show = if worker_busy { true } else { output.keep_open };
        if !self.publish_dialog.show {
            self.publish_dialog.focus_folder_requested = false;
        }
    }

    pub(super) fn show_create_branch_dialog(&mut self, ctx: &egui::Context) {
        let Some(active_index) = self.normalize_active_tab() else {
            return;
        };

        if !self.tabs[active_index]
            .state
            .dialogs
            .branch
            .show_create_branch_dialog
        {
            return;
        }

        let validation_error = AppRepoRead::validate_new_branch_name(
            &self.tabs[active_index].repo,
            &self.tabs[active_index].state.dialogs.branch.new_branch_name,
        );

        let state = &mut self.tabs[active_index].state;

        let output =
            ui::dialogs::branch::show_create_dialog(ctx, state, validation_error.as_deref());

        if let Some(branch_name) = output.submit_branch {
            state
                .ui
                .actions
                .push(UiAction::open_create_branch_confirm(branch_name));
        }

        if !output.keep_open && state.dialogs.branch.show_create_branch_dialog {
            state.dialogs.branch.new_branch_name.clear();
            state.dialogs.branch.focus_new_branch_name_requested = false;
        }
        state.dialogs.branch.show_create_branch_dialog = output.keep_open;
    }

    pub(super) fn show_create_branch_confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(active_index) = self.normalize_active_tab() else {
            return;
        };
        let state = &mut self.tabs[active_index].state;

        if !state.dialogs.branch.show_create_branch_confirm {
            return;
        }

        let output = ui::dialogs::branch::show_confirm_dialog(ctx, state);

        if output.confirm_requested {
            state.ui.actions.push(UiAction::confirm_create_branch());
        }

        if !output.keep_open && state.dialogs.branch.show_create_branch_confirm {
            state.dialogs.branch.create_branch_preview = None;
            state.dialogs.branch.pending_new_branch_name = None;
            state.dialogs.branch.new_branch_name.clear();
        }
        state.dialogs.branch.show_create_branch_confirm = output.keep_open;
    }

    pub(super) fn show_create_tag_dialog(&mut self, ctx: &egui::Context) {
        let Some(active_index) = self.normalize_active_tab() else {
            return;
        };
        let state = &mut self.tabs[active_index].state;

        if !state.dialogs.tag.show_create_tag_dialog {
            return;
        }

        let create_tag_busy = state
            .ui
            .busy
            .as_ref()
            .is_some_and(|busy| busy.action == BusyAction::CreateTag);
        let can_create_tag = AppRepoRead::can_create_tag_on_branch(&state.repo.branch)
            && (!state.repo.has_github_https_origin || self.github_auth_session.is_some());
        let create_tag_busy_label = state
            .ui
            .busy
            .as_ref()
            .filter(|busy| busy.action == BusyAction::CreateTag)
            .map(|busy| busy.label.clone());
        let output = ui::dialogs::tag::show(
            ctx,
            state,
            can_create_tag,
            self.github_auth_session.is_some(),
            create_tag_busy,
            create_tag_busy_label.as_deref(),
        );

        if let Some(tag_name) = output.submit_tag {
            state.ui.actions.push(UiAction::create_tag(tag_name));
        }

        state.dialogs.tag.show_create_tag_dialog = if create_tag_busy {
            true
        } else {
            output.keep_open
        };
        if !state.dialogs.tag.show_create_tag_dialog {
            state.dialogs.tag.new_tag_name.clear();
            state.dialogs.tag.focus_new_tag_name_requested = false;
        }
    }

    pub(super) fn show_cleanup_branches_dialog(&mut self, ctx: &egui::Context) {
        let Some(active_index) = self.normalize_active_tab() else {
            return;
        };
        let state = &mut self.tabs[active_index].state;

        if !state.dialogs.cleanup.show_cleanup_branches_dialog {
            return;
        }

        let output = ui::dialogs::cleanup_branches::show(ctx, state);

        if output.delete_requested {
            let names: Vec<String> = state
                .dialogs
                .cleanup
                .stale_branches
                .iter()
                .filter(|branch| branch.selected)
                .map(|branch| branch.name.clone())
                .collect();
            if !names.is_empty() {
                state
                    .ui
                    .actions
                    .push(UiAction::delete_stale_branches(names));
            }
        }

        state.dialogs.cleanup.show_cleanup_branches_dialog = output.keep_open;
        if !state.dialogs.cleanup.show_cleanup_branches_dialog {
            state.dialogs.cleanup.stale_branches.clear();
        }
    }

    pub(super) fn show_discard_dialog(&mut self, ctx: &egui::Context) {
        let Some(active_index) = self.normalize_active_tab() else {
            return;
        };
        let state = &mut self.tabs[active_index].state;

        if !state.dialogs.discard.show_discard_dialog {
            return;
        }

        let discard_busy = state
            .ui
            .busy
            .as_ref()
            .is_some_and(|busy| busy.action == BusyAction::DiscardAndReset);
        let discard_busy_label = state
            .ui
            .busy
            .as_ref()
            .filter(|busy| busy.action == BusyAction::DiscardAndReset)
            .map(|busy| busy.label.clone());
        let output =
            ui::dialogs::discard::show(ctx, state, discard_busy, discard_busy_label.as_deref());

        if output.confirm_requested {
            let clean_untracked = state.dialogs.discard.discard_clean_untracked;
            state
                .ui
                .actions
                .push(UiAction::discard_and_reset(clean_untracked));
        }

        state.dialogs.discard.show_discard_dialog =
            if discard_busy { true } else { output.keep_open };
        if !state.dialogs.discard.show_discard_dialog {
            state.dialogs.discard.discard_preview = None;
            state.dialogs.discard.discard_clean_untracked = false;
        }
    }

    pub(super) fn show_github_auth_dialog(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.github_auth_prompt.clone() else {
            return;
        };

        let output = ui::dialogs::github_auth::show(ctx, &prompt);
        if output.open_github_again_clicked {
            if let Err(error) = webbrowser::open(&prompt.browser_url) {
                let detail = error.to_string();
                self.logger.log_error("GitHub sign-in", &detail);
                self.set_status_message(helpers::status_message_for_error(
                    "GitHub sign-in",
                    &detail,
                ));
            }
        }
    }
}
