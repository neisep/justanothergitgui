use std::any::Any;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use crate::app::{AppRepoWorkerOps, AppWelcomeWorkerOps, RepoWorkerContext, WelcomeWorkerContext};
use crate::shared::github::{
    CreateGithubRepoRequest, CreateGithubRepoSuccess, GithubAuthPrompt, GithubAuthSession,
    GithubRepoSummary, PushSuccess,
};

pub trait HandleWelcomeTaskResult: Send {
    fn apply(self: Box<Self>, ctx: &mut WelcomeWorkerContext<'_>);
}

pub trait HandleRepoTaskResult: Send {
    fn apply(self: Box<Self>, ctx: &mut RepoWorkerContext<'_>);
}

pub struct WelcomeTaskResult(Box<dyn HandleWelcomeTaskResult>);
pub struct RepoTaskResult(Box<dyn HandleRepoTaskResult>);

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
#[cfg(test)]
struct WelcomeNoopResult;
#[cfg(test)]
struct RepoNoopResult;

impl WelcomeTaskResult {
    fn new(result: impl HandleWelcomeTaskResult + 'static) -> Self {
        Self(Box::new(result))
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

    fn list_github_repos(result: Result<Vec<GithubRepoSummary>, String>) -> Self {
        Self::new(ListGithubReposResult(result))
    }

    fn clone_repo(result: Result<PathBuf, String>) -> Self {
        Self::new(CloneRepoResult(result))
    }

    #[cfg(test)]
    fn noop() -> Self {
        Self::new(WelcomeNoopResult)
    }

    pub(crate) fn apply(self, ctx: &mut WelcomeWorkerContext<'_>) {
        self.0.apply(ctx);
    }
}

impl RepoTaskResult {
    fn new(result: impl HandleRepoTaskResult + 'static) -> Self {
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

    fn open_pull_request(result: Result<String, String>) -> Self {
        Self::new(OpenPullRequestResult(result))
    }

    fn create_pull_request(result: Result<String, String>) -> Self {
        Self::new(CreatePullRequestResult(result))
    }

    fn discard_and_reset(result: Result<String, String>) -> Self {
        Self::new(DiscardAndResetResult(result))
    }

    #[cfg(test)]
    fn noop() -> Self {
        Self::new(RepoNoopResult)
    }

    pub(crate) fn apply(self, ctx: &mut RepoWorkerContext<'_>) {
        self.0.apply(ctx);
    }
}

enum WelcomeWorkerTask {
    GithubAuth {
        client_id: String,
    },
    CreateGithubRepo(CreateGithubRepoRequest),
    ListGithubRepos {
        auth: GithubAuthSession,
    },
    CloneRepo {
        url: String,
        dest: PathBuf,
        auth: Option<GithubAuthSession>,
    },
    #[cfg(test)]
    Panic,
}

#[derive(Clone, Copy)]
enum WelcomeWorkerTaskKind {
    GithubAuth,
    CreateGithubRepo,
    ListGithubRepos,
    CloneRepo,
    #[cfg(test)]
    Panic,
}

enum RepoWorkerTask {
    Push(PathBuf, Option<GithubAuthSession>),
    Pull(PathBuf, Option<GithubAuthSession>),
    CreateTag(PathBuf, String, Option<GithubAuthSession>),
    OpenPullRequest(String),
    CreatePullRequest(String),
    DiscardAndReset {
        path: PathBuf,
        auth: Option<GithubAuthSession>,
        clean_untracked: bool,
    },
    #[cfg(test)]
    Panic,
}

#[derive(Clone, Copy)]
enum RepoWorkerTaskKind {
    Push,
    Pull,
    CreateTag,
    OpenPullRequest,
    CreatePullRequest,
    DiscardAndReset,
    #[cfg(test)]
    Panic,
}

trait WorkerTaskKind<Output>: Copy {
    fn panic_result(self, message: String) -> Output;
}

trait WorkerTaskSpec<Output>: Send + 'static {
    type Kind: WorkerTaskKind<Output> + Copy + Send + 'static;

    fn kind(&self) -> Self::Kind;
    fn run(self, result_tx: &mpsc::Sender<Output>) -> Output;
}

impl WorkerTaskKind<WelcomeTaskResult> for WelcomeWorkerTaskKind {
    fn panic_result(self, message: String) -> WelcomeTaskResult {
        match self {
            Self::GithubAuth => WelcomeTaskResult::github_auth(Err(message)),
            Self::CreateGithubRepo => WelcomeTaskResult::create_github_repo(Err(message)),
            Self::ListGithubRepos => WelcomeTaskResult::list_github_repos(Err(message)),
            Self::CloneRepo => WelcomeTaskResult::clone_repo(Err(message)),
            #[cfg(test)]
            Self::Panic => WelcomeTaskResult::noop(),
        }
    }
}

impl WorkerTaskKind<RepoTaskResult> for RepoWorkerTaskKind {
    fn panic_result(self, message: String) -> RepoTaskResult {
        match self {
            Self::Push => RepoTaskResult::push(Err(message)),
            Self::Pull => RepoTaskResult::pull(Err(message)),
            Self::CreateTag => RepoTaskResult::create_tag(Err(message)),
            Self::OpenPullRequest => RepoTaskResult::open_pull_request(Err(message)),
            Self::CreatePullRequest => RepoTaskResult::create_pull_request(Err(message)),
            Self::DiscardAndReset => RepoTaskResult::discard_and_reset(Err(message)),
            #[cfg(test)]
            Self::Panic => RepoTaskResult::noop(),
        }
    }
}

impl WorkerTaskSpec<WelcomeTaskResult> for WelcomeWorkerTask {
    type Kind = WelcomeWorkerTaskKind;

    fn kind(&self) -> Self::Kind {
        match self {
            Self::GithubAuth { .. } => WelcomeWorkerTaskKind::GithubAuth,
            Self::CreateGithubRepo(..) => WelcomeWorkerTaskKind::CreateGithubRepo,
            Self::ListGithubRepos { .. } => WelcomeWorkerTaskKind::ListGithubRepos,
            Self::CloneRepo { .. } => WelcomeWorkerTaskKind::CloneRepo,
            #[cfg(test)]
            Self::Panic => WelcomeWorkerTaskKind::Panic,
        }
    }

    fn run(self, result_tx: &mpsc::Sender<WelcomeTaskResult>) -> WelcomeTaskResult {
        match self {
            WelcomeWorkerTask::GithubAuth { client_id } => {
                let prompt_tx = result_tx.clone();
                WelcomeTaskResult::github_auth(AppWelcomeWorkerOps::github_auth_login(
                    &client_id,
                    move |prompt| {
                        let _ = prompt_tx.send(WelcomeTaskResult::github_auth_prompt(prompt));
                    },
                ))
            }
            WelcomeWorkerTask::CreateGithubRepo(request) => WelcomeTaskResult::create_github_repo(
                AppWelcomeWorkerOps::create_github_repo(&request),
            ),
            WelcomeWorkerTask::ListGithubRepos { auth } => WelcomeTaskResult::list_github_repos(
                AppWelcomeWorkerOps::list_github_repositories(&auth),
            ),
            WelcomeWorkerTask::CloneRepo { url, dest, auth } => WelcomeTaskResult::clone_repo(
                AppWelcomeWorkerOps::clone_repository(&url, &dest, auth.as_ref()),
            ),
            #[cfg(test)]
            WelcomeWorkerTask::Panic => panic!("panic task"),
        }
    }
}

impl WorkerTaskSpec<RepoTaskResult> for RepoWorkerTask {
    type Kind = RepoWorkerTaskKind;

    fn kind(&self) -> Self::Kind {
        match self {
            Self::Push(..) => RepoWorkerTaskKind::Push,
            Self::Pull(..) => RepoWorkerTaskKind::Pull,
            Self::CreateTag(..) => RepoWorkerTaskKind::CreateTag,
            Self::OpenPullRequest(..) => RepoWorkerTaskKind::OpenPullRequest,
            Self::CreatePullRequest(..) => RepoWorkerTaskKind::CreatePullRequest,
            Self::DiscardAndReset { .. } => RepoWorkerTaskKind::DiscardAndReset,
            #[cfg(test)]
            Self::Panic => RepoWorkerTaskKind::Panic,
        }
    }

    fn run(self, _result_tx: &mpsc::Sender<RepoTaskResult>) -> RepoTaskResult {
        match self {
            RepoWorkerTask::Push(path, auth) => {
                RepoTaskResult::push(AppRepoWorkerOps::push(&path, auth.as_ref()))
            }
            RepoWorkerTask::Pull(path, auth) => {
                RepoTaskResult::pull(AppRepoWorkerOps::pull(&path, auth.as_ref()))
            }
            RepoWorkerTask::CreateTag(path, tag_name, auth) => RepoTaskResult::create_tag(
                AppRepoWorkerOps::create_tag(&path, &tag_name, auth.as_ref()),
            ),
            RepoWorkerTask::OpenPullRequest(url) => {
                RepoTaskResult::open_pull_request(AppRepoWorkerOps::open_pull_request(&url))
            }
            RepoWorkerTask::CreatePullRequest(url) => {
                RepoTaskResult::create_pull_request(AppRepoWorkerOps::create_pull_request(&url))
            }
            RepoWorkerTask::DiscardAndReset {
                path,
                auth,
                clean_untracked,
            } => RepoTaskResult::discard_and_reset(AppRepoWorkerOps::discard_and_reset_to_remote(
                &path,
                auth.as_ref(),
                clean_untracked,
            )),
            #[cfg(test)]
            RepoWorkerTask::Panic => panic!("panic task"),
        }
    }
}

struct BusyGuard {
    busy: Arc<AtomicBool>,
}

impl BusyGuard {
    fn new(busy: Arc<AtomicBool>) -> Self {
        Self { busy }
    }
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.busy.store(false, Ordering::SeqCst);
    }
}

fn worker_panic_message(payload: Box<dyn Any + Send>) -> String {
    let detail = if let Some(message) = payload.downcast_ref::<String>() {
        message.as_str()
    } else if let Some(message) = payload.downcast_ref::<&'static str>() {
        message
    } else {
        "unknown panic payload"
    };

    format!("internal error: worker task panicked: {detail}")
}

struct WorkerCore<Task, Output> {
    tx: mpsc::Sender<Task>,
    rx: mpsc::Receiver<Output>,
    busy: Arc<AtomicBool>,
}

impl<Task, Output> WorkerCore<Task, Output>
where
    Task: WorkerTaskSpec<Output>,
    Output: Send + 'static,
{
    fn new() -> Self {
        let (task_tx, task_rx) = mpsc::channel::<Task>();
        let (result_tx, result_rx) = mpsc::channel::<Output>();
        let busy = Arc::new(AtomicBool::new(false));
        let busy_clone = busy.clone();

        std::thread::spawn(move || {
            while let Ok(task) = task_rx.recv() {
                let _busy_guard = BusyGuard::new(busy_clone.clone());
                let task_kind = task.kind();
                let result = panic::catch_unwind(AssertUnwindSafe(|| task.run(&result_tx)))
                    .unwrap_or_else(|payload| {
                        task_kind.panic_result(worker_panic_message(payload))
                    });
                let _ = result_tx.send(result);
            }
        });

        Self {
            tx: task_tx,
            rx: result_rx,
            busy,
        }
    }

    #[must_use]
    fn dispatch(&self, task: Task) -> bool {
        if self
            .busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return false;
        }

        if self.tx.send(task).is_ok() {
            true
        } else {
            self.busy.store(false, Ordering::SeqCst);
            false
        }
    }

    fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    fn try_recv(&self) -> Option<Output> {
        self.rx.try_recv().ok()
    }
}

pub struct WelcomeWorker(WorkerCore<WelcomeWorkerTask, WelcomeTaskResult>);

impl WelcomeWorker {
    pub fn new() -> Self {
        Self(WorkerCore::new())
    }

    #[must_use]
    pub fn login_github(&self, client_id: String) -> bool {
        self.0.dispatch(WelcomeWorkerTask::GithubAuth { client_id })
    }

    #[must_use]
    pub fn create_github_repo(&self, request: CreateGithubRepoRequest) -> bool {
        self.0
            .dispatch(WelcomeWorkerTask::CreateGithubRepo(request))
    }

    #[must_use]
    pub fn list_github_repos(&self, auth: GithubAuthSession) -> bool {
        self.0.dispatch(WelcomeWorkerTask::ListGithubRepos { auth })
    }

    #[must_use]
    pub fn clone_repo(&self, url: String, dest: PathBuf, auth: Option<GithubAuthSession>) -> bool {
        self.0
            .dispatch(WelcomeWorkerTask::CloneRepo { url, dest, auth })
    }

    pub fn is_busy(&self) -> bool {
        self.0.is_busy()
    }

    pub fn try_recv(&self) -> Option<WelcomeTaskResult> {
        self.0.try_recv()
    }
}

pub struct RepoWorker(WorkerCore<RepoWorkerTask, RepoTaskResult>);

impl RepoWorker {
    pub fn new() -> Self {
        Self(WorkerCore::new())
    }

    #[must_use]
    pub fn push(&self, repo_path: PathBuf, auth: Option<GithubAuthSession>) -> bool {
        self.0.dispatch(RepoWorkerTask::Push(repo_path, auth))
    }

    #[must_use]
    pub fn pull(&self, repo_path: PathBuf, auth: Option<GithubAuthSession>) -> bool {
        self.0.dispatch(RepoWorkerTask::Pull(repo_path, auth))
    }

    #[must_use]
    pub fn create_tag(
        &self,
        repo_path: PathBuf,
        tag_name: String,
        auth: Option<GithubAuthSession>,
    ) -> bool {
        self.0
            .dispatch(RepoWorkerTask::CreateTag(repo_path, tag_name, auth))
    }

    #[must_use]
    pub fn open_pull_request(&self, url: String) -> bool {
        self.0.dispatch(RepoWorkerTask::OpenPullRequest(url))
    }

    #[must_use]
    pub fn create_pull_request(&self, url: String) -> bool {
        self.0.dispatch(RepoWorkerTask::CreatePullRequest(url))
    }

    #[must_use]
    pub fn discard_and_reset(
        &self,
        repo_path: PathBuf,
        auth: Option<GithubAuthSession>,
        clean_untracked: bool,
    ) -> bool {
        self.0.dispatch(RepoWorkerTask::DiscardAndReset {
            path: repo_path,
            auth,
            clean_untracked,
        })
    }

    pub fn is_busy(&self) -> bool {
        self.0.is_busy()
    }

    pub fn try_recv(&self) -> Option<RepoTaskResult> {
        self.0.try_recv()
    }
}

#[cfg(test)]
impl HandleWelcomeTaskResult for WelcomeNoopResult {
    fn apply(self: Box<Self>, _ctx: &mut WelcomeWorkerContext<'_>) {}
}

#[cfg(test)]
impl HandleRepoTaskResult for RepoNoopResult {
    fn apply(self: Box<Self>, _ctx: &mut RepoWorkerContext<'_>) {}
}

#[cfg(test)]
impl WelcomeWorker {
    fn panic_for_test(&self) -> bool {
        self.0.dispatch(WelcomeWorkerTask::Panic)
    }
}

#[cfg(test)]
impl RepoWorker {
    fn panic_for_test(&self) -> bool {
        self.0.dispatch(RepoWorkerTask::Panic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::{Duration, Instant};

    enum TestTask {
        Sleep(Duration),
    }

    #[derive(Clone, Copy)]
    enum TestTaskKind {
        Sleep,
    }

    impl WorkerTaskKind<()> for TestTaskKind {
        fn panic_result(self, _message: String) {}
    }

    impl WorkerTaskSpec<()> for TestTask {
        type Kind = TestTaskKind;

        fn kind(&self) -> Self::Kind {
            match self {
                Self::Sleep(..) => TestTaskKind::Sleep,
            }
        }

        fn run(self, _result_tx: &mpsc::Sender<()>) {
            match self {
                Self::Sleep(duration) => thread::sleep(duration),
            }
        }
    }

    #[test]
    fn welcome_worker_recovers_after_task_panic() {
        let worker = WelcomeWorker::new();

        assert!(worker.panic_for_test());
        assert!(wait_until(|| worker.try_recv().is_some()));
        assert!(wait_until(|| !worker.is_busy()));

        assert!(worker.panic_for_test());
        assert!(wait_until(|| worker.try_recv().is_some()));
        assert!(wait_until(|| !worker.is_busy()));
    }

    #[test]
    fn repo_worker_recovers_after_task_panic() {
        let worker = RepoWorker::new();

        assert!(worker.panic_for_test());
        assert!(wait_until(|| worker.try_recv().is_some()));
        assert!(wait_until(|| !worker.is_busy()));

        assert!(worker.panic_for_test());
        assert!(wait_until(|| worker.try_recv().is_some()));
        assert!(wait_until(|| !worker.is_busy()));
    }

    #[test]
    fn worker_dispatch_returns_false_while_busy() {
        let worker = WorkerCore::<TestTask, ()>::new();

        assert!(worker.dispatch(TestTask::Sleep(Duration::from_millis(50))));
        assert!(worker.is_busy());
        assert!(!worker.dispatch(TestTask::Sleep(Duration::from_millis(10))));
        assert!(wait_until(|| worker.try_recv().is_some()));
        assert!(wait_until(|| !worker.is_busy()));
    }

    fn wait_until(mut predicate: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            if predicate() {
                return true;
            }
            thread::sleep(Duration::from_millis(10));
        }
        predicate()
    }
}
