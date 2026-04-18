use super::{helpers, *};
use crate::shared::actions::HandleUiAction;
use crate::state::{AppState, CenterView, SelectedFile};

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
        match git_ops::stage_file(&ctx.tab.repo, &self.0) {
            Ok(()) => ctx.tab.state.status_msg = format!("Staged: {}", self.0),
            Err(error) => log_action_error(ctx, "Stage", error.to_string()),
        }
        refresh_tab(ctx);
    }
}

impl HandleUiAction for UnstageFile {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        match git_ops::unstage_file(&ctx.tab.repo, &self.0) {
            Ok(()) => ctx.tab.state.status_msg = format!("Unstaged: {}", self.0),
            Err(error) => log_action_error(ctx, "Unstage", error.to_string()),
        }
        refresh_tab(ctx);
    }
}

impl HandleUiAction for StageAll {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        match git_ops::stage_all(&ctx.tab.repo) {
            Ok(()) => ctx.tab.state.status_msg = "Staged all changes".into(),
            Err(error) => log_action_error(ctx, "Stage all", error.to_string()),
        }
        refresh_tab(ctx);
    }
}

impl HandleUiAction for UnstageAll {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        match git_ops::unstage_all(&ctx.tab.repo) {
            Ok(()) => ctx.tab.state.status_msg = "Unstaged all changes".into(),
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
        let msg = commit_rules::build_message(&tab.state.commit_summary, &tab.state.commit_body);

        match commit_rules::validate_for_submit(ruleset, &msg) {
            Ok(()) => {
                should_refresh = true;
                match git_ops::create_commit(&tab.repo, &msg) {
                    Ok(oid) => {
                        tab.state.status_msg = format!("Committed: {}", &oid.to_string()[..8]);
                        clear_repo_selection(&mut tab.state);
                        tab.state.commit_summary.clear();
                        tab.state.commit_body.clear();
                    }
                    Err(error) => log_action_error(ctx, "Commit", error.to_string()),
                }
            }
            Err(detail) => {
                tab.state.status_msg = detail;
            }
        }

        if should_refresh {
            refresh_tab(ctx);
        }
    }
}

impl HandleUiAction for Push {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        let tab = &mut ctx.tab;
        if let Some(path) = tab.state.repo_path.clone() {
            if tab.worker.is_busy() {
                tab.state.status_msg = "Busy — please wait...".into();
            } else {
                let busy = BusyState::new(BusyAction::Push, "Pushing...");
                tab.state.status_msg = busy.label.clone();
                tab.state.busy = Some(busy);
                tab.worker.push(path, ctx.github_auth_session.clone());
            }
        }
    }
}

impl HandleUiAction for Pull {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        let tab = &mut ctx.tab;
        if let Some(path) = tab.state.repo_path.clone() {
            if tab.worker.is_busy() {
                tab.state.status_msg = "Busy — please wait...".into();
            } else {
                let busy = BusyState::new(BusyAction::Pull, "Pulling...");
                tab.state.status_msg = busy.label.clone();
                tab.state.busy = Some(busy);
                tab.worker.pull(path, ctx.github_auth_session.clone());
            }
        }
    }
}

impl HandleUiAction for SelectFile {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        ctx.tab.state.center_view = CenterView::Diff;
        helpers::load_selected_file(&mut ctx.tab.state, &ctx.tab.repo, self.path, self.staged);
    }
}

impl HandleUiAction for SwitchBranch {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        match git_ops::switch_branch(&ctx.tab.repo, &self.0) {
            Ok(()) => {
                ctx.tab.state.status_msg = format!("Switched to {}", self.0);
                clear_repo_selection(&mut ctx.tab.state);
            }
            Err(error) => log_action_error(ctx, "Switch branch", error.to_string()),
        }
        refresh_tab(ctx);
    }
}

impl HandleUiAction for CreateBranch {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        match git_ops::create_branch(&ctx.tab.repo, &self.0) {
            Ok(()) => {
                ctx.tab.state.status_msg = format!("Created and switched to {}", self.0);
                clear_repo_selection(&mut ctx.tab.state);
                ctx.tab.state.new_branch_name.clear();
                ctx.tab.state.show_create_branch_dialog = false;
                ctx.tab.state.show_create_branch_confirm = false;
                ctx.tab.state.create_branch_preview = None;
                ctx.tab.state.pending_new_branch_name = None;
            }
            Err(error) => log_action_error(ctx, "Create branch", error.to_string()),
        }
        refresh_tab(ctx);
    }
}

impl HandleUiAction for OpenCreateBranchConfirm {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        let preview = git_ops::preview_create_branch(&ctx.tab.repo, &self.0);
        let clean =
            preview.dirty_files == 0 && preview.untracked_files == 0 && preview.staged_files == 0;

        if clean {
            ctx.tab.state.actions.push(UiAction::create_branch(self.0));
        } else {
            ctx.tab.state.pending_new_branch_name = Some(self.0);
            ctx.tab.state.create_branch_preview = Some(preview);
            ctx.tab.state.show_create_branch_dialog = false;
            ctx.tab.state.show_create_branch_confirm = true;
        }
    }
}

impl HandleUiAction for ConfirmCreateBranch {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        if let Some(name) = ctx.tab.state.pending_new_branch_name.take() {
            ctx.tab.state.actions.push(UiAction::create_branch(name));
        }
        ctx.tab.state.show_create_branch_confirm = false;
        ctx.tab.state.create_branch_preview = None;
    }
}

impl HandleUiAction for CreateTag {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        let tab = &mut ctx.tab;
        if let Some(path) = tab.state.repo_path.clone() {
            if tab.worker.is_busy() {
                tab.state.status_msg = "Busy — please wait...".into();
            } else if !git_ops::can_create_tag_on_branch(&tab.state.branch) {
                tab.state.status_msg =
                    "Tags can only be created from the main or master branch.".into();
            } else if tab.state.has_github_https_origin && ctx.github_auth_session.is_none() {
                tab.state.status_msg =
                    "Sign in to GitHub before creating tags for this repository.".into();
            } else {
                let busy =
                    BusyState::new(BusyAction::CreateTag, format!("Creating tag {}...", self.0));
                tab.state.status_msg = busy.label.clone();
                tab.state.busy = Some(busy);
                tab.worker
                    .create_tag(path, self.0, ctx.github_auth_session.clone());
            }
        }
    }
}

impl HandleUiAction for LaunchPullRequest {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        let Some(prompt) = ctx.tab.state.pull_request_prompt.clone() else {
            ctx.tab.state.status_msg = "No pull request action available".into();
            return;
        };

        if ctx.tab.worker.is_busy() {
            ctx.tab.state.status_msg = "Busy — please wait...".into();
            return;
        }

        match prompt {
            PullRequestPrompt::Open { number, url, .. } => {
                let busy = BusyState::new(
                    BusyAction::OpenPullRequest,
                    format!("Opening pull request #{}...", number),
                );
                ctx.tab.state.status_msg = busy.label.clone();
                ctx.tab.state.busy = Some(busy);
                ctx.tab.worker.open_pull_request(url);
            }
            PullRequestPrompt::Create { branch, url } => {
                let busy = BusyState::new(
                    BusyAction::CreatePullRequest,
                    format!("Opening pull request creation for {}...", branch),
                );
                ctx.tab.state.status_msg = busy.label.clone();
                ctx.tab.state.busy = Some(busy);
                ctx.tab.worker.create_pull_request(url);
            }
        }
    }
}

impl HandleUiAction for ShowDiff {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        ctx.tab.state.center_view = CenterView::Diff;
    }
}

impl HandleUiAction for ShowHistory {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        ctx.tab.state.center_view = CenterView::History;
    }
}

impl HandleUiAction for OpenCleanupBranches {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        match git_ops::list_stale_branches(&ctx.tab.repo) {
            Ok(stale) => {
                ctx.tab.state.stale_branches = stale;
                ctx.tab.state.show_cleanup_branches_dialog = true;
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
            match git_ops::delete_local_branch(&ctx.tab.repo, name) {
                Ok(()) => deleted.push(name.clone()),
                Err(error) => failures.push(format!("{}: {}", name, error)),
            }
        }

        ctx.tab
            .state
            .stale_branches
            .retain(|branch| !deleted.contains(&branch.name));

        if failures.is_empty() {
            ctx.tab.state.status_msg = format!("Deleted {} branch(es)", deleted.len());
            ctx.tab.state.show_cleanup_branches_dialog = false;
        } else {
            log_action_error(ctx, "Delete branch", failures.join("; "));
        }

        refresh_tab(ctx);
    }
}

impl HandleUiAction for OpenDiscardDialog {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        let preview = git_ops::preview_discard_damage(&ctx.tab.repo);
        ctx.tab.state.discard_preview = Some(preview);
        ctx.tab.state.discard_clean_untracked = false;
        ctx.tab.state.show_discard_dialog = true;
    }
}

impl HandleUiAction for DiscardAndReset {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        let tab = &mut ctx.tab;
        if let Some(path) = tab.state.repo_path.clone() {
            if tab.worker.is_busy() {
                tab.state.status_msg = "Busy — please wait...".into();
            } else {
                let busy = BusyState::new(BusyAction::DiscardAndReset, "Resetting to remote...");
                tab.state.status_msg = busy.label.clone();
                tab.state.busy = Some(busy);
                tab.worker.discard_and_reset(
                    path,
                    ctx.github_auth_session.clone(),
                    self.clean_untracked,
                );
            }
        }
    }
}

impl HandleUiAction for SaveConflictResolution {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>) {
        if let Some(conflict_data) = ctx.tab.state.conflict_data.clone() {
            let path = conflict_data.path.clone();
            match git_ops::write_resolved_file(&ctx.tab.repo, &conflict_data) {
                Ok(()) => {
                    ctx.tab.state.status_msg = format!("Resolved and staged: {}", path);
                    ctx.tab.state.selected_file = Some(SelectedFile { path, staged: true });
                    ctx.tab.state.conflict_data = None;
                }
                Err(error) => log_action_error(ctx, "Save resolution", error.to_string()),
            }
        } else {
            ctx.tab.state.status_msg = "No conflict selected".into();
        }

        refresh_tab(ctx);
    }
}

impl GitGuiApp {
    pub(super) fn process_actions(&mut self) {
        if self.tabs.is_empty() {
            return;
        }

        self.active_tab = self.active_tab.min(self.tabs.len() - 1);
        let active_index = self.active_tab;
        let actions: Vec<UiAction> = self.tabs[active_index].state.actions.drain(..).collect();
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

fn clear_repo_selection(state: &mut AppState) {
    state.selected_file = None;
    state.diff_content.clear();
    state.conflict_data = None;
}

fn log_action_error(ctx: &mut TabActionContext<'_>, context: &str, detail: String) {
    ctx.tab.state.status_msg = helpers::status_message_for_error(context, &detail);
    ctx.logger.log_error(context, &detail);
}

fn refresh_tab(ctx: &mut TabActionContext<'_>) {
    if let Some(detail) = helpers::refresh_status(&mut ctx.tab.state, &ctx.tab.repo) {
        ctx.logger.log_error("Refresh", &detail);
    }
}
