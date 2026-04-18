use super::{helpers, *};
use crate::state::{CenterView, SelectedFile};

impl GitGuiApp {
    pub(super) fn process_actions(&mut self) {
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
                        Err(error) => {
                            let detail = error.to_string();
                            tab.state.status_msg =
                                helpers::status_message_for_error("Stage", &detail);
                            log_entry = Some(("Stage", detail));
                        }
                    }
                    refresh_error = helpers::refresh_status(&mut tab.state, &tab.repo);
                }

                UiAction::UnstageFile(path) => {
                    let tab = &mut self.tabs[active_index];
                    match git_ops::unstage_file(&tab.repo, &path) {
                        Ok(()) => tab.state.status_msg = format!("Unstaged: {}", path),
                        Err(error) => {
                            let detail = error.to_string();
                            tab.state.status_msg =
                                helpers::status_message_for_error("Unstage", &detail);
                            log_entry = Some(("Unstage", detail));
                        }
                    }
                    refresh_error = helpers::refresh_status(&mut tab.state, &tab.repo);
                }

                UiAction::StageAll => {
                    let tab = &mut self.tabs[active_index];
                    match git_ops::stage_all(&tab.repo) {
                        Ok(()) => tab.state.status_msg = "Staged all changes".into(),
                        Err(error) => {
                            let detail = error.to_string();
                            tab.state.status_msg =
                                helpers::status_message_for_error("Stage all", &detail);
                            log_entry = Some(("Stage all", detail));
                        }
                    }
                    refresh_error = helpers::refresh_status(&mut tab.state, &tab.repo);
                }

                UiAction::UnstageAll => {
                    let tab = &mut self.tabs[active_index];
                    match git_ops::unstage_all(&tab.repo) {
                        Ok(()) => tab.state.status_msg = "Unstaged all changes".into(),
                        Err(error) => {
                            let detail = error.to_string();
                            tab.state.status_msg =
                                helpers::status_message_for_error("Unstage all", &detail);
                            log_entry = Some(("Unstage all", detail));
                        }
                    }
                    refresh_error = helpers::refresh_status(&mut tab.state, &tab.repo);
                }

                UiAction::Commit => {
                    let tab = &mut self.tabs[active_index];
                    let msg = commit_rules::build_message(
                        &tab.state.commit_summary,
                        &tab.state.commit_body,
                    );
                    match commit_rules::validate_for_submit(
                        self.settings.commit_message_ruleset,
                        &msg,
                    ) {
                        Ok(()) => match git_ops::create_commit(&tab.repo, &msg) {
                            Ok(oid) => {
                                tab.state.status_msg =
                                    format!("Committed: {}", &oid.to_string()[..8]);
                                tab.state.commit_summary.clear();
                                tab.state.commit_body.clear();
                                tab.state.selected_file = None;
                                tab.state.diff_content.clear();
                                tab.state.conflict_data = None;
                            }
                            Err(error) => {
                                let detail = error.to_string();
                                tab.state.status_msg =
                                    helpers::status_message_for_error("Commit", &detail);
                                log_entry = Some(("Commit", detail));
                            }
                        },
                        Err(detail) => {
                            tab.state.status_msg = detail;
                        }
                    }

                    if tab.state.status_msg.starts_with("Committed:") || log_entry.is_some() {
                        refresh_error = helpers::refresh_status(&mut tab.state, &tab.repo);
                    }
                }

                UiAction::Push => {
                    let tab = &mut self.tabs[active_index];
                    if let Some(path) = tab.state.repo_path.clone() {
                        if tab.worker.is_busy() {
                            tab.state.status_msg = "Busy — please wait...".into();
                        } else {
                            let busy = BusyState::new(BusyAction::Push, "Pushing...");
                            tab.state.status_msg = busy.label.clone();
                            tab.state.busy = Some(busy);
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
                            let busy = BusyState::new(BusyAction::Pull, "Pulling...");
                            tab.state.status_msg = busy.label.clone();
                            tab.state.busy = Some(busy);
                            tab.worker.pull(path, self.github_auth_session.clone());
                        }
                    }
                }

                UiAction::SelectFile { path, staged } => {
                    let tab = &mut self.tabs[active_index];
                    tab.state.center_view = CenterView::Diff;
                    helpers::load_selected_file(&mut tab.state, &tab.repo, path, staged);
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
                        Err(error) => {
                            let detail = error.to_string();
                            tab.state.status_msg =
                                helpers::status_message_for_error("Switch branch", &detail);
                            log_entry = Some(("Switch branch", detail));
                        }
                    }
                    refresh_error = helpers::refresh_status(&mut tab.state, &tab.repo);
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
                        Err(error) => {
                            let detail = error.to_string();
                            tab.state.status_msg =
                                helpers::status_message_for_error("Create branch", &detail);
                            log_entry = Some(("Create branch", detail));
                        }
                    }
                    refresh_error = helpers::refresh_status(&mut tab.state, &tab.repo);
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
                            let busy = BusyState::new(
                                BusyAction::CreateTag,
                                format!("Creating tag {}...", tag_name),
                            );
                            tab.state.status_msg = busy.label.clone();
                            tab.state.busy = Some(busy);
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
                            let busy = BusyState::new(
                                BusyAction::OpenPullRequest,
                                format!("Opening pull request #{}...", number),
                            );
                            tab.state.status_msg = busy.label.clone();
                            tab.state.busy = Some(busy);
                            tab.worker.open_pull_request(url);
                        }
                        PullRequestPrompt::Create { branch, url } => {
                            let busy = BusyState::new(
                                BusyAction::CreatePullRequest,
                                format!("Opening pull request creation for {}...", branch),
                            );
                            tab.state.status_msg = busy.label.clone();
                            tab.state.busy = Some(busy);
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
                        Err(error) => {
                            let detail = error.to_string();
                            tab.state.status_msg =
                                helpers::status_message_for_error("Cleanup branches", &detail);
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
                            Err(error) => failures.push(format!("{}: {}", name, error)),
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
                        tab.state.status_msg =
                            helpers::status_message_for_error("Delete branch", &detail);
                        log_entry = Some(("Delete branch", detail));
                    }
                    refresh_error = helpers::refresh_status(&mut tab.state, &tab.repo);
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
                            let busy = BusyState::new(
                                BusyAction::DiscardAndReset,
                                "Resetting to remote...",
                            );
                            tab.state.status_msg = busy.label.clone();
                            tab.state.busy = Some(busy);
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
                            Err(error) => {
                                let detail = error.to_string();
                                tab.state.status_msg =
                                    helpers::status_message_for_error("Save resolution", &detail);
                                log_entry = Some(("Save resolution", detail));
                            }
                        }
                    } else {
                        tab.state.status_msg = "No conflict selected".into();
                    }
                    refresh_error = helpers::refresh_status(&mut tab.state, &tab.repo);
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
}
