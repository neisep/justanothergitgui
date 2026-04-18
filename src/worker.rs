use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use crate::app::{RepoWorkerContext, WelcomeWorkerContext};
use crate::shared::github::{
    CreateGithubRepoRequest, CreateGithubRepoSuccess, GithubAuthPrompt, GithubAuthSession,
    GithubRepoSummary, PushSuccess,
};

pub trait HandleTaskResult: Send {
    fn apply_to_welcome(self: Box<Self>, _ctx: &mut WelcomeWorkerContext<'_>) {}

    fn apply_to_repo(self: Box<Self>, _ctx: &mut RepoWorkerContext<'_>) {}
}

pub struct TaskResult(Box<dyn HandleTaskResult>);

pub(crate) struct PushResult(pub(crate) Result<PushSuccess, String>);
pub(crate) struct PullResult(pub(crate) Result<String, String>);
pub(crate) struct CreateTagResult(pub(crate) Result<String, String>);
pub(crate) struct GithubAuthPromptResult(pub(crate) GithubAuthPrompt);
pub(crate) struct GithubAuthResult(pub(crate) Result<GithubAuthSession, String>);
pub(crate) struct CreateGithubRepoResult(pub(crate) Result<CreateGithubRepoSuccess, String>);
pub(crate) struct OpenPullRequestResult(pub(crate) Result<String, String>);
pub(crate) struct CreatePullRequestResult(pub(crate) Result<String, String>);
pub(crate) struct DiscardAndResetResult(pub(crate) Result<String, String>);
pub(crate) struct ListGithubReposResult(pub(crate) Result<Vec<GithubRepoSummary>, String>);
pub(crate) struct CloneRepoResult(pub(crate) Result<PathBuf, String>);

impl TaskResult {
    fn new(result: impl HandleTaskResult + 'static) -> Self {
        Self(Box::new(result))
    }

    fn push(result: Result<PushSuccess, String>) -> Self {
        Self::new(PushResult(result))
    }

    fn pull(result: Result<String, String>) -> Self {
        Self::new(PullResult(result))
    }

    fn create_tag(result: Result<String, String>) -> Self {
        Self::new(CreateTagResult(result))
    }

    fn github_auth_prompt(prompt: GithubAuthPrompt) -> Self {
        Self::new(GithubAuthPromptResult(prompt))
    }

    fn github_auth(result: Result<GithubAuthSession, String>) -> Self {
        Self::new(GithubAuthResult(result))
    }

    fn create_github_repo(result: Result<CreateGithubRepoSuccess, String>) -> Self {
        Self::new(CreateGithubRepoResult(result))
    }

    fn open_pull_request(result: Result<String, String>) -> Self {
        Self::new(OpenPullRequestResult(result))
    }

    fn create_pull_request(result: Result<String, String>) -> Self {
        Self::new(CreatePullRequestResult(result))
    }

    fn discard_and_reset(result: Result<String, String>) -> Self {
        Self::new(DiscardAndResetResult(result))
    }

    fn list_github_repos(result: Result<Vec<GithubRepoSummary>, String>) -> Self {
        Self::new(ListGithubReposResult(result))
    }

    fn clone_repo(result: Result<PathBuf, String>) -> Self {
        Self::new(CloneRepoResult(result))
    }

    pub(crate) fn apply_to_welcome(self, ctx: &mut WelcomeWorkerContext<'_>) {
        self.0.apply_to_welcome(ctx);
    }

    pub(crate) fn apply_to_repo(self, ctx: &mut RepoWorkerContext<'_>) {
        self.0.apply_to_repo(ctx);
    }
}

enum WorkerTask {
    Push(PathBuf, Option<GithubAuthSession>),
    Pull(PathBuf, Option<GithubAuthSession>),
    CreateTag(PathBuf, String, Option<GithubAuthSession>),
    GithubAuth {
        client_id: String,
    },
    CreateGithubRepo(CreateGithubRepoRequest),
    OpenPullRequest(String),
    CreatePullRequest(String),
    DiscardAndReset {
        path: PathBuf,
        auth: Option<GithubAuthSession>,
        clean_untracked: bool,
    },
    ListGithubRepos {
        auth: GithubAuthSession,
    },
    CloneRepo {
        url: String,
        dest: PathBuf,
        auth: Option<GithubAuthSession>,
    },
}

pub struct Worker {
    tx: mpsc::Sender<WorkerTask>,
    rx: mpsc::Receiver<TaskResult>,
    busy: Arc<AtomicBool>,
}

impl Worker {
    pub fn new() -> Self {
        let (task_tx, task_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        let busy = Arc::new(AtomicBool::new(false));
        let busy_clone = busy.clone();

        std::thread::spawn(move || {
            while let Ok(task) = task_rx.recv() {
                busy_clone.store(true, Ordering::SeqCst);
                let result = match task {
                    WorkerTask::Push(path, auth) => {
                        TaskResult::push(crate::git_ops::push(&path, auth.as_ref()))
                    }
                    WorkerTask::Pull(path, auth) => {
                        TaskResult::pull(crate::git_ops::pull(&path, auth.as_ref()))
                    }
                    WorkerTask::CreateTag(path, tag_name, auth) => TaskResult::create_tag(
                        crate::git_ops::create_tag(&path, &tag_name, auth.as_ref()),
                    ),
                    WorkerTask::GithubAuth { client_id } => {
                        let prompt_tx = result_tx.clone();
                        TaskResult::github_auth(crate::git_ops::github_auth_login(
                            &client_id,
                            move |prompt| {
                                let _ = prompt_tx.send(TaskResult::github_auth_prompt(prompt));
                            },
                        ))
                    }
                    WorkerTask::CreateGithubRepo(request) => {
                        TaskResult::create_github_repo(crate::git_ops::create_github_repo(&request))
                    }
                    WorkerTask::OpenPullRequest(url) => {
                        TaskResult::open_pull_request(crate::git_ops::open_pull_request(&url))
                    }
                    WorkerTask::CreatePullRequest(url) => {
                        TaskResult::create_pull_request(crate::git_ops::create_pull_request(&url))
                    }
                    WorkerTask::DiscardAndReset {
                        path,
                        auth,
                        clean_untracked,
                    } => {
                        TaskResult::discard_and_reset(crate::git_ops::discard_and_reset_to_remote(
                            &path,
                            auth.as_ref(),
                            clean_untracked,
                        ))
                    }
                    WorkerTask::ListGithubRepos { auth } => TaskResult::list_github_repos(
                        crate::git_ops::list_github_repositories(&auth),
                    ),
                    WorkerTask::CloneRepo { url, dest, auth } => TaskResult::clone_repo(
                        crate::git_ops::clone_repository(&url, &dest, auth.as_ref()),
                    ),
                };
                let _ = result_tx.send(result);
                busy_clone.store(false, Ordering::SeqCst);
            }
        });

        Worker {
            tx: task_tx,
            rx: result_rx,
            busy,
        }
    }

    pub fn push(&self, repo_path: PathBuf, auth: Option<GithubAuthSession>) {
        if !self.is_busy() {
            let _ = self.tx.send(WorkerTask::Push(repo_path, auth));
        }
    }

    pub fn pull(&self, repo_path: PathBuf, auth: Option<GithubAuthSession>) {
        if !self.is_busy() {
            let _ = self.tx.send(WorkerTask::Pull(repo_path, auth));
        }
    }

    pub fn create_tag(
        &self,
        repo_path: PathBuf,
        tag_name: String,
        auth: Option<GithubAuthSession>,
    ) {
        if !self.is_busy() {
            let _ = self
                .tx
                .send(WorkerTask::CreateTag(repo_path, tag_name, auth));
        }
    }

    pub fn login_github(&self, client_id: String) {
        if !self.is_busy() {
            let _ = self.tx.send(WorkerTask::GithubAuth { client_id });
        }
    }

    pub fn create_github_repo(&self, request: CreateGithubRepoRequest) {
        if !self.is_busy() {
            let _ = self.tx.send(WorkerTask::CreateGithubRepo(request));
        }
    }

    pub fn open_pull_request(&self, url: String) {
        if !self.is_busy() {
            let _ = self.tx.send(WorkerTask::OpenPullRequest(url));
        }
    }

    pub fn create_pull_request(&self, url: String) {
        if !self.is_busy() {
            let _ = self.tx.send(WorkerTask::CreatePullRequest(url));
        }
    }

    pub fn discard_and_reset(
        &self,
        repo_path: PathBuf,
        auth: Option<GithubAuthSession>,
        clean_untracked: bool,
    ) {
        if !self.is_busy() {
            let _ = self.tx.send(WorkerTask::DiscardAndReset {
                path: repo_path,
                auth,
                clean_untracked,
            });
        }
    }

    pub fn list_github_repos(&self, auth: GithubAuthSession) {
        if !self.is_busy() {
            let _ = self.tx.send(WorkerTask::ListGithubRepos { auth });
        }
    }

    pub fn clone_repo(&self, url: String, dest: PathBuf, auth: Option<GithubAuthSession>) {
        if !self.is_busy() {
            let _ = self.tx.send(WorkerTask::CloneRepo { url, dest, auth });
        }
    }

    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    pub fn try_recv(&self) -> Option<TaskResult> {
        self.rx.try_recv().ok()
    }
}
