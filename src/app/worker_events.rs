use super::ports::AppWorktreeMetadata;
use super::{helpers, *};
use crate::shared::worktree_metadata::{WorktreeMetadata, storage_key};
use crate::shared::worktrees::{CreatedWorktree, LinkedWorktree};
use crate::worker::{
    CloneRepoResult, CreateGithubRepoResult, CreatePullRequestResult, CreateTagResult,
    CreateWorktreeResult, DiscardAndResetResult, GithubAuthPromptResult, GithubAuthResult,
    HandleRepoTaskResult, HandleWelcomeTaskResult, ListGithubReposResult, OpenPullRequestResult,
    PullResult, PushResult, RemoveWorktreeResult, UndoLastCommitResult,
};

pub(crate) struct WelcomeWorkerContext<'a> {
    app: &'a mut GitGuiApp,
}

pub(crate) struct RepoWorkerContext<'a> {
    tab: &'a mut RepoTab,
    refresh_requested: &'a mut bool,
}

impl<'a> WelcomeWorkerContext<'a> {
    fn new(app: &'a mut GitGuiApp) -> Self {
        Self { app }
    }
}

impl<'a> RepoWorkerContext<'a> {
    fn request_refresh(&mut self) {
        *self.refresh_requested = true;
    }

    /// Record the commit a freshly created worktree started from.
    ///
    /// Bookkeeping around an operation that already succeeded, so a failure is
    /// logged rather than raised over the success — the rule session writes
    /// follow. The metadata text itself is never logged.
    fn record_base_commit(&mut self, created: &CreatedWorktree) {
        let key = format!("wt:{}", created.name);
        let metadata = WorktreeMetadata {
            base_commit: created.base_commit.clone(),
            // Recorded here for the same reason as the base commit: this is the
            // only moment the app knows the worktree is new.
            started: helpers::now_secs(),
            ..WorktreeMetadata::default()
        };
        self.write_metadata(&key, Some(metadata), "Record worktree base commit");
    }

    /// Drop a removed worktree's metadata, so a later worktree reusing the name
    /// does not inherit it.
    fn forget_metadata(&mut self, worktree: &LinkedWorktree) {
        let key = storage_key(worktree);
        self.write_metadata(&key, None, "Forget worktree metadata");
    }

    fn write_metadata(&mut self, key: &str, entry: Option<WorktreeMetadata>, context: &str) {
        match AppWorktreeMetadata::update(&self.tab.repo, key, entry) {
            Ok(entries) => self.tab.state.repo.worktree_metadata = entries,
            Err(detail) => self.tab.logger.log_error(context, &detail),
        }
    }

    fn log_error(&mut self, context: &str, detail: &str) {
        self.tab.logger.log_error(context, detail);
    }
}

impl HandleWelcomeTaskResult for GithubAuthPromptResult {
    fn apply(self: Box<Self>, ctx: &mut WelcomeWorkerContext<'_>) {
        let message = format!(
            "Enter GitHub code {} to finish signing in.",
            self.0.user_code
        );
        ctx.app.github_auth_prompt = Some(self.0.clone());
        ctx.app.publish_dialog.github_status = message.clone();
        ctx.app.publish_dialog.operation_status = format!(
            "If GitHub did not open automatically, visit {}.",
            self.0.verification_uri
        );
        ctx.app.welcome_status = StatusMessage::info(message.clone());
        ctx.app.set_status_message(StatusMessage::info(message));
    }
}

impl HandleWelcomeTaskResult for GithubAuthResult {
    fn apply(self: Box<Self>, ctx: &mut WelcomeWorkerContext<'_>) {
        ctx.app.welcome_busy = None;

        match self.0 {
            Ok(session) => {
                let persistence_result = AppGitHubAuth::save_session(&session);
                // Signing in worked either way; a failed keychain write still
                // leaves the session usable for this run, so it reports as an
                // error rather than quietly passing as a success.
                let status = match &persistence_result {
                    Ok(()) => StatusMessage::success(format!(
                        "GitHub sign-in complete for @{}",
                        session.login
                    )),
                    Err(error) => {
                        ctx.app.logger.log_error("GitHub sign-in", error);
                        StatusMessage::error(format!(
                            "GitHub sign-in complete for @{}, but {}",
                            session.login,
                            logging::summarize_for_ui(error)
                        ))
                    }
                };
                ctx.app.github_auth_prompt = None;
                ctx.app.github_auth_session = Some(session);
                ctx.app.publish_dialog.github_authenticated = true;
                ctx.app.publish_dialog.github_status = status.text().to_string();
                ctx.app.publish_dialog.operation_status.clear();
                ctx.app.welcome_status = status.clone();
                ctx.app.set_status_message(status);
            }
            Err(msg) => {
                ctx.app.logger.log_error("GitHub sign-in", &msg);
                ctx.app.github_auth_prompt = None;
                ctx.app.publish_dialog.github_authenticated = ctx.app.github_auth_session.is_some();
                let status = if let Some(session) = &ctx.app.github_auth_session {
                    StatusMessage::error(format!(
                        "Signed in to GitHub as @{} (latest sign-in failed: {})",
                        session.login,
                        logging::summarize_for_ui(&msg)
                    ))
                } else {
                    helpers::status_message_for_error("GitHub sign-in", &msg)
                };
                ctx.app.publish_dialog.github_status = status.text().to_string();
                ctx.app.publish_dialog.operation_status.clear();
                ctx.app.welcome_status = status.clone();
                ctx.app.set_status_message(status);
            }
        }
    }
}

impl HandleWelcomeTaskResult for CreateGithubRepoResult {
    fn apply(self: Box<Self>, ctx: &mut WelcomeWorkerContext<'_>) {
        ctx.app.welcome_busy = None;

        match self.0 {
            Ok(result) => {
                let status = StatusMessage::success(result.message.clone());
                ctx.app.publish_dialog.show = false;
                ctx.app.publish_dialog.operation_status.clear();
                ctx.app.welcome_status = status.clone();
                ctx.app.open_repo(result.folder_path);
                ctx.app.set_status_message(status);
            }
            Err(msg) => {
                ctx.app.logger.log_error("Publish to GitHub", &msg);
                let status = helpers::status_message_for_error("Publish to GitHub", &msg);
                ctx.app.publish_dialog.operation_status = status.text().to_string();
                ctx.app.welcome_status = status;
            }
        }
    }
}

impl HandleWelcomeTaskResult for ListGithubReposResult {
    fn apply(self: Box<Self>, ctx: &mut WelcomeWorkerContext<'_>) {
        match self.0 {
            Ok(list) => {
                ctx.app.clone_dialog.github_repos = list;
                ctx.app.clone_dialog.github_repos_loading = false;
                ctx.app.clone_dialog.github_repos_error = None;
            }
            Err(msg) => {
                ctx.app.clone_dialog.github_repos_loading = false;
                ctx.app.clone_dialog.github_repos_error = Some(msg.clone());
                ctx.app.logger.log_error("GitHub repos", &msg);
            }
        }
    }
}

impl HandleWelcomeTaskResult for CloneRepoResult {
    fn apply(self: Box<Self>, ctx: &mut WelcomeWorkerContext<'_>) {
        ctx.app.welcome_busy = None;

        match self.0 {
            Ok(path) => {
                ctx.app.clone_dialog.show = false;
                ctx.app.clone_dialog.status.clear();
                let status =
                    StatusMessage::success(format!("Cloned repository to {}", path.display()));
                ctx.app.welcome_status = status.clone();
                ctx.app.open_repo(path);
                ctx.app.set_status_message(status);
            }
            Err(msg) => {
                ctx.app.logger.log_error("Clone", &msg);
                let status = helpers::status_message_for_error("Clone", &msg);
                ctx.app.clone_dialog.status = status.text().to_string();
                ctx.app.welcome_status = status;
            }
        }
    }
}

impl HandleRepoTaskResult for PushResult {
    fn apply(self: Box<Self>, ctx: &mut RepoWorkerContext<'_>) {
        ctx.tab.state.ui.busy = None;

        match self.0 {
            Ok(result) => {
                let prompt_message = match &result.pull_request_prompt {
                    Some(PullRequestPrompt::Open { number, .. }) => {
                        format!(" Pull request #{} is ready.", number)
                    }
                    Some(PullRequestPrompt::Create { .. }) => {
                        " You can create a pull request now.".into()
                    }
                    None => String::new(),
                };
                ctx.tab.state.repo.pull_request_prompt = result.pull_request_prompt;
                ctx.tab.state.ui.status =
                    StatusMessage::success(format!("Push: {}{}", result.message, prompt_message));
                ctx.request_refresh();
            }
            Err(msg) => {
                ctx.tab.state.ui.status = helpers::status_message_for_error("Push", &msg);
                ctx.log_error("Push", &msg);
            }
        }
    }
}

impl HandleRepoTaskResult for PullResult {
    fn apply(self: Box<Self>, ctx: &mut RepoWorkerContext<'_>) {
        ctx.tab.state.ui.busy = None;

        match self.0 {
            Ok(msg) => {
                ctx.tab.state.ui.status = StatusMessage::success(format!("Pull: {}", msg));
                ctx.request_refresh();
            }
            Err(msg) => {
                ctx.tab.state.ui.status = helpers::status_message_for_error("Pull", &msg);
                ctx.log_error("Pull", &msg);
            }
        }
    }
}

impl HandleRepoTaskResult for CreateTagResult {
    fn apply(self: Box<Self>, ctx: &mut RepoWorkerContext<'_>) {
        ctx.tab.state.ui.busy = None;

        match self.0 {
            Ok(msg) => {
                ctx.tab.state.ui.status = StatusMessage::success(msg);
                ctx.tab.state.dialogs.tag.new_tag_name.clear();
                ctx.tab.state.dialogs.tag.focus_new_tag_name_requested = false;
                ctx.tab.state.dialogs.tag.show_create_tag_dialog = false;
            }
            Err(msg) => {
                ctx.tab.state.ui.status = helpers::status_message_for_error("Create tag", &msg);
                ctx.log_error("Create tag", &msg);
            }
        }

        ctx.request_refresh();
    }
}

impl HandleRepoTaskResult for OpenPullRequestResult {
    fn apply(self: Box<Self>, ctx: &mut RepoWorkerContext<'_>) {
        ctx.tab.state.ui.busy = None;

        match self.0 {
            Ok(msg) => {
                ctx.tab.state.ui.status = StatusMessage::success(msg);
            }
            Err(msg) => {
                ctx.tab.state.ui.status = helpers::status_message_for_error("Open PR", &msg);
                ctx.log_error("Open PR", &msg);
            }
        }
    }
}

impl HandleRepoTaskResult for CreatePullRequestResult {
    fn apply(self: Box<Self>, ctx: &mut RepoWorkerContext<'_>) {
        ctx.tab.state.ui.busy = None;

        match self.0 {
            Ok(msg) => {
                ctx.tab.state.ui.status = StatusMessage::success(msg);
            }
            Err(msg) => {
                ctx.tab.state.ui.status = helpers::status_message_for_error("Create PR", &msg);
                ctx.log_error("Create PR", &msg);
            }
        }
    }
}

impl HandleRepoTaskResult for DiscardAndResetResult {
    fn apply(self: Box<Self>, ctx: &mut RepoWorkerContext<'_>) {
        ctx.tab.state.ui.busy = None;

        match self.0 {
            Ok(msg) => {
                ctx.tab.state.ui.status = StatusMessage::success(format!("Discard: {}", msg));
                ctx.tab.state.dialogs.discard.show_discard_dialog = false;
                ctx.tab.state.dialogs.discard.discard_preview = None;
                ctx.tab.state.dialogs.discard.discard_clean_untracked = false;
            }
            Err(msg) => {
                ctx.tab.state.ui.status =
                    helpers::status_message_for_error("Discard & reset", &msg);
                ctx.log_error("Discard & reset", &msg);
            }
        }

        ctx.request_refresh();
    }
}

impl HandleRepoTaskResult for UndoLastCommitResult {
    fn apply(self: Box<Self>, ctx: &mut RepoWorkerContext<'_>) {
        ctx.tab.state.ui.busy = None;

        match self.0 {
            Ok(msg) => {
                ctx.tab.state.ui.status =
                    StatusMessage::success(format!("Undo last commit: {}", msg));
            }
            Err(msg) => {
                ctx.tab.state.ui.status =
                    helpers::status_message_for_error("Undo last commit", &msg);
                ctx.log_error("Undo last commit", &msg);
            }
        }

        ctx.request_refresh();
    }
}

impl HandleRepoTaskResult for CreateWorktreeResult {
    fn apply(self: Box<Self>, ctx: &mut RepoWorkerContext<'_>) {
        ctx.tab.state.ui.busy = None;

        match self.0 {
            Ok(outcome) => {
                ctx.tab.state.ui.status = StatusMessage::success(outcome.message);
                // The commit the checkout started from is only knowable here;
                // record it so the worktree's metadata can show where it began.
                ctx.record_base_commit(&outcome.created);
                // Only clear the form once the worktree really exists, so a
                // rejected request comes back with everything still typed in.
                helpers::reset_worktree_dialog_state(&mut ctx.tab.state.dialogs.worktree);
            }
            Err(msg) => {
                ctx.tab.state.ui.status = helpers::status_message_for_error("New worktree", &msg);
                ctx.log_error("New worktree", &msg);
            }
        }

        ctx.request_refresh();
    }
}

impl HandleRepoTaskResult for RemoveWorktreeResult {
    fn apply(self: Box<Self>, ctx: &mut RepoWorkerContext<'_>) {
        ctx.tab.state.ui.busy = None;
        let removed = self.0.is_ok();

        match self.0 {
            Ok(msg) => {
                ctx.tab.state.ui.status = StatusMessage::success(msg);
            }
            Err(msg) => {
                ctx.tab.state.ui.status =
                    helpers::status_message_for_error("Remove worktree", &msg);
                ctx.log_error("Remove worktree", &msg);
            }
        }

        // Read the name before clearing the confirmation: the result carries
        // only a sentence, and the worktree awaiting confirmation is the only
        // record of which one this was.
        if removed && let Some(worktree) = ctx.tab.state.dialogs.worktree.pending_remove.clone() {
            ctx.forget_metadata(&worktree);
        }

        // The confirmation is closed either way: it was answered, and the
        // refreshed list is what says whether the worktree survived.
        ctx.tab.state.dialogs.worktree.pending_remove = None;
        ctx.request_refresh();
    }
}

impl GitGuiApp {
    pub(super) fn poll_workers(&mut self) -> bool {
        while let Some(result) = self.welcome_worker.try_recv() {
            let mut ctx = WelcomeWorkerContext::new(self);
            result.apply(&mut ctx);
        }

        let mut any_busy = self.welcome_worker.is_busy();

        for tab in &mut self.tabs {
            let mut refresh_requested = false;

            while let Some(result) = tab.worker.try_recv() {
                let mut ctx = RepoWorkerContext {
                    tab,
                    refresh_requested: &mut refresh_requested,
                };
                result.apply(&mut ctx);
            }

            if tab.worker.is_busy() {
                any_busy = true;
            }

            if refresh_requested {
                refresh_repo_tab(tab);
            }
        }

        any_busy
    }
}

fn refresh_repo_tab(tab: &mut RepoTab) {
    let Some(path) = tab.state.repo.path.clone() else {
        return;
    };

    match AppRepoRead::open(&path) {
        Ok(repo) => {
            let refresh_result = {
                let (repo_state, worktree_state, commit_state, inspector_state, ui_state) =
                    tab.state.refresh_parts_mut();
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
                tab.logger.log_error("Refresh", &detail);
            }
            tab.repo = repo;
        }
        Err(error) => {
            let detail = error.to_string();
            tab.state.ui.status = helpers::status_message_for_error("Refresh", &detail);
            tab.logger.log_error("Refresh", &detail);
        }
    }
}
