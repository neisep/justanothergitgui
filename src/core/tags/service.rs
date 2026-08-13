use std::path::Path;

use crate::core::ports::{
    GitBranchReadPort, GitHubRemoteInfoPort, GitRemoteAuth, GitRemoteInfoPort, GitTagPort,
};
use crate::shared::github::GithubAuthSession;

pub fn create_tag(
    repo_path: &Path,
    tag_name: &str,
    auth: Option<&GithubAuthSession>,
    git: &(impl GitBranchReadPort + GitTagPort + GitRemoteInfoPort),
    github: &impl GitHubRemoteInfoPort,
) -> Result<String, String> {
    let tag_name = tag_name.trim();
    if tag_name.is_empty() {
        return Err("Tag name cannot be empty.".into());
    }

    let branch_name = git
        .current_branch_name(repo_path)?
        .ok_or_else(|| "Tag creation requires a checked-out branch.".to_string())?;
    if !git.can_create_tag_on_branch(&branch_name) {
        return Err("Tags can only be created from the main or master branch.".into());
    }

    git.create_tag(repo_path, tag_name)?;

    if !git.has_origin_remote(repo_path)? {
        return Ok(format!("Created local tag {}", tag_name));
    }

    match push_tag(repo_path, tag_name, auth, git, github) {
        Ok(()) => Ok(format!("Created and pushed tag {}", tag_name)),
        Err(error) => match git.rollback_tag(repo_path, tag_name) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(format!(
                "{} Local tag rollback also failed: {}",
                error, rollback_error
            )),
        },
    }
}

fn push_tag(
    repo_path: &Path,
    tag_name: &str,
    auth: Option<&GithubAuthSession>,
    git: &impl GitTagPort,
    github: &impl GitHubRemoteInfoPort,
) -> Result<(), String> {
    if github.is_github_https_origin(repo_path) {
        let auth = auth.ok_or_else(|| {
            "GitHub tag creation requires the app's GitHub sign-in. Use 'Sign in to GitHub...' and try again."
                .to_string()
        })?;
        return git.push_tag(repo_path, tag_name, GitRemoteAuth::GitHub(auth));
    }

    git.push_tag(repo_path, tag_name, GitRemoteAuth::System)
}

#[cfg(test)]
mod tests {
    use super::create_tag;
    use crate::core::ports::{
        GitBranchReadPort, GitHubRemoteInfoPort, GitRemoteAuth, GitRemoteInfoPort, GitTagPort,
    };
    use crate::shared::github::{GithubAuthSession, PullRequestPrompt};
    use std::cell::RefCell;
    use std::path::Path;

    #[derive(Default)]
    struct FakeTagGit {
        branch_name: Option<String>,
        can_tag: bool,
        has_origin: bool,
        push_tag_err: Option<String>,
        rollback_err: Option<String>,
        created: RefCell<Vec<String>>,
        pushed: RefCell<Vec<(String, &'static str)>>,
        rolled_back: RefCell<Vec<String>>,
    }

    impl GitBranchReadPort for FakeTagGit {
        fn current_branch_name(&self, _repo_path: &Path) -> Result<Option<String>, String> {
            Ok(self.branch_name.clone())
        }
    }

    impl GitRemoteInfoPort for FakeTagGit {
        fn has_origin_remote(&self, _repo_path: &Path) -> Result<bool, String> {
            Ok(self.has_origin)
        }
    }

    impl GitTagPort for FakeTagGit {
        fn can_create_tag_on_branch(&self, _branch_name: &str) -> bool {
            self.can_tag
        }

        fn create_tag(&self, _repo_path: &Path, tag_name: &str) -> Result<(), String> {
            self.created.borrow_mut().push(tag_name.to_string());
            Ok(())
        }

        fn push_tag(
            &self,
            _repo_path: &Path,
            tag_name: &str,
            auth: GitRemoteAuth<'_>,
        ) -> Result<(), String> {
            let mode = match auth {
                GitRemoteAuth::GitHub(_) => "github",
                GitRemoteAuth::System => "system",
            };
            self.pushed.borrow_mut().push((tag_name.to_string(), mode));
            match &self.push_tag_err {
                Some(error) => Err(error.clone()),
                None => Ok(()),
            }
        }

        fn rollback_tag(&self, _repo_path: &Path, tag_name: &str) -> Result<(), String> {
            self.rolled_back.borrow_mut().push(tag_name.to_string());
            match &self.rollback_err {
                Some(error) => Err(error.clone()),
                None => Ok(()),
            }
        }
    }

    struct FakeGitHub {
        https_origin: bool,
    }

    impl GitHubRemoteInfoPort for FakeGitHub {
        fn is_github_https_origin(&self, _repo_path: &Path) -> bool {
            self.https_origin
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

    fn app_auth() -> GithubAuthSession {
        GithubAuthSession {
            access_token: "token".into(),
            login: "octocat".into(),
        }
    }

    fn path() -> &'static Path {
        Path::new("/virtual/repo")
    }

    #[test]
    fn rejects_empty_tag_name() {
        let git = FakeTagGit::default();
        let github = FakeGitHub {
            https_origin: false,
        };

        let error = create_tag(path(), "   ", None, &git, &github).expect_err("empty tag");

        assert_eq!(error, "Tag name cannot be empty.");
        assert!(git.created.borrow().is_empty());
    }

    #[test]
    fn requires_checked_out_branch() {
        let git = FakeTagGit {
            branch_name: None,
            ..Default::default()
        };
        let github = FakeGitHub {
            https_origin: false,
        };

        let error = create_tag(path(), "v1.0.0", None, &git, &github).expect_err("no branch");

        assert_eq!(error, "Tag creation requires a checked-out branch.");
        assert!(git.created.borrow().is_empty());
    }

    #[test]
    fn rejects_disallowed_branch() {
        let git = FakeTagGit {
            branch_name: Some("feature".into()),
            can_tag: false,
            ..Default::default()
        };
        let github = FakeGitHub {
            https_origin: false,
        };

        let error =
            create_tag(path(), "v1.0.0", None, &git, &github).expect_err("disallowed branch");

        assert_eq!(
            error,
            "Tags can only be created from the main or master branch."
        );
        assert!(git.created.borrow().is_empty());
    }

    #[test]
    fn creates_local_tag_when_no_origin() {
        let git = FakeTagGit {
            branch_name: Some("main".into()),
            can_tag: true,
            has_origin: false,
            ..Default::default()
        };
        let github = FakeGitHub {
            https_origin: false,
        };

        let message = create_tag(path(), "v1.0.0", None, &git, &github).expect("local tag");

        assert_eq!(message, "Created local tag v1.0.0");
        assert_eq!(git.created.borrow().as_slice(), &["v1.0.0".to_string()]);
        assert!(git.pushed.borrow().is_empty());
    }

    #[test]
    fn creates_and_pushes_tag_over_system_auth() {
        let git = FakeTagGit {
            branch_name: Some("main".into()),
            can_tag: true,
            has_origin: true,
            ..Default::default()
        };
        let github = FakeGitHub {
            https_origin: false,
        };

        let message = create_tag(path(), "v1.0.0", None, &git, &github).expect("push tag");

        assert_eq!(message, "Created and pushed tag v1.0.0");
        assert_eq!(
            git.pushed.borrow().as_slice(),
            &[("v1.0.0".to_string(), "system")]
        );
        assert!(git.rolled_back.borrow().is_empty());
    }

    #[test]
    fn pushes_tag_with_app_auth_for_https_origin() {
        let git = FakeTagGit {
            branch_name: Some("main".into()),
            can_tag: true,
            has_origin: true,
            ..Default::default()
        };
        let github = FakeGitHub { https_origin: true };
        let auth = app_auth();

        let message = create_tag(path(), "v1.0.0", Some(&auth), &git, &github).expect("push tag");

        assert_eq!(message, "Created and pushed tag v1.0.0");
        assert_eq!(
            git.pushed.borrow().as_slice(),
            &[("v1.0.0".to_string(), "github")]
        );
    }

    #[test]
    fn https_origin_push_requires_app_auth_and_rolls_back() {
        let git = FakeTagGit {
            branch_name: Some("main".into()),
            can_tag: true,
            has_origin: true,
            ..Default::default()
        };
        let github = FakeGitHub { https_origin: true };

        let error = create_tag(path(), "v1.0.0", None, &git, &github).expect_err("auth required");

        assert!(error.contains("GitHub tag creation requires the app's GitHub sign-in"));
        assert!(git.pushed.borrow().is_empty(), "push not attempted");
        assert_eq!(git.rolled_back.borrow().as_slice(), &["v1.0.0".to_string()]);
    }

    #[test]
    fn push_failure_triggers_rollback_and_returns_original_error() {
        let git = FakeTagGit {
            branch_name: Some("main".into()),
            can_tag: true,
            has_origin: true,
            push_tag_err: Some("push exploded".into()),
            ..Default::default()
        };
        let github = FakeGitHub {
            https_origin: false,
        };

        let error = create_tag(path(), "v1.0.0", None, &git, &github).expect_err("push failed");

        assert_eq!(error, "push exploded");
        assert_eq!(git.rolled_back.borrow().as_slice(), &["v1.0.0".to_string()]);
    }

    #[test]
    fn push_and_rollback_failure_combines_messages() {
        let git = FakeTagGit {
            branch_name: Some("main".into()),
            can_tag: true,
            has_origin: true,
            push_tag_err: Some("push exploded".into()),
            rollback_err: Some("rollback exploded".into()),
            ..Default::default()
        };
        let github = FakeGitHub {
            https_origin: false,
        };

        let error = create_tag(path(), "v1.0.0", None, &git, &github).expect_err("both failed");

        assert_eq!(
            error,
            "push exploded Local tag rollback also failed: rollback exploded"
        );
    }
}
