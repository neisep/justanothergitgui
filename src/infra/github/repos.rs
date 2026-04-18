use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path};

use crate::infra::github::auth::github_http_client;
use crate::shared::github::{GithubAuthSession, GithubRepoSummary, GithubRepoVisibility};

const GITHUB_REPO_LIST_MAX_PAGES: usize = 5;

#[derive(Deserialize)]
struct GithubRepo {
    clone_url: String,
}

#[derive(Serialize)]
struct GithubCreateRepoBody<'a> {
    name: &'a str,
    private: bool,
}

pub fn list_github_repositories(
    auth: &GithubAuthSession,
) -> Result<Vec<GithubRepoSummary>, String> {
    let client = github_http_client()?;
    let mut results = Vec::new();
    let mut next_url = Some(
        "https://api.github.com/user/repos?per_page=100&sort=updated&affiliation=owner,collaborator,organization_member"
            .to_string(),
    );
    let mut pages = 0;

    while let Some(url) = next_url.take() {
        if pages >= GITHUB_REPO_LIST_MAX_PAGES {
            break;
        }
        pages += 1;

        let response = client
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {}", auth.access_token))
            .header(ACCEPT, "application/vnd.github+json")
            .header(USER_AGENT, "justanothergitgui")
            .send()
            .map_err(|error| format!("GitHub repo list failed: {}", error))?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(
                "GitHub token is no longer valid. Sign in again to refresh your session.".into(),
            );
        }
        if !status.is_success() {
            return Err(format!("GitHub repo list failed with status {}", status));
        }

        let link_header = response
            .headers()
            .get("link")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);

        let page: Vec<GithubRepoSummary> = response
            .json()
            .map_err(|error| format!("Invalid GitHub repo list response: {}", error))?;
        results.extend(page);

        next_url = link_header.as_deref().and_then(parse_link_header_next);
    }

    Ok(results)
}

pub fn create_repository(
    auth: &GithubAuthSession,
    repo_name: &str,
    visibility: GithubRepoVisibility,
) -> Result<String, String> {
    let client = github_http_client()?;
    let (owner, repo_name_only) = parse_target_repo_name(repo_name, &auth.login)?;
    let create_url = if owner == auth.login {
        "https://api.github.com/user/repos".to_string()
    } else {
        format!("https://api.github.com/orgs/{}/repos", owner)
    };

    let github_repo: GithubRepo = client
        .post(create_url)
        .header(AUTHORIZATION, format!("Bearer {}", auth.access_token))
        .header(ACCEPT, "application/vnd.github+json")
        .header(USER_AGENT, "justanothergitgui")
        .json(&GithubCreateRepoBody {
            name: &repo_name_only,
            private: visibility.is_private(),
        })
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("GitHub repository creation failed: {}", error))?
        .json()
        .map_err(|error| format!("Invalid GitHub repository response: {}", error))?;

    Ok(github_repo.clone_url)
}

pub fn repo_name_from_clone_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }

    let without_git = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    let segment = without_git
        .rsplit(|ch: char| ch == '/' || ch == ':')
        .find(|part| !part.is_empty())?;
    let segment = segment.trim();
    if segment.is_empty() || segment.contains('\\') || segment.contains('\0') {
        return None;
    }

    let path = Path::new(segment);
    let mut components = path.components();
    let Some(first) = components.next() else {
        return None;
    };
    if components.next().is_some() {
        return None;
    }

    match first {
        Component::Normal(os) if os == segment => Some(segment.to_string()),
        _ => None,
    }
}

pub(crate) fn parse_link_header_next(header: &str) -> Option<String> {
    for part in header.split(',') {
        let part = part.trim();
        let Some((target, params)) = part.split_once(';') else {
            continue;
        };
        let url = target.trim().trim_start_matches('<').trim_end_matches('>');
        if params
            .split(';')
            .any(|param| param.trim() == "rel=\"next\"" || param.trim() == "rel=next")
        {
            return Some(url.to_string());
        }
    }

    None
}

fn parse_target_repo_name(
    repo_name: &str,
    fallback_owner: &str,
) -> Result<(String, String), String> {
    if let Some((owner, name)) = repo_name.split_once('/') {
        let owner = owner.trim();
        let name = name.trim();
        if owner.is_empty() || name.is_empty() {
            return Err("Repository name must look like owner/name or name".into());
        }
        Ok((owner.to_string(), name.to_string()))
    } else {
        Ok((fallback_owner.to_string(), repo_name.trim().to_string()))
    }
}
