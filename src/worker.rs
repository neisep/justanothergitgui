use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

pub enum TaskResult {
    Push(Result<String, String>),
    Pull(Result<String, String>),
    GithubAuth(Result<String, String>),
    CreateGithubRepo(Result<crate::git_ops::CreateGithubRepoSuccess, String>),
}

enum WorkerTask {
    Push(PathBuf),
    Pull(PathBuf),
    GithubAuth,
    CreateGithubRepo(crate::git_ops::CreateGithubRepoRequest),
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
                    WorkerTask::Push(path) => TaskResult::Push(crate::git_ops::push(&path)),
                    WorkerTask::Pull(path) => TaskResult::Pull(crate::git_ops::pull(&path)),
                    WorkerTask::GithubAuth => {
                        TaskResult::GithubAuth(crate::git_ops::github_auth_login())
                    }
                    WorkerTask::CreateGithubRepo(request) => {
                        TaskResult::CreateGithubRepo(crate::git_ops::create_github_repo(&request))
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

    pub fn push(&self, repo_path: PathBuf) {
        if !self.is_busy() {
            let _ = self.tx.send(WorkerTask::Push(repo_path));
        }
    }

    pub fn pull(&self, repo_path: PathBuf) {
        if !self.is_busy() {
            let _ = self.tx.send(WorkerTask::Pull(repo_path));
        }
    }

    pub fn login_github(&self) {
        if !self.is_busy() {
            let _ = self.tx.send(WorkerTask::GithubAuth);
        }
    }

    pub fn create_github_repo(&self, request: crate::git_ops::CreateGithubRepoRequest) {
        if !self.is_busy() {
            let _ = self.tx.send(WorkerTask::CreateGithubRepo(request));
        }
    }

    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    pub fn try_recv(&self) -> Option<TaskResult> {
        self.rx.try_recv().ok()
    }
}
