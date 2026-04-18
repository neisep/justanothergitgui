use super::{helpers, *};

impl GitGuiApp {
    pub(super) fn open_repo_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.open_repo(path);
        }
    }

    pub(super) fn open_repo(&mut self, path: PathBuf) {
        match git_ops::open_repo(&path) {
            Ok(repo) => self.add_repo_tab(repo),
            Err(error) => {
                let detail = error.to_string();
                self.logger.log_error("Open repository", &detail);
                self.set_status_message(helpers::status_message_for_error(
                    "Open repository",
                    &detail,
                ));
            }
        }
    }

    pub(super) fn add_repo_tab(&mut self, repo: Repository) {
        let repo_path = helpers::repo_root_path(&repo);

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
        helpers::reset_repo_view_state(&mut state);
        let refresh_error = helpers::refresh_status(&mut state, &repo);
        if let Some(detail) = refresh_error {
            self.logger.log_error("Refresh", &detail);
        } else {
            state.status_msg = format!(
                "Repository loaded: {}",
                helpers::repo_tab_label(Some(&repo_path))
            );
        }

        self.tabs.push(RepoTab {
            state,
            repo,
            worker: Worker::new(),
        });
        self.active_tab = self.tabs.len() - 1;
        self.welcome_status = "Open a Git repository to get started.".into();
    }

    pub(super) fn set_status_message(&mut self, message: String) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.state.status_msg = message;
        } else {
            self.welcome_status = message;
        }
    }

    pub(super) fn open_publish_repo_dialog(&mut self, path: Option<PathBuf>) {
        self.publish_dialog
            .reset_for_path(path, self.settings.commit_message_ruleset);
        self.publish_dialog.show = true;
        self.publish_dialog.focus_folder_requested = true;
        self.refresh_github_auth_status();
    }

    pub(super) fn open_clone_repo_dialog(&mut self) {
        self.clone_dialog.reset();
        self.clone_dialog.show = true;
        self.clone_dialog.focus_url_requested = true;

        if let Some(session) = self.github_auth_session.clone()
            && !self.welcome_worker.is_busy()
        {
            self.clone_dialog.github_repos_loading = true;
            self.welcome_worker.list_github_repos(session);
        }
    }

    pub(super) fn open_settings_dialog(&mut self) {
        self.settings_dialog.show = true;
        self.settings_dialog.focus_custom_scopes_requested = true;
        self.settings_dialog.custom_scopes_input =
            self.settings.commit_message_custom_scopes.join(", ");
    }

    pub(super) fn refresh_github_auth_status(&mut self) {
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

    pub(super) fn begin_github_sign_in(&mut self, start_message: &str) {
        if self.welcome_worker.is_busy() {
            self.welcome_status = "Busy — please wait...".into();
            self.set_status_message("Busy — please wait...".into());
            return;
        }

        self.github_auth_prompt = None;
        self.publish_dialog.github_status = start_message.into();
        self.publish_dialog.operation_status.clear();
        self.welcome_status = start_message.into();
        self.welcome_busy = Some(BusyState::new(BusyAction::GithubSignIn, start_message));
        self.set_status_message(start_message.into());
        self.welcome_worker
            .login_github(GITHUB_OAUTH_CLIENT_ID.into());
    }
}
