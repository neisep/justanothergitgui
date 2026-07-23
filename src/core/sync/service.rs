use std::path::Path;

use crate::core::ports::{
    GitBranchReadPort, GitHubRemoteInfoPort, GitRemoteAuth, GitRemoteSyncPort, GitUndoCommitPort,
};
use crate::shared::github::{GithubAuthSession, PushSuccess};

pub fn push(
    repo_path: &Path,
    auth: Option<&GithubAuthSession>,
    git: &(impl GitBranchReadPort + GitRemoteSyncPort),
    github: &impl GitHubRemoteInfoPort,
) -> Result<PushSuccess, String> {
    let branch_name = git.current_branch_name(repo_path)?;
    let base_message = if let Some(message) =
        try_push_with_auth(repo_path, branch_name.as_deref(), auth, git, github)?
    {
        message
    } else {
        let branch_name = branch_name
            .as_deref()
            .ok_or_else(|| "Push requires a checked-out branch.".to_string())?;
        git.push(repo_path, branch_name, GitRemoteAuth::System)?
    };

    let mut message = base_message;
    let pull_request_prompt = match branch_name.as_deref() {
        Some(branch) => match github.detect_pull_request_prompt(repo_path, branch, auth) {
            Ok(prompt) => prompt,
            Err(error) => {
                message.push_str(&format!(" PR check unavailable: {}", error));
                None
            }
        },
        None => None,
    };

    Ok(PushSuccess {
        message,
        pull_request_prompt,
    })
}

pub fn pull(
    repo_path: &Path,
    auth: Option<&GithubAuthSession>,
    git: &(impl GitBranchReadPort + GitRemoteSyncPort),
    github: &impl GitHubRemoteInfoPort,
) -> Result<String, String> {
    if github.is_github_https_origin(repo_path) {
        let branch_name = git
            .current_branch_name(repo_path)?
            .ok_or_else(|| "GitHub pull requires a checked-out branch.".to_string())?;
        let auth = auth.ok_or_else(|| {
            "GitHub pull requires the app's GitHub sign-in. Use 'Sign in to GitHub...' and try again."
                .to_string()
        })?;

        return git.pull(repo_path, &branch_name, GitRemoteAuth::GitHub(auth));
    }

    let branch_name = git
        .current_branch_name(repo_path)?
        .ok_or_else(|| "Pull requires a checked-out branch.".to_string())?;
    git.pull(repo_path, &branch_name, GitRemoteAuth::System)
}

pub fn discard_and_reset_to_remote(
    repo_path: &Path,
    auth: Option<&GithubAuthSession>,
    clean_untracked: bool,
    git: &impl GitRemoteSyncPort,
    github: &impl GitHubRemoteInfoPort,
) -> Result<String, String> {
    let remote_auth = if github.is_github_https_origin(repo_path) {
        let auth = auth.ok_or_else(|| {
            "Reset requires the app's GitHub sign-in. Use 'Sign in to GitHub...' and try again."
                .to_string()
        })?;
        GitRemoteAuth::GitHub(auth)
    } else {
        GitRemoteAuth::System
    };

    git.reset_to_remote(repo_path, remote_auth, clean_untracked)
}

pub fn undo_last_commit(
    repo_path: &Path,
    git: &(impl GitBranchReadPort + GitUndoCommitPort),
) -> Result<String, String> {
    let branch_name = git
        .current_branch_name(repo_path)?
        .ok_or_else(|| "Undo last commit requires a checked-out branch.".to_string())?;
    let outgoing_commit_count = git.outgoing_commit_count(repo_path)?;
    if outgoing_commit_count == 0 {
        return Err(format!(
            "Undo last commit requires at least one local-only commit on {}.",
            branch_name
        ));
    }

    git.undo_last_commit(repo_path)
}

pub(crate) fn try_push_with_auth(
    repo_path: &Path,
    branch_name: Option<&str>,
    auth: Option<&GithubAuthSession>,
    git: &impl GitRemoteSyncPort,
    github: &impl GitHubRemoteInfoPort,
) -> Result<Option<String>, String> {
    if !github.is_github_https_origin(repo_path) {
        return Ok(None);
    }

    let Some(branch_name) = branch_name else {
        return Err("GitHub push requires a checked-out branch.".into());
    };
    let Some(auth) = auth else {
        return Err(
            "GitHub push requires the app's GitHub sign-in. Use 'Sign in to GitHub...' and try again."
                .into(),
        );
    };

    git.push(repo_path, branch_name, GitRemoteAuth::GitHub(auth))?;
    Ok(Some("Push successful".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::shared::github::PullRequestPrompt;

    fn auth_mode(auth: &GitRemoteAuth<'_>) -> &'static str {
        match auth {
            GitRemoteAuth::GitHub(_) => "github",
            GitRemoteAuth::System => "system",
        }
    }

    #[derive(Default)]
    struct FakeGitPort {
        branch_name: Option<String>,
        outgoing_commit_count: usize,
        push_message: String,
        undo_message: String,
        pull_message: String,
        reset_message: String,
        push_calls: std::cell::RefCell<Vec<(PathBuf, String, &'static str)>>,
        undo_calls: std::cell::RefCell<Vec<PathBuf>>,
        pull_calls: std::cell::RefCell<Vec<(PathBuf, String, &'static str)>>,
        reset_calls: std::cell::RefCell<Vec<(PathBuf, &'static str, bool)>>,
    }

    impl GitBranchReadPort for FakeGitPort {
        fn current_branch_name(&self, _repo_path: &Path) -> Result<Option<String>, String> {
            Ok(self.branch_name.clone())
        }
    }

    impl GitRemoteSyncPort for FakeGitPort {
        fn push(
            &self,
            repo_path: &Path,
            branch_name: &str,
            auth: GitRemoteAuth<'_>,
        ) -> Result<String, String> {
            let mode = match auth {
                GitRemoteAuth::GitHub(_) => "github",
                GitRemoteAuth::System => "system",
            };
            self.push_calls.borrow_mut().push((
                repo_path.to_path_buf(),
                branch_name.to_string(),
                mode,
            ));
            Ok(self.push_message.clone())
        }

        fn pull(
            &self,
            repo_path: &Path,
            branch_name: &str,
            auth: GitRemoteAuth<'_>,
        ) -> Result<String, String> {
            self.pull_calls.borrow_mut().push((
                repo_path.to_path_buf(),
                branch_name.to_string(),
                auth_mode(&auth),
            ));
            Ok(self.pull_message.clone())
        }

        fn reset_to_remote(
            &self,
            repo_path: &Path,
            auth: GitRemoteAuth<'_>,
            clean_untracked: bool,
        ) -> Result<String, String> {
            self.reset_calls.borrow_mut().push((
                repo_path.to_path_buf(),
                auth_mode(&auth),
                clean_untracked,
            ));
            Ok(self.reset_message.clone())
        }
    }

    impl GitUndoCommitPort for FakeGitPort {
        fn outgoing_commit_count(&self, _repo_path: &Path) -> Result<usize, String> {
            Ok(self.outgoing_commit_count)
        }

        fn undo_last_commit(&self, repo_path: &Path) -> Result<String, String> {
            self.undo_calls.borrow_mut().push(repo_path.to_path_buf());
            Ok(self.undo_message.clone())
        }
    }

    struct FakeGitHubPort {
        https_origin: bool,
        prompt: Option<PullRequestPrompt>,
    }

    impl GitHubRemoteInfoPort for FakeGitHubPort {
        fn is_github_https_origin(&self, _repo_path: &Path) -> bool {
            self.https_origin
        }

        fn detect_pull_request_prompt(
            &self,
            _repo_path: &Path,
            _branch: &str,
            _auth: Option<&GithubAuthSession>,
        ) -> Result<Option<PullRequestPrompt>, String> {
            Ok(self.prompt.clone())
        }
    }

    #[test]
    fn push_uses_injected_ports_without_real_repo() {
        let git = FakeGitPort {
            branch_name: Some("feature/demo".into()),
            outgoing_commit_count: 0,
            push_message: "Push complete".into(),
            undo_message: String::new(),
            push_calls: std::cell::RefCell::new(Vec::new()),
            undo_calls: std::cell::RefCell::new(Vec::new()),
            ..Default::default()
        };
        let github = FakeGitHubPort {
            https_origin: false,
            prompt: Some(PullRequestPrompt::Create {
                branch: "feature/demo".into(),
                url: "https://example.com/pr".into(),
            }),
        };

        let result = push(Path::new("/virtual/repo"), None, &git, &github).expect("push");

        assert_eq!(result.message, "Push complete");
        assert!(matches!(
            result.pull_request_prompt,
            Some(PullRequestPrompt::Create { branch, .. }) if branch == "feature/demo"
        ));
        assert_eq!(
            git.push_calls.borrow().as_slice(),
            &[(
                PathBuf::from("/virtual/repo"),
                "feature/demo".into(),
                "system"
            )]
        );
    }

    #[test]
    fn github_https_push_requires_app_auth_without_repo_access() {
        let git = FakeGitPort {
            branch_name: Some("main".into()),
            outgoing_commit_count: 0,
            push_message: String::new(),
            undo_message: String::new(),
            push_calls: std::cell::RefCell::new(Vec::new()),
            undo_calls: std::cell::RefCell::new(Vec::new()),
            ..Default::default()
        };
        let github = FakeGitHubPort {
            https_origin: true,
            prompt: None,
        };

        let error = try_push_with_auth(
            Path::new("/virtual/repo"),
            Some("main"),
            None,
            &git,
            &github,
        )
        .expect_err("https GitHub remotes should require app auth");

        assert!(error.contains("GitHub push requires the app's GitHub sign-in"));
        assert!(git.push_calls.borrow().is_empty());
    }

    #[test]
    fn undo_last_commit_requires_checked_out_branch() {
        let git = FakeGitPort {
            branch_name: None,
            outgoing_commit_count: 1,
            push_message: String::new(),
            undo_message: "Undid commit".into(),
            push_calls: std::cell::RefCell::new(Vec::new()),
            undo_calls: std::cell::RefCell::new(Vec::new()),
            ..Default::default()
        };

        let error =
            undo_last_commit(Path::new("/virtual/repo"), &git).expect_err("branch is required");

        assert_eq!(error, "Undo last commit requires a checked-out branch.");
        assert!(git.undo_calls.borrow().is_empty());
    }

    #[test]
    fn undo_last_commit_requires_local_only_commit() {
        let git = FakeGitPort {
            branch_name: Some("main".into()),
            outgoing_commit_count: 0,
            push_message: String::new(),
            undo_message: "Undid commit".into(),
            push_calls: std::cell::RefCell::new(Vec::new()),
            undo_calls: std::cell::RefCell::new(Vec::new()),
            ..Default::default()
        };

        let error = undo_last_commit(Path::new("/virtual/repo"), &git)
            .expect_err("local-only commit should be required");

        assert_eq!(
            error,
            "Undo last commit requires at least one local-only commit on main."
        );
        assert!(git.undo_calls.borrow().is_empty());
    }

    #[test]
    fn undo_last_commit_uses_injected_port_when_commit_is_local_only() {
        let git = FakeGitPort {
            branch_name: Some("main".into()),
            outgoing_commit_count: 1,
            push_message: String::new(),
            undo_message: "Removed commit abc12345 from main and kept its changes staged".into(),
            push_calls: std::cell::RefCell::new(Vec::new()),
            undo_calls: std::cell::RefCell::new(Vec::new()),
            ..Default::default()
        };

        let result = undo_last_commit(Path::new("/virtual/repo"), &git).expect("undo");

        assert_eq!(
            result,
            "Removed commit abc12345 from main and kept its changes staged"
        );
        assert_eq!(
            git.undo_calls.borrow().as_slice(),
            &[PathBuf::from("/virtual/repo")]
        );
    }

    fn app_auth() -> GithubAuthSession {
        GithubAuthSession {
            access_token: "token".into(),
            login: "octocat".into(),
        }
    }

    #[test]
    fn pull_uses_system_auth_for_non_github_origin() {
        let git = FakeGitPort {
            branch_name: Some("main".into()),
            pull_message: "Pull complete".into(),
            ..Default::default()
        };
        let github = FakeGitHubPort {
            https_origin: false,
            prompt: None,
        };

        let message = pull(Path::new("/virtual/repo"), None, &git, &github).expect("pull");

        assert_eq!(message, "Pull complete");
        assert_eq!(
            git.pull_calls.borrow().as_slice(),
            &[(PathBuf::from("/virtual/repo"), "main".into(), "system")]
        );
    }

    #[test]
    fn pull_uses_github_auth_for_https_origin() {
        let git = FakeGitPort {
            branch_name: Some("main".into()),
            pull_message: "Pull complete".into(),
            ..Default::default()
        };
        let github = FakeGitHubPort {
            https_origin: true,
            prompt: None,
        };
        let auth = app_auth();

        let message = pull(Path::new("/virtual/repo"), Some(&auth), &git, &github).expect("pull");

        assert_eq!(message, "Pull complete");
        assert_eq!(
            git.pull_calls.borrow().as_slice(),
            &[(PathBuf::from("/virtual/repo"), "main".into(), "github")]
        );
    }

    #[test]
    fn pull_https_origin_requires_app_auth() {
        let git = FakeGitPort {
            branch_name: Some("main".into()),
            ..Default::default()
        };
        let github = FakeGitHubPort {
            https_origin: true,
            prompt: None,
        };

        let error = pull(Path::new("/virtual/repo"), None, &git, &github)
            .expect_err("https origin should require app auth");

        assert!(error.contains("GitHub pull requires the app's GitHub sign-in"));
        assert!(git.pull_calls.borrow().is_empty());
    }

    #[test]
    fn discard_and_reset_threads_clean_untracked_and_system_auth() {
        let git = FakeGitPort {
            reset_message: "Reset complete".into(),
            ..Default::default()
        };
        let github = FakeGitHubPort {
            https_origin: false,
            prompt: None,
        };

        let message = discard_and_reset_to_remote(
            Path::new("/virtual/repo"),
            None,
            true,
            &git,
            &github,
        )
        .expect("reset");

        assert_eq!(message, "Reset complete");
        assert_eq!(
            git.reset_calls.borrow().as_slice(),
            &[(PathBuf::from("/virtual/repo"), "system", true)]
        );
    }

    #[test]
    fn discard_and_reset_uses_github_auth_for_https_origin() {
        let git = FakeGitPort {
            reset_message: "Reset complete".into(),
            ..Default::default()
        };
        let github = FakeGitHubPort {
            https_origin: true,
            prompt: None,
        };
        let auth = app_auth();

        discard_and_reset_to_remote(
            Path::new("/virtual/repo"),
            Some(&auth),
            false,
            &git,
            &github,
        )
        .expect("reset");

        assert_eq!(
            git.reset_calls.borrow().as_slice(),
            &[(PathBuf::from("/virtual/repo"), "github", false)]
        );
    }

    #[test]
    fn discard_and_reset_https_origin_requires_app_auth() {
        let git = FakeGitPort::default();
        let github = FakeGitHubPort {
            https_origin: true,
            prompt: None,
        };

        let error =
            discard_and_reset_to_remote(Path::new("/virtual/repo"), None, false, &git, &github)
                .expect_err("https origin should require app auth");

        assert!(error.contains("Reset requires the app's GitHub sign-in"));
        assert!(git.reset_calls.borrow().is_empty());
    }
}
