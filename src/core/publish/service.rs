use crate::core::ports::{
    GitBranchReadPort, GitHubRemoteInfoPort, GitHubRepoCreationPort, GitRemoteInfoPort,
    GitRemoteSyncPort, GitRepoBootstrapPort, GitWorktreeCommitPort,
};
use crate::core::sync::service as sync_service;
use crate::shared::github::{CreateGithubRepoRequest, CreateGithubRepoSuccess};

pub fn create_github_repo<G, H>(
    request: &CreateGithubRepoRequest,
    git: &G,
    github: &H,
) -> Result<CreateGithubRepoSuccess, String>
where
    G: GitRepoBootstrapPort
        + GitRemoteInfoPort
        + GitWorktreeCommitPort
        + GitBranchReadPort
        + GitRemoteSyncPort,
    H: GitHubRepoCreationPort + GitHubRemoteInfoPort,
{
    let repo_name = request.repo_name.trim();
    let commit_message = request.commit_message.trim();
    if repo_name.is_empty() {
        return Err("Repository name cannot be empty".into());
    }
    if commit_message.is_empty() {
        return Err("Initial commit message cannot be empty".into());
    }

    let folder_path = request
        .folder_path
        .canonicalize()
        .map_err(|error| format!("Invalid folder: {}", error))?;
    if !folder_path.is_dir() {
        return Err("Selected path is not a folder".into());
    }

    git.open_or_init_repo(&folder_path)?;
    if git.has_origin_remote(&folder_path)? {
        return Err("Remote 'origin' already exists for this repository".into());
    }

    let has_changes = git.repo_has_changes(&folder_path)?;
    let has_head = git.head_exists(&folder_path)?;
    if has_changes || !has_head {
        git.stage_all(&folder_path)?;
        git.create_commit(&folder_path, commit_message)?;
    }

    let clone_url = github.create_repository(&request.auth, repo_name, request.visibility)?;
    git.add_remote(&folder_path, "origin", &clone_url)?;
    let push_result = sync_service::push(&folder_path, Some(&request.auth), git, github)?;
    let message = format!(
        "Created GitHub repository {}. {}",
        repo_name, push_result.message
    );

    Ok(CreateGithubRepoSuccess {
        folder_path,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::create_github_repo;
    use crate::core::ports::{
        GitBranchReadPort, GitHubRemoteInfoPort, GitHubRepoCreationPort, GitRemoteAuth,
        GitRemoteInfoPort, GitRemoteSyncPort, GitRepoBootstrapPort, GitWorktreeCommitPort,
    };
    use crate::shared::github::{
        CreateGithubRepoRequest, GithubAuthSession, GithubRepoVisibility, PullRequestPrompt,
    };
    use crate::testutil::TestRepoDir;
    use std::cell::RefCell;
    use std::path::Path;

    #[derive(Default)]
    struct FakeGit {
        has_origin: bool,
        has_changes: bool,
        has_head: bool,
        branch_name: Option<String>,
        calls: RefCell<Vec<String>>,
    }

    impl FakeGit {
        fn record(&self, entry: String) {
            self.calls.borrow_mut().push(entry);
        }
    }

    impl GitRepoBootstrapPort for FakeGit {
        fn open_or_init_repo(&self, _repo_path: &Path) -> Result<(), String> {
            self.record("open_or_init".into());
            Ok(())
        }

        fn add_remote(&self, _repo_path: &Path, name: &str, url: &str) -> Result<(), String> {
            self.record(format!("add_remote:{}:{}", name, url));
            Ok(())
        }
    }

    impl GitRemoteInfoPort for FakeGit {
        fn has_origin_remote(&self, _repo_path: &Path) -> Result<bool, String> {
            Ok(self.has_origin)
        }
    }

    impl GitWorktreeCommitPort for FakeGit {
        fn repo_has_changes(&self, _repo_path: &Path) -> Result<bool, String> {
            Ok(self.has_changes)
        }

        fn head_exists(&self, _repo_path: &Path) -> Result<bool, String> {
            Ok(self.has_head)
        }

        fn stage_all(&self, _repo_path: &Path) -> Result<(), String> {
            self.record("stage_all".into());
            Ok(())
        }

        fn create_commit(&self, _repo_path: &Path, message: &str) -> Result<(), String> {
            self.record(format!("create_commit:{}", message));
            Ok(())
        }
    }

    impl GitBranchReadPort for FakeGit {
        fn current_branch_name(&self, _repo_path: &Path) -> Result<Option<String>, String> {
            Ok(self.branch_name.clone())
        }
    }

    impl GitRemoteSyncPort for FakeGit {
        fn push(
            &self,
            _repo_path: &Path,
            branch_name: &str,
            auth: GitRemoteAuth<'_>,
        ) -> Result<String, String> {
            let mode = match auth {
                GitRemoteAuth::GitHub(_) => "github",
                GitRemoteAuth::System => "system",
            };
            self.record(format!("push:{}:{}", branch_name, mode));
            Ok("Push complete".into())
        }

        fn pull(
            &self,
            _repo_path: &Path,
            _branch_name: &str,
            _auth: GitRemoteAuth<'_>,
        ) -> Result<String, String> {
            unreachable!()
        }

        fn reset_to_remote(
            &self,
            _repo_path: &Path,
            _auth: GitRemoteAuth<'_>,
            _clean_untracked: bool,
        ) -> Result<String, String> {
            unreachable!()
        }
    }

    struct FakeGitHub {
        clone_url: String,
    }

    impl GitHubRepoCreationPort for FakeGitHub {
        fn create_repository(
            &self,
            _auth: &GithubAuthSession,
            repo_name: &str,
            _visibility: GithubRepoVisibility,
        ) -> Result<String, String> {
            let _ = repo_name;
            Ok(self.clone_url.clone())
        }
    }

    impl GitHubRemoteInfoPort for FakeGitHub {
        fn is_github_https_origin(&self, _repo_path: &Path) -> bool {
            false
        }

        fn detect_pull_request_prompt(
            &self,
            _repo_path: &Path,
            _branch: &str,
            _auth: Option<&GithubAuthSession>,
        ) -> Result<Option<PullRequestPrompt>, String> {
            Ok(None)
        }
    }

    fn request(dir: &TestRepoDir, repo_name: &str, commit_message: &str) -> CreateGithubRepoRequest {
        CreateGithubRepoRequest {
            folder_path: dir.path().to_path_buf(),
            repo_name: repo_name.into(),
            commit_message: commit_message.into(),
            visibility: GithubRepoVisibility::Private,
            auth: GithubAuthSession {
                access_token: "token".into(),
                login: "octocat".into(),
            },
        }
    }

    #[test]
    fn rejects_empty_repo_name() {
        let dir = TestRepoDir::empty();
        let git = FakeGit::default();
        let github = FakeGitHub {
            clone_url: String::new(),
        };

        let error = create_github_repo(&request(&dir, "   ", "Initial commit"), &git, &github)
            .expect_err("empty repo name");

        assert_eq!(error, "Repository name cannot be empty");
        assert!(git.calls.borrow().is_empty());
    }

    #[test]
    fn rejects_empty_commit_message() {
        let dir = TestRepoDir::empty();
        let git = FakeGit::default();
        let github = FakeGitHub {
            clone_url: String::new(),
        };

        let error = create_github_repo(&request(&dir, "my-repo", "   "), &git, &github)
            .expect_err("empty commit message");

        assert_eq!(error, "Initial commit message cannot be empty");
        assert!(git.calls.borrow().is_empty());
    }

    #[test]
    fn rejects_when_origin_already_exists() {
        let dir = TestRepoDir::empty();
        let git = FakeGit {
            has_origin: true,
            ..Default::default()
        };
        let github = FakeGitHub {
            clone_url: String::new(),
        };

        let error = create_github_repo(&request(&dir, "my-repo", "Initial commit"), &git, &github)
            .expect_err("origin exists");

        assert_eq!(error, "Remote 'origin' already exists for this repository");
        // It opened/inited the repo but stopped before committing.
        assert_eq!(git.calls.borrow().as_slice(), &["open_or_init".to_string()]);
    }

    #[test]
    fn happy_path_commits_creates_remote_and_pushes_in_order() {
        let dir = TestRepoDir::empty();
        let git = FakeGit {
            has_origin: false,
            has_changes: true,
            has_head: false,
            branch_name: Some("main".into()),
            ..Default::default()
        };
        let github = FakeGitHub {
            clone_url: "https://example.com/my-repo.git".into(),
        };

        let success = create_github_repo(&request(&dir, "my-repo", "Initial commit"), &git, &github)
            .expect("publish");

        assert_eq!(
            success.message,
            "Created GitHub repository my-repo. Push complete"
        );
        assert_eq!(
            git.calls.borrow().as_slice(),
            &[
                "open_or_init".to_string(),
                "stage_all".to_string(),
                "create_commit:Initial commit".to_string(),
                "add_remote:origin:https://example.com/my-repo.git".to_string(),
                "push:main:system".to_string(),
            ]
        );
    }

    #[test]
    fn skips_commit_when_repo_already_has_head_and_no_changes() {
        let dir = TestRepoDir::empty();
        let git = FakeGit {
            has_origin: false,
            has_changes: false,
            has_head: true,
            branch_name: Some("main".into()),
            ..Default::default()
        };
        let github = FakeGitHub {
            clone_url: "https://example.com/my-repo.git".into(),
        };

        create_github_repo(&request(&dir, "my-repo", "Initial commit"), &git, &github)
            .expect("publish");

        let calls = git.calls.borrow();
        assert!(!calls.iter().any(|c| c == "stage_all"));
        assert!(!calls.iter().any(|c| c.starts_with("create_commit")));
        assert!(calls.iter().any(|c| c == "push:main:system"));
    }
}
