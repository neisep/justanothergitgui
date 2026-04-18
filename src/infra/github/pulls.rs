use git2::Repository;
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::Deserialize;
use std::path::Path;

use crate::infra::github::auth::github_http_client;
use crate::shared::github::{GithubAuthSession, PullRequestPrompt};

#[derive(Deserialize)]
struct GithubPullRequest {
    number: u64,
    html_url: String,
}

#[derive(Deserialize)]
struct GithubRepoInfo {
    html_url: String,
    default_branch: String,
}

pub fn has_github_origin(repo: &Repository) -> bool {
    repo.find_remote("origin")
        .ok()
        .and_then(|remote| remote.url().and_then(parse_github_remote_slug))
        .is_some()
}

pub fn has_github_https_origin(repo: &Repository) -> bool {
    repo.find_remote("origin")
        .ok()
        .is_some_and(|remote| remote.url().is_some_and(is_github_https_url))
}

pub fn is_github_https_origin(repo_path: &Path) -> bool {
    let Ok(repo) = Repository::open(repo_path) else {
        return false;
    };
    let Ok(remote) = repo.find_remote("origin") else {
        return false;
    };
    remote.url().is_some_and(is_github_https_url)
}

pub fn is_github_https_url(url: &str) -> bool {
    url.starts_with("https://github.com/")
}

pub fn detect_pull_request_prompt(
    repo_path: &Path,
    branch: &str,
    auth: Option<&GithubAuthSession>,
) -> Result<Option<PullRequestPrompt>, String> {
    let Some(auth) = auth else {
        return Ok(None);
    };
    let Some((owner, repo)) = github_repo_slug(repo_path) else {
        return Ok(None);
    };

    let client = github_http_client()?;
    let pulls: Vec<GithubPullRequest> = client
        .get(format!(
            "https://api.github.com/repos/{owner}/{repo}/pulls?state=open&head={}%3A{}",
            urlencoding::encode(&owner),
            urlencoding::encode(branch)
        ))
        .header(AUTHORIZATION, format!("Bearer {}", auth.access_token))
        .header(ACCEPT, "application/vnd.github+json")
        .header(USER_AGENT, "justanothergitgui")
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("GitHub PR lookup failed: {}", error))?
        .json()
        .map_err(|error| format!("Invalid GitHub PR response: {}", error))?;

    if let Some(pr) = pulls.into_iter().next() {
        return Ok(Some(PullRequestPrompt::Open {
            branch: branch.to_string(),
            number: pr.number,
            url: pr.html_url,
        }));
    }

    let repo_info: GithubRepoInfo = client
        .get(format!("https://api.github.com/repos/{owner}/{repo}"))
        .header(AUTHORIZATION, format!("Bearer {}", auth.access_token))
        .header(ACCEPT, "application/vnd.github+json")
        .header(USER_AGENT, "justanothergitgui")
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("GitHub repository lookup failed: {}", error))?
        .json()
        .map_err(|error| format!("Invalid GitHub repository response: {}", error))?;

    Ok(Some(PullRequestPrompt::Create {
        branch: branch.to_string(),
        url: format!(
            "{}/compare/{}...{}?expand=1",
            repo_info.html_url,
            urlencoding::encode(&repo_info.default_branch),
            urlencoding::encode(branch)
        ),
    }))
}

pub(crate) fn parse_github_remote_slug(remote_url: &str) -> Option<(String, String)> {
    if let Some(rest) = remote_url.strip_prefix("https://github.com/") {
        return parse_github_slug(rest);
    }
    if let Some(rest) = remote_url.strip_prefix("http://github.com/") {
        return parse_github_slug(rest);
    }
    if let Some(rest) = remote_url.strip_prefix("git@github.com:") {
        return parse_github_slug(rest);
    }
    if let Some(rest) = remote_url.strip_prefix("ssh://git@github.com/") {
        return parse_github_slug(rest);
    }
    None
}

fn github_repo_slug(repo_path: &Path) -> Option<(String, String)> {
    let repo = Repository::open(repo_path).ok()?;
    let remote = repo.find_remote("origin").ok()?;
    let url = remote.url()?;
    parse_github_remote_slug(url)
}

fn parse_github_slug(slug: &str) -> Option<(String, String)> {
    let slug = slug.trim_end_matches(".git");
    let mut parts = slug.splitn(2, '/');
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim();
    if owner.is_empty() || repo.is_empty() {
        None
    } else {
        Some((owner.to_string(), repo.to_string()))
    }
}
