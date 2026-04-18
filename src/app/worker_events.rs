use super::{helpers, *};

impl GitGuiApp {
    pub(super) fn poll_workers(&mut self) -> bool {
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
                    self.welcome_busy = None;
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
                    self.welcome_busy = None;
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
                            helpers::status_message_for_error("GitHub sign-in", &msg)
                        };
                    self.publish_dialog.operation_status.clear();
                    self.welcome_status = helpers::status_message_for_error("GitHub sign-in", &msg);
                    self.set_status_message(self.publish_dialog.github_status.clone());
                }
                TaskResult::CreateGithubRepo(Ok(result)) => {
                    self.welcome_busy = None;
                    let message = result.message.clone();
                    self.publish_dialog.show = false;
                    self.publish_dialog.operation_status.clear();
                    self.welcome_status = message.clone();
                    self.open_repo(result.folder_path);
                    self.set_status_message(message);
                }
                TaskResult::CreateGithubRepo(Err(msg)) => {
                    self.welcome_busy = None;
                    self.logger.log_error("Publish to GitHub", &msg);
                    self.publish_dialog.operation_status =
                        helpers::status_message_for_error("Publish to GitHub", &msg);
                    self.welcome_status =
                        helpers::status_message_for_error("Publish to GitHub", &msg);
                }
                TaskResult::ListGithubRepos(Ok(list)) => {
                    self.clone_dialog.github_repos = list;
                    self.clone_dialog.github_repos_loading = false;
                    self.clone_dialog.github_repos_error = None;
                }
                TaskResult::ListGithubRepos(Err(msg)) => {
                    self.clone_dialog.github_repos_loading = false;
                    self.clone_dialog.github_repos_error = Some(msg.clone());
                    self.logger.log_error("GitHub repos", &msg);
                }
                TaskResult::CloneRepo(Ok(path)) => {
                    self.welcome_busy = None;
                    self.clone_dialog.show = false;
                    self.clone_dialog.status.clear();
                    let message = format!("Cloned repository to {}", path.display());
                    self.welcome_status = message.clone();
                    self.open_repo(path);
                    self.set_status_message(message);
                }
                TaskResult::CloneRepo(Err(msg)) => {
                    self.welcome_busy = None;
                    self.logger.log_error("Clone", &msg);
                    self.clone_dialog.status = helpers::status_message_for_error("Clone", &msg);
                    self.welcome_status = helpers::status_message_for_error("Clone", &msg);
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
                        tab.state.busy = None;
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
                        tab.state.busy = None;
                        tab.state.status_msg = helpers::status_message_for_error("Push", &msg);
                        tab_logs.push(("Push".into(), msg));
                    }
                    TaskResult::Pull(Ok(msg)) => {
                        tab.state.busy = None;
                        tab.state.status_msg = format!("Pull: {}", msg);
                        refresh_indices.push(index);
                    }
                    TaskResult::Pull(Err(msg)) => {
                        tab.state.busy = None;
                        tab.state.status_msg = helpers::status_message_for_error("Pull", &msg);
                        tab_logs.push(("Pull".into(), msg));
                    }
                    TaskResult::CreateTag(Ok(msg)) => {
                        tab.state.busy = None;
                        tab.state.status_msg = msg;
                        tab.state.new_tag_name.clear();
                        tab.state.focus_new_tag_name_requested = false;
                        tab.state.show_create_tag_dialog = false;
                        refresh_indices.push(index);
                    }
                    TaskResult::CreateTag(Err(msg)) => {
                        tab.state.busy = None;
                        tab.state.status_msg =
                            helpers::status_message_for_error("Create tag", &msg);
                        tab_logs.push(("Create tag".into(), msg));
                        refresh_indices.push(index);
                    }
                    TaskResult::OpenPullRequest(Ok(msg)) => {
                        tab.state.busy = None;
                        tab.state.status_msg = msg;
                    }
                    TaskResult::OpenPullRequest(Err(msg)) => {
                        tab.state.busy = None;
                        tab.state.status_msg = helpers::status_message_for_error("Open PR", &msg);
                        tab_logs.push(("Open PR".into(), msg));
                    }
                    TaskResult::CreatePullRequest(Ok(msg)) => {
                        tab.state.busy = None;
                        tab.state.status_msg = msg;
                    }
                    TaskResult::CreatePullRequest(Err(msg)) => {
                        tab.state.busy = None;
                        tab.state.status_msg = helpers::status_message_for_error("Create PR", &msg);
                        tab_logs.push(("Create PR".into(), msg));
                    }
                    TaskResult::DiscardAndReset(Ok(msg)) => {
                        tab.state.busy = None;
                        tab.state.status_msg = format!("Discard: {}", msg);
                        tab.state.show_discard_dialog = false;
                        tab.state.discard_preview = None;
                        tab.state.discard_clean_untracked = false;
                        refresh_indices.push(index);
                    }
                    TaskResult::DiscardAndReset(Err(msg)) => {
                        tab.state.busy = None;
                        tab.state.status_msg =
                            helpers::status_message_for_error("Discard & reset", &msg);
                        tab_logs.push(("Discard & reset".into(), msg));
                        refresh_indices.push(index);
                    }
                    TaskResult::GithubAuthPrompt(_)
                    | TaskResult::GithubAuth(_)
                    | TaskResult::CreateGithubRepo(_)
                    | TaskResult::ListGithubRepos(_)
                    | TaskResult::CloneRepo(_) => {}
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
                    if let Some(detail) =
                        helpers::refresh_status(&mut self.tabs[index].state, &repo)
                    {
                        tab_logs.push(("Refresh".into(), detail));
                    }
                    self.tabs[index].repo = repo;
                }
                Err(error) => {
                    let detail = error.to_string();
                    self.tabs[index].state.status_msg =
                        helpers::status_message_for_error("Refresh", &detail);
                    tab_logs.push(("Refresh".into(), detail));
                }
            }
        }

        for (context, detail) in tab_logs {
            self.logger.log_error(&context, &detail);
        }

        any_busy
    }
}
