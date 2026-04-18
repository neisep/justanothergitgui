use super::{helpers, *};
use crate::shared::actions::HandleUiAction;
use crate::state::{CenterView, InspectorState, SelectedFile};

pub(crate) struct TabActionContext<'a> {
    tab: &'a mut RepoTab,
    settings: &'a AppSettings,
    github_auth_session: &'a Option<GithubAuthSession>,
    logger: &'a mut AppLogger,
}

struct StageFile(String);
struct UnstageFile(String);
struct StageAll;
struct UnstageAll;
struct Commit;
struct Push;
struct Pull;
struct SelectFile {
    path: String,
    staged: bool,
}
struct SwitchBranch(String);
struct CreateBranch(String);
struct OpenCreateBranchConfirm(String);
struct ConfirmCreateBranch;
struct CreateTag(String);
struct LaunchPullRequest;
struct ShowDiff;
struct ShowHistory;
struct OpenCleanupBranches;
struct DeleteStaleBranches(Vec<String>);
struct OpenDiscardDialog;
struct DiscardAndReset {
    clean_untracked: bool,
}
struct SaveConflictResolution;

impl UiAction {
    pub fn stage_file(path: impl Into<String>) -> Self {
        Self::new(StageFile(path.into()))
    }

    pub fn unstage_file(path: impl Into<String>) -> Self {
        Self::new(UnstageFile(path.into()))
    }

    pub fn stage_all() -> Self {
        Self::new(StageAll)
    }

    pub fn unstage_all() -> Self {
        Self::new(UnstageAll)
    }

    pub fn commit() -> Self {
        Self::new(Commit)
    }

    pub fn push() -> Self {
        Self::new(Push)
    }

    pub fn pull() -> Self {
        Self::new(Pull)
    }

    pub fn select_file(path: impl Into<String>, staged: bool) -> Self {
        Self::new(SelectFile {
            path: path.into(),
            staged,
        })
    }

    pub fn switch_branch(branch: impl Into<String>) -> Self {
        Self::new(SwitchBranch(branch.into()))
    }

    pub fn create_branch(branch: impl Into<String>) -> Self {
        Self::new(CreateBranch(branch.into()))
    }

    pub fn open_create_branch_confirm(branch: impl Into<String>) -> Self {
        Self::new(OpenCreateBranchConfirm(branch.into()))
    }

    pub fn confirm_create_branch() -> Self {
        Self::new(ConfirmCreateBranch)
    }

    pub fn create_tag(tag_name: impl Into<String>) -> Self {
        Self::new(CreateTag(tag_name.into()))
    }

    pub fn launch_pull_request() -> Self {
        Self::new(LaunchPullRequest)
    }

    pub fn show_diff() -> Self {
        Self::new(ShowDiff)
    }

    pub fn show_history() -> Self {
        Self::new(ShowHistory)
    }

    pub fn open_cleanup_branches() -> Self {
        Self::new(OpenCleanupBranches)
    }

    pub fn delete_stale_branches(names: Vec<String>) -> Self {
        Self::new(DeleteStaleBranches(names))
    }

    pub fn open_discard_dialog() -> Self {
        Self::new(OpenDiscardDialog)
    }

    pub fn discard_and_reset(clean_untracked: bool) -> Self {
        Self::new(DiscardAndReset { clean_untracked })
    }

    pub fn save_conflict_resolution() -> Self {
        Self::new(SaveConflictResolution)
    }
}

impl HandleUiAction for StageFile {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        match AppRepoWrite::stage_file(&ctx.tab.repo, &self.0) {
            Ok(()) => ctx.tab.state.ui.status_msg = format!("Staged: {}", self.0),
            Err(error) => log_action_error(ctx, "Stage", error.to_string()),
        }
        refresh_tab(ctx);
    }
}

impl HandleUiAction for UnstageFile {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        match AppRepoWrite::unstage_file(&ctx.tab.repo, &self.0) {
            Ok(()) => ctx.tab.state.ui.status_msg = format!("Unstaged: {}", self.0),
            Err(error) => log_action_error(ctx, "Unstage", error.to_string()),
        }
        refresh_tab(ctx);
    }
}

impl HandleUiAction for StageAll {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        match AppRepoWrite::stage_all(&ctx.tab.repo) {
            Ok(()) => ctx.tab.state.ui.status_msg = "Staged all changes".into(),
            Err(error) => log_action_error(ctx, "Stage all", error.to_string()),
        }
        refresh_tab(ctx);
    }
}

impl HandleUiAction for UnstageAll {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        match AppRepoWrite::unstage_all(&ctx.tab.repo) {
            Ok(()) => ctx.tab.state.ui.status_msg = "Unstaged all changes".into(),
            Err(error) => log_action_error(ctx, "Unstage all", error.to_string()),
        }
        refresh_tab(ctx);
    }
}

impl HandleUiAction for Commit {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
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
}

impl HandleUiAction for Push {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
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
}

impl HandleUiAction for Pull {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
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
}

impl HandleUiAction for SelectFile {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        let state = &mut ctx.tab.state;
        state.inspector.center_view = CenterView::Diff;
        helpers::load_selected_file(
            &state.worktree,
            &mut state.inspector,
            &ctx.tab.repo,
            self.path,
            self.staged,
        );
    }
}

impl HandleUiAction for SwitchBranch {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        match AppRepoWrite::switch_branch(&ctx.tab.repo, &self.0) {
            Ok(()) => {
                ctx.tab.state.ui.status_msg = format!("Switched to {}", self.0);
                clear_repo_selection(&mut ctx.tab.state.inspector);
            }
            Err(error) => log_action_error(ctx, "Switch branch", error.to_string()),
        }
        refresh_tab(ctx);
    }
}

impl HandleUiAction for CreateBranch {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        match AppRepoWrite::create_branch(&ctx.tab.repo, &self.0) {
            Ok(()) => {
                ctx.tab.state.ui.status_msg = format!("Created and switched to {}", self.0);
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
}

impl HandleUiAction for OpenCreateBranchConfirm {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        let preview = AppRepoWrite::preview_create_branch(&ctx.tab.repo, &self.0);
        let clean =
            preview.dirty_files == 0 && preview.untracked_files == 0 && preview.staged_files == 0;

        if clean {
            ctx.tab
                .state
                .ui
                .actions
                .push(UiAction::create_branch(self.0));
        } else {
            ctx.tab.state.dialogs.branch.pending_new_branch_name = Some(self.0);
            ctx.tab.state.dialogs.branch.create_branch_preview = Some(preview);
            ctx.tab.state.dialogs.branch.show_create_branch_dialog = false;
            ctx.tab.state.dialogs.branch.show_create_branch_confirm = true;
        }
    }
}

impl HandleUiAction for ConfirmCreateBranch {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        if let Some(name) = ctx.tab.state.dialogs.branch.pending_new_branch_name.take() {
            ctx.tab.state.ui.actions.push(UiAction::create_branch(name));
        }
        ctx.tab.state.dialogs.branch.show_create_branch_confirm = false;
        ctx.tab.state.dialogs.branch.create_branch_preview = None;
    }
}

impl HandleUiAction for CreateTag {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        if let Some(path) = ctx.tab.state.repo.path.clone() {
            if ctx.tab.worker.is_busy() {
                ctx.tab.state.ui.status_msg = "Busy — please wait...".into();
            } else if !AppRepoRead::can_create_tag_on_branch(&ctx.tab.state.repo.branch) {
                ctx.tab.state.ui.status_msg =
                    "Tags can only be created from the main or master branch.".into();
            } else if ctx.tab.state.repo.has_github_https_origin
                && ctx.github_auth_session.is_none()
            {
                ctx.tab.state.ui.status_msg =
                    "Sign in to GitHub before creating tags for this repository.".into();
            } else {
                let busy =
                    BusyState::new(BusyAction::CreateTag, format!("Creating tag {}...", self.0));
                if ctx
                    .tab
                    .worker
                    .create_tag(path, self.0, ctx.github_auth_session.clone())
                {
                    ctx.tab.state.ui.status_msg = busy.label.clone();
                    ctx.tab.state.ui.busy = Some(busy);
                } else {
                    log_worker_dispatch_error(ctx, "Create tag");
                }
            }
        }
    }
}

impl HandleUiAction for LaunchPullRequest {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
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
                    format!("Opening pull request #{}...", number),
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
                    format!("Opening pull request creation for {}...", branch),
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
}

impl HandleUiAction for ShowDiff {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        ctx.tab.state.inspector.center_view = CenterView::Diff;
    }
}

impl HandleUiAction for ShowHistory {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        ctx.tab.state.inspector.center_view = CenterView::History;
    }
}

impl HandleUiAction for OpenCleanupBranches {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        match AppRepoWrite::list_stale_branches(&ctx.tab.repo) {
            Ok(stale) => {
                ctx.tab.state.dialogs.cleanup.stale_branches = stale;
                ctx.tab.state.dialogs.cleanup.show_cleanup_branches_dialog = true;
            }
            Err(error) => log_action_error(ctx, "Cleanup branches", error.to_string()),
        }
    }
}

impl HandleUiAction for DeleteStaleBranches {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        let mut deleted = Vec::new();
        let mut failures = Vec::new();

        for name in &self.0 {
            match AppRepoWrite::delete_local_branch(&ctx.tab.repo, name) {
                Ok(()) => deleted.push(name.clone()),
                Err(error) => failures.push(format!("{}: {}", name, error)),
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
}

impl HandleUiAction for OpenDiscardDialog {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        let preview = AppRepoWrite::preview_discard_damage(&ctx.tab.repo);
        ctx.tab.state.dialogs.discard.discard_preview = Some(preview);
        ctx.tab.state.dialogs.discard.discard_clean_untracked = false;
        ctx.tab.state.dialogs.discard.show_discard_dialog = true;
    }
}

impl HandleUiAction for DiscardAndReset {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        if let Some(path) = ctx.tab.state.repo.path.clone() {
            if ctx.tab.worker.is_busy() {
                ctx.tab.state.ui.status_msg = "Busy — please wait...".into();
            } else {
                let busy = BusyState::new(BusyAction::DiscardAndReset, "Resetting to remote...");
                if ctx.tab.worker.discard_and_reset(
                    path,
                    ctx.github_auth_session.clone(),
                    self.clean_untracked,
                ) {
                    ctx.tab.state.ui.status_msg = busy.label.clone();
                    ctx.tab.state.ui.busy = Some(busy);
                } else {
                    log_worker_dispatch_error(ctx, "Discard & reset");
                }
            }
        }
    }
}

impl HandleUiAction for SaveConflictResolution {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        if let Some(conflict_data) = ctx.tab.state.inspector.conflict_data.clone() {
            let path = conflict_data.path.clone();
            match AppRepoWrite::write_resolved_file(&ctx.tab.repo, &conflict_data) {
                Ok(()) => {
                    ctx.tab.state.ui.status_msg = format!("Resolved and staged: {}", path);
                    ctx.tab.state.inspector.selected_file =
                        Some(SelectedFile { path, staged: true });
                    ctx.tab.state.inspector.conflict_data = None;
                }
                Err(error) => log_action_error(ctx, "Save resolution", error.to_string()),
            }
        } else {
            ctx.tab.state.ui.status_msg = "No conflict selected".into();
        }

        refresh_tab(ctx);
    }
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
