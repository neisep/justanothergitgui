use super::{helpers, *};
use crate::worker::{
    CloneRepoResult, CreateGithubRepoResult, CreatePullRequestResult, CreateTagResult,
    DiscardAndResetResult, GithubAuthPromptResult, GithubAuthResult, HandleRepoTaskResult,
    HandleWelcomeTaskResult, ListGithubReposResult, OpenPullRequestResult, PullResult, PushResult,
    UndoLastCommitResult,
};

pub(crate) struct WelcomeWorkerContext<'a> {
    app: &'a mut GitGuiApp,
}

pub(crate) struct RepoWorkerContext<'a> {
    tab: &'a mut RepoTab,
    logger: &'a mut AppLogger,
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

    fn log_error(&mut self, context: &str, detail: &str) {
        self.logger.log_error(context, detail);
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
        ctx.app.welcome_status = message.clone();
        ctx.app.set_status_message(message);
    }
}

impl HandleWelcomeTaskResult for GithubAuthResult {
    fn apply(self: Box<Self>, ctx: &mut WelcomeWorkerContext<'_>) {
        ctx.app.welcome_busy = None;

        match self.0 {
            Ok(session) => {
                let persistence_result = AppGitHubAuth::save_session(&session);
                let message = match &persistence_result {
                    Ok(()) => format!("GitHub sign-in complete for @{}", session.login),
                    Err(error) => {
                        ctx.app.logger.log_error("GitHub sign-in", error);
                        format!(
                            "GitHub sign-in complete for @{}, but {}",
                            session.login,
                            logging::summarize_for_ui(error)
                        )
                    }
                };
                ctx.app.github_auth_prompt = None;
                ctx.app.github_auth_session = Some(session);
                ctx.app.publish_dialog.github_authenticated = true;
                ctx.app.publish_dialog.github_status = message.clone();
                ctx.app.publish_dialog.operation_status.clear();
                ctx.app.welcome_status = message;
                ctx.app
                    .set_status_message(ctx.app.publish_dialog.github_status.clone());
            }
            Err(msg) => {
                ctx.app.logger.log_error("GitHub sign-in", &msg);
                ctx.app.github_auth_prompt = None;
                ctx.app.publish_dialog.github_authenticated = ctx.app.github_auth_session.is_some();
                ctx.app.publish_dialog.github_status =
                    if let Some(session) = &ctx.app.github_auth_session {
                        format!(
                            "Signed in to GitHub as @{} (latest sign-in failed: {})",
                            session.login,
                            logging::summarize_for_ui(&msg)
                        )
                    } else {
                        helpers::status_message_for_error("GitHub sign-in", &msg)
                    };
                ctx.app.publish_dialog.operation_status.clear();
                ctx.app.welcome_status = helpers::status_message_for_error("GitHub sign-in", &msg);
                ctx.app
                    .set_status_message(ctx.app.publish_dialog.github_status.clone());
            }
        }
    }
}

impl HandleWelcomeTaskResult for CreateGithubRepoResult {
    fn apply(self: Box<Self>, ctx: &mut WelcomeWorkerContext<'_>) {
        ctx.app.welcome_busy = None;

        match self.0 {
            Ok(result) => {
                let message = result.message.clone();
                ctx.app.publish_dialog.show = false;
                ctx.app.publish_dialog.operation_status.clear();
                ctx.app.welcome_status = message.clone();
                ctx.app.open_repo(result.folder_path);
                ctx.app.set_status_message(message);
            }
            Err(msg) => {
                ctx.app.logger.log_error("Publish to GitHub", &msg);
                ctx.app.publish_dialog.operation_status =
                    helpers::status_message_for_error("Publish to GitHub", &msg);
                ctx.app.welcome_status =
                    helpers::status_message_for_error("Publish to GitHub", &msg);
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
                let message = format!("Cloned repository to {}", path.display());
                ctx.app.welcome_status = message.clone();
                ctx.app.open_repo(path);
                ctx.app.set_status_message(message);
            }
            Err(msg) => {
                ctx.app.logger.log_error("Clone", &msg);
                ctx.app.clone_dialog.status = helpers::status_message_for_error("Clone", &msg);
                ctx.app.welcome_status = helpers::status_message_for_error("Clone", &msg);
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
                ctx.tab.state.ui.status_msg = format!("Push: {}{}", result.message, prompt_message);
                ctx.request_refresh();
            }
            Err(msg) => {
                ctx.tab.state.ui.status_msg = helpers::status_message_for_error("Push", &msg);
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
                ctx.tab.state.ui.status_msg = format!("Pull: {}", msg);
                ctx.request_refresh();
            }
            Err(msg) => {
                ctx.tab.state.ui.status_msg = helpers::status_message_for_error("Pull", &msg);
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
                ctx.tab.state.ui.status_msg = msg;
                ctx.tab.state.dialogs.tag.new_tag_name.clear();
                ctx.tab.state.dialogs.tag.focus_new_tag_name_requested = false;
                ctx.tab.state.dialogs.tag.show_create_tag_dialog = false;
            }
            Err(msg) => {
                ctx.tab.state.ui.status_msg = helpers::status_message_for_error("Create tag", &msg);
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
                ctx.tab.state.ui.status_msg = msg;
            }
            Err(msg) => {
                ctx.tab.state.ui.status_msg = helpers::status_message_for_error("Open PR", &msg);
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
                ctx.tab.state.ui.status_msg = msg;
            }
            Err(msg) => {
                ctx.tab.state.ui.status_msg = helpers::status_message_for_error("Create PR", &msg);
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
                ctx.tab.state.ui.status_msg = format!("Discard: {}", msg);
                ctx.tab.state.dialogs.discard.show_discard_dialog = false;
                ctx.tab.state.dialogs.discard.discard_preview = None;
                ctx.tab.state.dialogs.discard.discard_clean_untracked = false;
            }
            Err(msg) => {
                ctx.tab.state.ui.status_msg =
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
                ctx.tab.state.ui.status_msg = format!("Undo last commit: {}", msg);
            }
            Err(msg) => {
                ctx.tab.state.ui.status_msg =
                    helpers::status_message_for_error("Undo last commit", &msg);
                ctx.log_error("Undo last commit", &msg);
            }
        }

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
        let logger = &mut self.logger;

        for tab in &mut self.tabs {
            let mut refresh_requested = false;

            while let Some(result) = tab.worker.try_recv() {
                let mut ctx = RepoWorkerContext {
                    tab,
                    logger,
                    refresh_requested: &mut refresh_requested,
                };
                result.apply(&mut ctx);
            }

            if tab.worker.is_busy() {
                any_busy = true;
            }

            if refresh_requested {
                refresh_repo_tab(tab, logger);
            }
        }

        any_busy
    }
}

fn refresh_repo_tab(tab: &mut RepoTab, logger: &mut AppLogger) {
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
                logger.log_error("Refresh", &detail);
            }
            tab.repo = repo;
        }
        Err(error) => {
            let detail = error.to_string();
            tab.state.ui.status_msg = helpers::status_message_for_error("Refresh", &detail);
            logger.log_error("Refresh", &detail);
        }
    }
}
