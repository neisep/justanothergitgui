use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

pub enum TaskResult {
    Push(Result<crate::git_ops::PushSuccess, String>),
    Pull(Result<String, String>),
    CreateTag(Result<String, String>),
    GithubAuthPrompt(crate::git_ops::GithubAuthPrompt),
    GithubAuth(Result<crate::git_ops::GithubAuthSession, String>),
    CreateGithubRepo(Result<crate::git_ops::CreateGithubRepoSuccess, String>),
    OpenPullRequest(Result<String, String>),
    CreatePullRequest(Result<String, String>),
}

enum WorkerTask {
    Push(PathBuf, Option<crate::git_ops::GithubAuthSession>),
    Pull(PathBuf, Option<crate::git_ops::GithubAuthSession>),
    CreateTag(PathBuf, String, Option<crate::git_ops::GithubAuthSession>),
    GithubAuth { client_id: String },
    CreateGithubRepo(crate::git_ops::CreateGithubRepoRequest),
    OpenPullRequest(String),
    CreatePullRequest(String),
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
                        TaskResult::Push(crate::git_ops::push(&path, auth.as_ref()))
                    }
                    WorkerTask::Pull(path, auth) => {
                        TaskResult::Pull(crate::git_ops::pull(&path, auth.as_ref()))
                    }
                    WorkerTask::CreateTag(path, tag_name, auth) => TaskResult::CreateTag(
                        crate::git_ops::create_tag(&path, &tag_name, auth.as_ref()),
                    ),
                    WorkerTask::GithubAuth { client_id } => {
                        let prompt_tx = result_tx.clone();
                        TaskResult::GithubAuth(crate::git_ops::github_auth_login(
                            &client_id,
                            move |prompt| {
                                let _ = prompt_tx.send(TaskResult::GithubAuthPrompt(prompt));
                            },
                        ))
                    }
                    WorkerTask::CreateGithubRepo(request) => {
                        TaskResult::CreateGithubRepo(crate::git_ops::create_github_repo(&request))
                    }
                    WorkerTask::OpenPullRequest(url) => {
                        TaskResult::OpenPullRequest(crate::git_ops::open_pull_request(&url))
                    }
                    WorkerTask::CreatePullRequest(url) => {
                        TaskResult::CreatePullRequest(crate::git_ops::create_pull_request(&url))
                    }
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

    pub fn push(&self, repo_path: PathBuf, auth: Option<crate::git_ops::GithubAuthSession>) {
        if !self.is_busy() {
            let _ = self.tx.send(WorkerTask::Push(repo_path, auth));
        }
    }

    pub fn pull(&self, repo_path: PathBuf, auth: Option<crate::git_ops::GithubAuthSession>) {
        if !self.is_busy() {
            let _ = self.tx.send(WorkerTask::Pull(repo_path, auth));
        }
    }

    pub fn create_tag(
        &self,
        repo_path: PathBuf,
        tag_name: String,
        auth: Option<crate::git_ops::GithubAuthSession>,
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

    pub fn create_github_repo(&self, request: crate::git_ops::CreateGithubRepoRequest) {
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

    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    pub fn try_recv(&self) -> Option<TaskResult> {
        self.rx.try_recv().ok()
    }
}
