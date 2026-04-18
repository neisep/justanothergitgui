use std::path::Path;

use crate::core::{publish, sync, tags};
use crate::infra::system::browser;
use crate::shared::github::{
    CreateGithubRepoRequest, CreateGithubRepoSuccess, GithubAuthSession, PushSuccess,
};

pub use crate::infra::git::clone::{clone_repository, repo_name_from_clone_url};
pub use crate::infra::git::repository::{
    can_create_tag_on_branch, create_branch, delete_local_branch, get_branches, get_commit_history,
    get_current_branch, get_outgoing_commit_count, has_origin_remote, list_stale_branches,
    open_repo, preview_create_branch, preview_discard_damage, suggest_next_tag, switch_branch,
    validate_new_branch_name,
};
pub use crate::infra::git::worktree::{
    create_commit, get_file_diff, get_file_statuses, read_conflict_file, stage_all, stage_file,
    unstage_all, unstage_file, write_resolved_file,
};
pub use crate::infra::github::auth::{
    clear_github_auth_session, github_auth_login, load_github_auth_session,
    save_github_auth_session, verify_github_auth_session,
};
pub use crate::infra::github::pulls::{has_github_https_origin, has_github_origin};
pub use crate::infra::github::repos::list_github_repositories;

pub fn push(repo_path: &Path, auth: Option<&GithubAuthSession>) -> Result<PushSuccess, String> {
    sync::service::push(repo_path, auth)
}

pub fn pull(repo_path: &Path, auth: Option<&GithubAuthSession>) -> Result<String, String> {
    sync::service::pull(repo_path, auth)
}

pub fn discard_and_reset_to_remote(
    repo_path: &Path,
    auth: Option<&GithubAuthSession>,
    clean_untracked: bool,
) -> Result<String, String> {
    sync::service::discard_and_reset_to_remote(repo_path, auth, clean_untracked)
}

pub fn create_tag(
    repo_path: &Path,
    tag_name: &str,
    auth: Option<&GithubAuthSession>,
) -> Result<String, String> {
    tags::service::create_tag(repo_path, tag_name, auth)
}

pub fn create_github_repo(
    request: &CreateGithubRepoRequest,
) -> Result<CreateGithubRepoSuccess, String> {
    publish::service::create_github_repo(request)
}

pub fn open_pull_request(url: &str) -> Result<String, String> {
    browser::open_url(url, "pull request")?;
    Ok("Opened pull request in browser".into())
}

pub fn create_pull_request(url: &str) -> Result<String, String> {
    browser::open_url(url, "pull request creation page")?;
    Ok("Opened pull request creation in browser".into())
}

#[cfg(test)]
fn is_github_https_url(url: &str) -> bool {
    crate::infra::github::pulls::is_github_https_url(url)
}

#[cfg(test)]
fn parse_github_remote_slug(remote_url: &str) -> Option<(String, String)> {
    crate::infra::github::pulls::parse_github_remote_slug(remote_url)
}

#[cfg(test)]
fn parse_link_header_next(header: &str) -> Option<String> {
    crate::infra::github::repos::parse_link_header_next(header)
}

#[cfg(test)]
fn parse_semver_tag(name: &str) -> Option<([u32; 4], bool)> {
    crate::infra::git::repository::parse_semver_tag(name)
}

#[cfg(test)]
mod tests {
    use super::{
        is_github_https_url, parse_github_remote_slug, parse_link_header_next, parse_semver_tag,
        repo_name_from_clone_url, suggest_next_tag,
    };
    use git2::Repository;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestRepoDir {
        path: PathBuf,
    }

    impl TestRepoDir {
        fn init_with_origin(origin_url: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "justanothergitgui-git-ops-test-{}-{}",
                std::process::id(),
                unique
            ));
            std::fs::create_dir_all(&path).expect("create temp repo dir");
            let repo = Repository::init(&path).expect("init temp repo");
            repo.remote("origin", origin_url)
                .expect("add origin remote");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestRepoDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn treats_https_github_remotes_as_app_auth_candidates() {
        assert!(is_github_https_url(
            "https://github.com/octocat/hello-world.git"
        ));
    }

    #[test]
    fn keeps_github_ssh_remotes_on_system_auth_path() {
        assert_eq!(
            parse_github_remote_slug("git@github.com:octocat/hello-world.git"),
            Some(("octocat".into(), "hello-world".into()))
        );
        assert_eq!(
            parse_github_remote_slug("ssh://git@github.com/octocat/hello-world.git"),
            Some(("octocat".into(), "hello-world".into()))
        );
        assert!(!is_github_https_url(
            "git@github.com:octocat/hello-world.git"
        ));
        assert!(!is_github_https_url(
            "ssh://git@github.com/octocat/hello-world.git"
        ));
    }

    #[test]
    fn github_https_pushes_require_app_auth() {
        let repo_dir = TestRepoDir::init_with_origin("https://github.com/octocat/hello-world.git");
        let error =
            crate::core::sync::service::try_push_with_auth(repo_dir.path(), Some("main"), None)
                .expect_err("https GitHub remotes should require app auth");
        assert!(error.contains("GitHub push requires the app's GitHub sign-in"));
    }

    #[test]
    fn github_ssh_pushes_fall_back_to_system_auth() {
        let repo_dir = TestRepoDir::init_with_origin("git@github.com:octocat/hello-world.git");
        assert_eq!(
            crate::core::sync::service::try_push_with_auth(repo_dir.path(), Some("main"), None)
                .expect("ssh GitHub remotes should stay on system auth"),
            None
        );
    }

    #[test]
    fn parse_semver_tag_accepts_plain_and_v_prefixed() {
        assert_eq!(parse_semver_tag("v1.2.3.4"), Some(([1, 2, 3, 4], true)));
        assert_eq!(parse_semver_tag("0.10.4.7"), Some(([0, 10, 4, 7], false)));
    }

    #[test]
    fn parse_semver_tag_rejects_non_semver() {
        assert!(parse_semver_tag("v1.2").is_none());
        assert!(parse_semver_tag("v1.2.3").is_none());
        assert!(parse_semver_tag("v1.2.3.4.5").is_none());
        assert!(parse_semver_tag("release-5").is_none());
        assert!(parse_semver_tag("v1.2.3.4-rc1").is_none());
    }

    #[test]
    fn suggest_next_tag_bumps_patch_from_highest_existing_tag() {
        let repo_dir = TestRepoDir::init_with_origin("git@github.com:octocat/hello-world.git");
        let repo = Repository::open(repo_dir.path()).expect("open repo");

        let sig = git2::Signature::now("tester", "tester@example.com").expect("sig");
        let tree_id = {
            let mut index = repo.index().expect("index");
            index.write_tree().expect("write tree")
        };
        let tree = repo.find_tree(tree_id).expect("find tree");
        let commit_id = repo
            .commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .expect("commit");
        let commit = repo.find_commit(commit_id).expect("find commit");

        for tag in [
            "v0.9.0.0",
            "v1.2.3.5",
            "v1.2.0.0",
            "nightly-2024",
            "v2.0.0.0-rc1",
        ] {
            repo.tag_lightweight(tag, commit.as_object(), false)
                .expect("tag");
        }

        assert_eq!(suggest_next_tag(&repo), "v1.2.3.6");
    }

    #[test]
    fn suggest_next_tag_defaults_when_no_tags_exist() {
        let repo_dir = TestRepoDir::init_with_origin("git@github.com:octocat/hello-world.git");
        let repo = Repository::open(repo_dir.path()).expect("open repo");
        assert_eq!(suggest_next_tag(&repo), "v1.0.0.0");
    }

    #[test]
    fn suggest_next_tag_preserves_prefix_style_of_highest_tag() {
        let repo_dir = TestRepoDir::init_with_origin("git@github.com:octocat/hello-world.git");
        let repo = Repository::open(repo_dir.path()).expect("open repo");

        let sig = git2::Signature::now("tester", "tester@example.com").expect("sig");
        let tree_id = {
            let mut index = repo.index().expect("index");
            index.write_tree().expect("write tree")
        };
        let tree = repo.find_tree(tree_id).expect("find tree");
        let commit_id = repo
            .commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .expect("commit");
        let commit = repo.find_commit(commit_id).expect("find commit");

        for tag in ["v0.1.0.0", "3.4.5.2"] {
            repo.tag_lightweight(tag, commit.as_object(), false)
                .expect("tag");
        }

        assert_eq!(suggest_next_tag(&repo), "3.4.5.3");
    }

    #[test]
    fn repo_name_strips_git_suffix_and_picks_last_segment() {
        assert_eq!(
            repo_name_from_clone_url("https://github.com/octocat/Hello-World.git"),
            Some("Hello-World".to_string())
        );
        assert_eq!(
            repo_name_from_clone_url("https://github.com/octocat/Hello-World"),
            Some("Hello-World".to_string())
        );
        assert_eq!(
            repo_name_from_clone_url("git@github.com:octocat/Hello-World.git"),
            Some("Hello-World".to_string())
        );
        assert_eq!(
            repo_name_from_clone_url("  https://example.com/a/b/repo/  "),
            Some("repo".to_string())
        );
        assert_eq!(repo_name_from_clone_url(""), None);
        assert_eq!(repo_name_from_clone_url("   "), None);
    }

    #[test]
    fn repo_name_rejects_traversal_segments() {
        assert_eq!(repo_name_from_clone_url("https://example.com/a/."), None);
        assert_eq!(repo_name_from_clone_url("https://example.com/a/.."), None);
        assert_eq!(
            repo_name_from_clone_url("https://example.com/a/../b/."),
            None
        );
        assert_eq!(
            repo_name_from_clone_url("https://example.com/a/..git"),
            None
        );
        assert_eq!(repo_name_from_clone_url("/"), None);
        assert_eq!(
            repo_name_from_clone_url("https://example.com/a/foo\\bar"),
            None
        );
    }

    #[test]
    fn parse_link_header_finds_next_url() {
        let header = "<https://api.github.com/user/repos?page=2>; rel=\"next\", <https://api.github.com/user/repos?page=5>; rel=\"last\"";
        assert_eq!(
            parse_link_header_next(header).as_deref(),
            Some("https://api.github.com/user/repos?page=2")
        );

        let last_only = "<https://api.github.com/user/repos?page=5>; rel=\"last\"";
        assert_eq!(parse_link_header_next(last_only), None);
    }
}
