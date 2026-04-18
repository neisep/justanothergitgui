use super::{helpers, *};
use crate::state::{CenterView, InspectorState, SelectedFile};

struct TabActionContext<'a> {
    tab: &'a mut RepoTab,
    settings: &'a AppSettings,
    github_auth_session: &'a Option<GithubAuthSession>,
    logger: &'a mut AppLogger,
}

impl UiAction {
    fn apply(self, ctx: &mut TabActionContext<'_>) {
        match self {
            Self::StageFile(path) => stage_file(ctx, path),
            Self::UnstageFile(path) => unstage_file(ctx, path),
            Self::StageAll => stage_all(ctx),
            Self::UnstageAll => unstage_all(ctx),
            Self::Commit => commit(ctx),
            Self::Push => push(ctx),
            Self::Pull => pull(ctx),
            Self::SelectFile { path, staged } => select_file(ctx, path, staged),
            Self::SwitchBranch(branch) => switch_branch(ctx, branch),
            Self::CreateBranch(branch) => create_branch(ctx, branch),
            Self::OpenCreateBranchConfirm(branch) => open_create_branch_confirm(ctx, branch),
            Self::ConfirmCreateBranch => confirm_create_branch(ctx),
            Self::CreateTag(tag_name) => create_tag(ctx, tag_name),
            Self::LaunchPullRequest => launch_pull_request(ctx),
            Self::ShowDiff => show_diff(ctx),
            Self::ShowHistory => show_history(ctx),
            Self::OpenCleanupBranches => open_cleanup_branches(ctx),
            Self::DeleteStaleBranches(names) => delete_stale_branches(ctx, names),
            Self::OpenDiscardDialog => open_discard_dialog(ctx),
            Self::DiscardAndReset { clean_untracked } => discard_and_reset(ctx, clean_untracked),
            Self::UndoLastCommit => undo_last_commit(ctx),
            Self::SaveConflictResolution => save_conflict_resolution(ctx),
        }
    }
}

fn stage_file(ctx: &mut TabActionContext<'_>, path: String) {
    match AppRepoWrite::stage_file(&ctx.tab.repo, &path) {
        Ok(()) => ctx.tab.state.ui.status_msg = format!("Staged: {path}"),
        Err(error) => log_action_error(ctx, "Stage", error.to_string()),
    }
    refresh_tab(ctx);
}

fn unstage_file(ctx: &mut TabActionContext<'_>, path: String) {
    match AppRepoWrite::unstage_file(&ctx.tab.repo, &path) {
        Ok(()) => ctx.tab.state.ui.status_msg = format!("Unstaged: {path}"),
        Err(error) => log_action_error(ctx, "Unstage", error.to_string()),
    }
    refresh_tab(ctx);
}

fn stage_all(ctx: &mut TabActionContext<'_>) {
    match AppRepoWrite::stage_all(&ctx.tab.repo) {
        Ok(()) => ctx.tab.state.ui.status_msg = "Staged all changes".into(),
        Err(error) => log_action_error(ctx, "Stage all", error.to_string()),
    }
    refresh_tab(ctx);
}

fn unstage_all(ctx: &mut TabActionContext<'_>) {
    match AppRepoWrite::unstage_all(&ctx.tab.repo) {
        Ok(()) => ctx.tab.state.ui.status_msg = "Unstaged all changes".into(),
        Err(error) => log_action_error(ctx, "Unstage all", error.to_string()),
    }
    refresh_tab(ctx);
}

fn commit(ctx: &mut TabActionContext<'_>) {
    let ruleset = ctx.settings.commit_message_ruleset;
    let mut should_refresh = false;
    let tab = &mut ctx.tab;
    let msg = commit_rules::build_message(
        &tab.state.commit.commit_summary,
        &tab.state.commit.commit_body,
    );

    match commit_rules::validate_for_submit(ruleset, &msg) {
        Ok(()) => {
            should_refresh = true;
            match AppRepoWrite::create_commit(&tab.repo, &msg) {
                Ok(oid) => {
                    tab.state.ui.status_msg = format!("Committed: {}", &oid.to_string()[..8]);
                    clear_repo_selection(&mut tab.state.inspector);
                    tab.state.commit.commit_summary.clear();
                    tab.state.commit.commit_body.clear();
                }
                Err(error) => log_action_error(ctx, "Commit", error.to_string()),
            }
        }
        Err(detail) => {
            tab.state.ui.status_msg = detail;
        }
    }

    if should_refresh {
        refresh_tab(ctx);
    }
}

fn push(ctx: &mut TabActionContext<'_>) {
    if let Some(path) = ctx.tab.state.repo.path.clone() {
        if ctx.tab.worker.is_busy() {
            ctx.tab.state.ui.status_msg = "Busy — please wait...".into();
        } else {
            let busy = BusyState::new(BusyAction::Push, "Pushing...");
            if ctx.tab.worker.push(path, ctx.github_auth_session.clone()) {
                ctx.tab.state.ui.status_msg = busy.label.clone();
                ctx.tab.state.ui.busy = Some(busy);
            } else {
                log_worker_dispatch_error(ctx, "Push");
            }
        }
    }
}

fn pull(ctx: &mut TabActionContext<'_>) {
    if let Some(path) = ctx.tab.state.repo.path.clone() {
        if ctx.tab.worker.is_busy() {
            ctx.tab.state.ui.status_msg = "Busy — please wait...".into();
        } else {
            let busy = BusyState::new(BusyAction::Pull, "Pulling...");
            if ctx.tab.worker.pull(path, ctx.github_auth_session.clone()) {
                ctx.tab.state.ui.status_msg = busy.label.clone();
                ctx.tab.state.ui.busy = Some(busy);
            } else {
                log_worker_dispatch_error(ctx, "Pull");
            }
        }
    }
}

fn select_file(ctx: &mut TabActionContext<'_>, path: String, staged: bool) {
    let state = &mut ctx.tab.state;
    state.inspector.center_view = CenterView::Diff;
    helpers::load_selected_file(
        &state.worktree,
        &mut state.inspector,
        &ctx.tab.repo,
        path,
        staged,
    );
}

fn switch_branch(ctx: &mut TabActionContext<'_>, branch: String) {
    match AppRepoWrite::switch_branch(&ctx.tab.repo, &branch) {
        Ok(()) => {
            ctx.tab.state.ui.status_msg = format!("Switched to {branch}");
            clear_repo_selection(&mut ctx.tab.state.inspector);
        }
        Err(error) => log_action_error(ctx, "Switch branch", error.to_string()),
    }
    refresh_tab(ctx);
}

fn create_branch(ctx: &mut TabActionContext<'_>, branch: String) {
    match AppRepoWrite::create_branch(&ctx.tab.repo, &branch) {
        Ok(()) => {
            ctx.tab.state.ui.status_msg = format!("Created and switched to {branch}");
            clear_repo_selection(&mut ctx.tab.state.inspector);
            ctx.tab.state.dialogs.branch.new_branch_name.clear();
            ctx.tab.state.dialogs.branch.show_create_branch_dialog = false;
            ctx.tab.state.dialogs.branch.show_create_branch_confirm = false;
            ctx.tab.state.dialogs.branch.create_branch_preview = None;
            ctx.tab.state.dialogs.branch.pending_new_branch_name = None;
        }
        Err(error) => log_action_error(ctx, "Create branch", error.to_string()),
    }
    refresh_tab(ctx);
}

fn open_create_branch_confirm(ctx: &mut TabActionContext<'_>, branch: String) {
    let preview = AppRepoWrite::preview_create_branch(&ctx.tab.repo, &branch);
    let clean =
        preview.dirty_files == 0 && preview.untracked_files == 0 && preview.staged_files == 0;

    if clean {
        ctx.tab
            .state
            .ui
            .actions
            .push(UiAction::create_branch(branch));
    } else {
        ctx.tab.state.dialogs.branch.pending_new_branch_name = Some(branch);
        ctx.tab.state.dialogs.branch.create_branch_preview = Some(preview);
        ctx.tab.state.dialogs.branch.show_create_branch_dialog = false;
        ctx.tab.state.dialogs.branch.show_create_branch_confirm = true;
    }
}

fn confirm_create_branch(ctx: &mut TabActionContext<'_>) {
    if let Some(name) = ctx.tab.state.dialogs.branch.pending_new_branch_name.take() {
        ctx.tab.state.ui.actions.push(UiAction::create_branch(name));
    }
    ctx.tab.state.dialogs.branch.show_create_branch_confirm = false;
    ctx.tab.state.dialogs.branch.create_branch_preview = None;
}

fn create_tag(ctx: &mut TabActionContext<'_>, tag_name: String) {
    if let Some(path) = ctx.tab.state.repo.path.clone() {
        if ctx.tab.worker.is_busy() {
            ctx.tab.state.ui.status_msg = "Busy — please wait...".into();
        } else if !AppRepoRead::can_create_tag_on_branch(&ctx.tab.state.repo.branch) {
            ctx.tab.state.ui.status_msg =
                "Tags can only be created from the main or master branch.".into();
        } else if ctx.tab.state.repo.has_github_https_origin && ctx.github_auth_session.is_none() {
            ctx.tab.state.ui.status_msg =
                "Sign in to GitHub before creating tags for this repository.".into();
        } else {
            let busy = BusyState::new(BusyAction::CreateTag, format!("Creating tag {tag_name}..."));
            if ctx
                .tab
                .worker
                .create_tag(path, tag_name, ctx.github_auth_session.clone())
            {
                ctx.tab.state.ui.status_msg = busy.label.clone();
                ctx.tab.state.ui.busy = Some(busy);
            } else {
                log_worker_dispatch_error(ctx, "Create tag");
            }
        }
    }
}

fn launch_pull_request(ctx: &mut TabActionContext<'_>) {
    let Some(prompt) = ctx.tab.state.repo.pull_request_prompt.clone() else {
        ctx.tab.state.ui.status_msg = "No pull request action available".into();
        return;
    };

    if ctx.tab.worker.is_busy() {
        ctx.tab.state.ui.status_msg = "Busy — please wait...".into();
        return;
    }

    match prompt {
        PullRequestPrompt::Open { number, url, .. } => {
            let busy = BusyState::new(
                BusyAction::OpenPullRequest,
                format!("Opening pull request #{number}..."),
            );
            if ctx.tab.worker.open_pull_request(url) {
                ctx.tab.state.ui.status_msg = busy.label.clone();
                ctx.tab.state.ui.busy = Some(busy);
            } else {
                log_worker_dispatch_error(ctx, "Open PR");
            }
        }
        PullRequestPrompt::Create { branch, url } => {
            let busy = BusyState::new(
                BusyAction::CreatePullRequest,
                format!("Opening pull request creation for {branch}..."),
            );
            if ctx.tab.worker.create_pull_request(url) {
                ctx.tab.state.ui.status_msg = busy.label.clone();
                ctx.tab.state.ui.busy = Some(busy);
            } else {
                log_worker_dispatch_error(ctx, "Create PR");
            }
        }
    }
}

fn show_diff(ctx: &mut TabActionContext<'_>) {
    ctx.tab.state.inspector.center_view = CenterView::Diff;
}

fn show_history(ctx: &mut TabActionContext<'_>) {
    ctx.tab.state.inspector.center_view = CenterView::History;
}

fn open_cleanup_branches(ctx: &mut TabActionContext<'_>) {
    match AppRepoWrite::list_stale_branches(&ctx.tab.repo) {
        Ok(stale) => {
            ctx.tab.state.dialogs.cleanup.stale_branches = stale;
            ctx.tab.state.dialogs.cleanup.show_cleanup_branches_dialog = true;
        }
        Err(error) => log_action_error(ctx, "Cleanup branches", error.to_string()),
    }
}

fn delete_stale_branches(ctx: &mut TabActionContext<'_>, names: Vec<String>) {
    let mut deleted = Vec::new();
    let mut failures = Vec::new();

    for name in &names {
        match AppRepoWrite::delete_local_branch(&ctx.tab.repo, name) {
            Ok(()) => deleted.push(name.clone()),
            Err(error) => failures.push(format!("{name}: {error}")),
        }
    }

    ctx.tab
        .state
        .dialogs
        .cleanup
        .stale_branches
        .retain(|branch| !deleted.contains(&branch.name));

    if failures.is_empty() {
        ctx.tab.state.ui.status_msg = format!("Deleted {} branch(es)", deleted.len());
        ctx.tab.state.dialogs.cleanup.show_cleanup_branches_dialog = false;
    } else {
        log_action_error(ctx, "Delete branch", failures.join("; "));
    }

    refresh_tab(ctx);
}

fn open_discard_dialog(ctx: &mut TabActionContext<'_>) {
    let preview = AppRepoWrite::preview_discard_damage(&ctx.tab.repo);
    ctx.tab.state.dialogs.discard.discard_preview = Some(preview);
    ctx.tab.state.dialogs.discard.discard_clean_untracked = false;
    ctx.tab.state.dialogs.discard.show_discard_dialog = true;
}

fn discard_and_reset(ctx: &mut TabActionContext<'_>, clean_untracked: bool) {
    if let Some(path) = ctx.tab.state.repo.path.clone() {
        if ctx.tab.worker.is_busy() {
            ctx.tab.state.ui.status_msg = "Busy — please wait...".into();
        } else {
            let busy = BusyState::new(BusyAction::DiscardAndReset, "Resetting to remote...");
            if ctx.tab.worker.discard_and_reset(
                path,
                ctx.github_auth_session.clone(),
                clean_untracked,
            ) {
                ctx.tab.state.ui.status_msg = busy.label.clone();
                ctx.tab.state.ui.busy = Some(busy);
            } else {
                log_worker_dispatch_error(ctx, "Discard & reset");
            }
        }
    }
}

fn undo_last_commit(ctx: &mut TabActionContext<'_>) {
    if let Some(path) = ctx.tab.state.repo.path.clone() {
        if ctx.tab.worker.is_busy() {
            ctx.tab.state.ui.status_msg = "Busy — please wait...".into();
        } else {
            let busy = BusyState::new(BusyAction::UndoLastCommit, "Undoing last commit...");
            if ctx.tab.worker.undo_last_commit(path) {
                ctx.tab.state.ui.status_msg = busy.label.clone();
                ctx.tab.state.ui.busy = Some(busy);
            } else {
                log_worker_dispatch_error(ctx, "Undo last commit");
            }
        }
    }
}

fn save_conflict_resolution(ctx: &mut TabActionContext<'_>) {
    if let Some(conflict_data) = ctx.tab.state.inspector.conflict_data.clone() {
        let path = conflict_data.path.clone();
        match AppRepoWrite::write_resolved_file(&ctx.tab.repo, &conflict_data) {
            Ok(()) => {
                ctx.tab.state.ui.status_msg = format!("Resolved and staged: {path}");
                ctx.tab.state.inspector.selected_file = Some(SelectedFile { path, staged: true });
                ctx.tab.state.inspector.conflict_data = None;
            }
            Err(error) => log_action_error(ctx, "Save resolution", error.to_string()),
        }
    } else {
        ctx.tab.state.ui.status_msg = "No conflict selected".into();
    }

    refresh_tab(ctx);
}

impl GitGuiApp {
    pub(super) fn process_actions(&mut self) {
        let Some(active_index) = self.normalize_active_tab() else {
            return;
        };
        let actions: Vec<UiAction> = self.tabs[active_index].state.ui.actions.drain(..).collect();
        let Self {
            tabs,
            settings,
            github_auth_session,
            logger,
            ..
        } = self;
        let mut ctx = TabActionContext {
            tab: &mut tabs[active_index],
            settings,
            github_auth_session,
            logger,
        };

        for action in actions {
            action.apply(&mut ctx);
        }
    }
}

fn clear_repo_selection(inspector_state: &mut InspectorState) {
    inspector_state.selected_file = None;
    inspector_state.diff_content.clear();
    inspector_state.conflict_data = None;
}

fn log_action_error(ctx: &mut TabActionContext<'_>, context: &str, detail: String) {
    ctx.tab.state.ui.status_msg = helpers::status_message_for_error(context, &detail);
    ctx.logger.log_error(context, &detail);
}

fn log_worker_dispatch_error(ctx: &mut TabActionContext<'_>, context: &str) {
    ctx.tab.state.ui.status_msg = helpers::status_message_for_worker_dispatch(context);
    ctx.logger
        .log_error(context, helpers::WORKER_DISPATCH_ERROR_DETAIL);
}

fn refresh_tab(ctx: &mut TabActionContext<'_>) {
    let (repo_state, worktree_state, commit_state, inspector_state, ui_state) =
        ctx.tab.state.refresh_parts_mut();
    if let Some(detail) = helpers::refresh_status(
        repo_state,
        worktree_state,
        commit_state,
        inspector_state,
        ui_state,
        &ctx.tab.repo,
    ) {
        ctx.logger.log_error("Refresh", &detail);
    }
}
