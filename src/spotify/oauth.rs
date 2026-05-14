use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::Rng;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{Tokens, REQUIRED_SCOPES};

const AUTH_ENDPOINT: &str = "https://accounts.spotify.com/authorize";
const TOKEN_ENDPOINT: &str = "https://accounts.spotify.com/api/token";

pub struct AuthSession {
    pub client_id: String,
    pub redirect_uri: String,
    pub state: String,
    pub code_verifier: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: Option<String>,
    expires_in: u64,
    refresh_token: Option<String>,
    scope: Option<String>,
}

pub fn authorize(client_id: &str, redirect_uri: &str) -> Result<(String, AuthSession)> {
    let verifier = random_verifier();
    let challenge = code_challenge(&verifier);
    let state = random_state();

    let scope = REQUIRED_SCOPES.join(" ");
    let url = format!(
        "{AUTH_ENDPOINT}?response_type=code&client_id={cid}&redirect_uri={ru}&code_challenge_method=S256&code_challenge={cc}&state={st}&scope={sc}",
        cid = urlencoding::encode(client_id),
        ru = urlencoding::encode(redirect_uri),
        cc = urlencoding::encode(&challenge),
        st = urlencoding::encode(&state),
        sc = urlencoding::encode(&scope),
    );

    Ok((
        url,
        AuthSession {
            client_id: client_id.to_string(),
            redirect_uri: redirect_uri.to_string(),
            state,
            code_verifier: verifier,
        },
    ))
}

pub fn exchange_code(session: &AuthSession, code: &str) -> Result<Tokens> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;
    let resp = client
        .post(TOKEN_ENDPOINT)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", session.redirect_uri.as_str()),
            ("client_id", session.client_id.as_str()),
            ("code_verifier", session.code_verifier.as_str()),
        ])
        .send()
        .context("calling Spotify token endpoint")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(anyhow!("token exchange failed ({status}): {body}"));
    }

    let body: TokenResponse = resp.json()?;
    Ok(tokens_from(body))
}

pub fn refresh_token(client_id: &str, refresh: &str) -> Result<Tokens> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;
    let resp = client
        .post(TOKEN_ENDPOINT)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh),
            ("client_id", client_id),
        ])
        .send()
        .context("calling Spotify token refresh endpoint")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(anyhow!("token refresh failed ({status}): {body}"));
    }

    let mut body: TokenResponse = resp.json()?;
    if body.refresh_token.is_none() {
        body.refresh_token = Some(refresh.to_string());
    }
    Ok(tokens_from(body))
}

fn tokens_from(r: TokenResponse) -> Tokens {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Tokens {
        access_token: r.access_token,
        refresh_token: r.refresh_token,
        expires_at_unix: now + r.expires_in,
        scope: r.scope,
        token_type: r.token_type,
    }
}

fn random_verifier() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..64).map(|_| rng.gen::<u8>()).collect();
    URL_SAFE_NO_PAD.encode(bytes)
}

fn code_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hasher.finalize())
}

fn random_state() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..16).map(|_| rng.gen::<u8>()).collect();
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn wait_for_redirect(port: u16, expected_state: &str) -> Result<String> {
    let server = tiny_http::Server::http(format!("127.0.0.1:{port}"))
        .map_err(|e| anyhow!("could not bind 127.0.0.1:{port}: {e}"))?;

    for req in server.incoming_requests() {
        let url = req.url().to_string();
        let query = url.splitn(2, '?').nth(1).unwrap_or("");
        let mut code = None::<String>;
        let mut state = None::<String>;
        let mut error = None::<String>;
        for pair in query.split('&') {
            let mut kv = pair.splitn(2, '=');
            let k = kv.next().unwrap_or("");
            let v = kv.next().unwrap_or("");
            let decoded = percent_decode(v);
            match k {
                "code" => code = Some(decoded),
                "state" => state = Some(decoded),
                "error" => error = Some(decoded),
                _ => {}
            }
        }

        let body = "<html><body><h2>Noctune</h2><p>You can close this tab.</p></body></html>";
        let resp = tiny_http::Response::from_string(body)
            .with_header("Content-Type: text/html; charset=utf-8".parse::<tiny_http::Header>().unwrap());
        let _ = req.respond(resp);

        if let Some(err) = error {
            return Err(anyhow!("spotify auth error: {err}"));
        }
        if state.as_deref() != Some(expected_state) {
            return Err(anyhow!("state mismatch — possible CSRF"));
        }
        if let Some(c) = code {
            return Ok(c);
        }
        return Err(anyhow!("redirect missing 'code' parameter"));
    }
    Err(anyhow!("redirect server closed before receiving request"))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for b in s.as_bytes() {
            match *b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(*b as char);
                }
                _ => out.push_str(&format!("%{:02X}", b)),
            }
        }
        out
    }
}
