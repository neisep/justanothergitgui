use keyring::Error as KeyringError;
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::Deserialize;
use std::time::Duration;

use crate::infra::system::keychain;
use crate::shared::github::{GithubAuthCheck, GithubAuthPrompt, GithubAuthSession};

const GITHUB_AUTH_KEYRING_SERVICE: &str = "justanothergitgui";
const GITHUB_AUTH_KEYRING_USER: &str = "github-auth-session";

#[derive(Deserialize)]
struct GithubUser {
    login: String,
}

#[derive(Deserialize)]
struct GithubTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct GithubDeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    verification_uri_complete: Option<String>,
    expires_in: u64,
    interval: Option<u64>,
}

pub fn load_github_auth_session() -> Result<Option<GithubAuthSession>, String> {
    let entry = github_auth_keyring_entry()?;
    match entry.get_password() {
        Ok(payload) => serde_json::from_str(&payload)
            .map(Some)
            .map_err(|error| format!("Saved GitHub sign-in is invalid: {}", error)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(error) => Err(format!("Could not load saved GitHub sign-in: {}", error)),
    }
}

pub fn save_github_auth_session(session: &GithubAuthSession) -> Result<(), String> {
    let entry = github_auth_keyring_entry()?;
    let payload = serde_json::to_string(session)
        .map_err(|error| format!("Could not serialize GitHub sign-in: {}", error))?;
    entry.set_password(&payload).map_err(|error| {
        format!(
            "Could not save GitHub sign-in to system keychain: {}",
            error
        )
    })
}

pub fn clear_github_auth_session() -> Result<(), String> {
    let entry = github_auth_keyring_entry()?;
    match entry.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(error) => Err(format!("Could not clear saved GitHub sign-in: {}", error)),
    }
}

pub fn verify_github_auth_session(session: &GithubAuthSession) -> Result<GithubAuthCheck, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|error| format!("Could not create GitHub HTTP client: {}", error))?;

    let response = client
        .get("https://api.github.com/user")
        .header(AUTHORIZATION, format!("Bearer {}", session.access_token))
        .header(ACCEPT, "application/vnd.github+json")
        .header(USER_AGENT, "justanothergitgui")
        .send()
        .map_err(|error| format!("GitHub token check failed: {}", error))?;

    let status = response.status();
    if status.is_success() {
        Ok(GithubAuthCheck::Valid)
    } else if status == reqwest::StatusCode::UNAUTHORIZED {
        Ok(GithubAuthCheck::Revoked)
    } else {
        Err(format!("GitHub token check failed with status {}", status))
    }
}

pub fn github_auth_login<F>(client_id: &str, on_prompt: F) -> Result<GithubAuthSession, String>
where
    F: FnOnce(GithubAuthPrompt) -> Result<(), String>,
{
    let client = github_http_client()?;
    let device_response = client
        .post("https://github.com/login/device/code")
        .header(ACCEPT, "application/json")
        .header(USER_AGENT, "justanothergitgui")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "client_id={}&scope={}",
            urlencoding::encode(client_id),
            urlencoding::encode("repo workflow")
        ))
        .send()
        .map_err(|error| format!("GitHub device sign-in failed: {}", error))?;

    if !device_response.status().is_success() {
        return Err(format!(
            "GitHub device sign-in failed with status {}",
            device_response.status()
        ));
    }

    let device: GithubDeviceCodeResponse = device_response
        .json()
        .map_err(|error| format!("Invalid GitHub device sign-in response: {}", error))?;

    let open_url = device
        .verification_uri_complete
        .as_deref()
        .unwrap_or(&device.verification_uri);
    on_prompt(GithubAuthPrompt {
        user_code: device.user_code.clone(),
        verification_uri: device.verification_uri.clone(),
        browser_url: open_url.to_string(),
    })?;
    let _ = webbrowser::open(open_url);

    let mut poll_interval = device.interval.unwrap_or(5).max(1);
    let mut remaining_seconds = device.expires_in;

    while remaining_seconds > 0 {
        std::thread::sleep(Duration::from_secs(poll_interval));
        remaining_seconds = remaining_seconds.saturating_sub(poll_interval);

        let token_response = client
            .post("https://github.com/login/oauth/access_token")
            .header(ACCEPT, "application/json")
            .header(USER_AGENT, "justanothergitgui")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!(
                "client_id={}&device_code={}&grant_type={}",
                urlencoding::encode(client_id),
                urlencoding::encode(&device.device_code),
                urlencoding::encode("urn:ietf:params:oauth:grant-type:device_code")
            ))
            .send()
            .map_err(|error| format!("GitHub token exchange failed: {}", error))?;

        if !token_response.status().is_success() {
            return Err(format!(
                "GitHub token exchange failed with status {}",
                token_response.status()
            ));
        }

        let token_body: GithubTokenResponse = token_response
            .json()
            .map_err(|error| format!("Invalid GitHub token response: {}", error))?;

        if let Some(access_token) = token_body.access_token {
            return fetch_github_user(&client, &access_token);
        }

        match token_body.error.as_deref() {
            Some("authorization_pending") => continue,
            Some("slow_down") => {
                poll_interval += 5;
            }
            Some("access_denied") => return Err("GitHub sign-in was cancelled.".into()),
            Some("expired_token") => {
                return Err("GitHub sign-in timed out before authorization completed.".into());
            }
            _ => {
                let message = token_body
                    .error_description
                    .or(token_body.error)
                    .unwrap_or_else(|| "GitHub did not return an access token".into());
                return Err(normalize_github_oauth_error(message));
            }
        }
    }

    Err("GitHub sign-in timed out before authorization completed.".into())
}

pub(crate) fn github_http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| format!("Could not create GitHub HTTP client: {}", error))
}

fn github_auth_keyring_entry() -> Result<keyring::Entry, String> {
    keychain::entry(GITHUB_AUTH_KEYRING_SERVICE, GITHUB_AUTH_KEYRING_USER)
}

fn normalize_github_oauth_error(message: String) -> String {
    if message.contains("incorrect_client_credentials")
        || message.contains("client_id and/or client_secret passed are incorrect")
        || message.contains("client_id is invalid")
        || message.contains("device flow is disabled")
    {
        return "GitHub OAuth configuration error: the configured client ID is not valid for a GitHub OAuth App device flow. Use a GitHub OAuth App client ID and make sure Device Flow is enabled for that app.".into();
    }

    message
}

fn fetch_github_user(client: &Client, access_token: &str) -> Result<GithubAuthSession, String> {
    let user: GithubUser = client
        .get("https://api.github.com/user")
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .header(ACCEPT, "application/vnd.github+json")
        .header(USER_AGENT, "justanothergitgui")
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("GitHub user lookup failed: {}", error))?
        .json()
        .map_err(|error| format!("Invalid GitHub user response: {}", error))?;

    Ok(GithubAuthSession {
        access_token: access_token.to_string(),
        login: user.login,
    })
}
