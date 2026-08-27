use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

use super::Author;

const GITHUB_DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GITHUB_USER_URL: &str = "https://api.github.com/user";

// Default client ID for Noctune Share OAuth application
pub const DEFAULT_GITHUB_CLIENT_ID: &str = "Ov23liau4NoctuneApp";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    #[allow(dead_code)]
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubUserResponse {
    id: u64,
    login: String,
    name: Option<String>,
}

pub fn request_device_code(client_id: &str) -> Result<DeviceCodeResponse> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let res = client
        .post(GITHUB_DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .form(&[("client_id", client_id), ("scope", "read:user")])
        .send()
        .context("Falha ao contatar GitHub Device Auth")?;

    if !res.status().is_success() {
        return Err(anyhow!("GitHub retornou status {}", res.status()));
    }

    let code_resp: DeviceCodeResponse = res.json()?;
    Ok(code_resp)
}

pub fn poll_access_token(
    client_id: &str,
    device_code: &str,
    interval_secs: u64,
    expires_in_secs: u64,
) -> Result<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let start = Instant::now();
    let max_dur = Duration::from_secs(expires_in_secs);
    let interval = Duration::from_secs(interval_secs.max(5));

    while start.elapsed() < max_dur {
        std::thread::sleep(interval);

        let res = client
            .post(GITHUB_TOKEN_URL)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", client_id),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send();

        if let Ok(resp) = res {
            if let Ok(token_data) = resp.json::<TokenResponse>() {
                if let Some(token) = token_data.access_token {
                    return Ok(token);
                }
                if let Some(err) = token_data.error {
                    if err == "authorization_pending" {
                        continue;
                    }
                    if err == "slow_down" {
                        std::thread::sleep(Duration::from_secs(5));
                        continue;
                    }
                    return Err(anyhow!("GitHub Auth erro: {err}"));
                }
            }
        }
    }
    Err(anyhow!("Tempo de autorização do GitHub expirou"))
}

pub fn get_github_user(token: &str) -> Result<Author> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let res = client
        .get(GITHUB_USER_URL)
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", "Noctune-Music-Player")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .context("Falha ao obter perfil do GitHub")?;

    if !res.status().is_success() {
        return Err(anyhow!("GitHub API retornou status {}", res.status()));
    }

    let user: GithubUserResponse = res.json()?;
    Ok(Author {
        id: user.id.to_string(),
        display_name: user.name.unwrap_or(user.login),
    })
}
