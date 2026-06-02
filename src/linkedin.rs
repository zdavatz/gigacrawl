//! LinkedIn auth + image publishing for gigacrawl.
//!
//! Uses gigacrawl's OWN app credentials so it does not share / rotate tokens
//! with other projects. Looks up `linkedin_credentials.json` and
//! `linkedin_token.json` in the current directory, then `$HOME`.
//!
//! First-time setup:
//!   1. Create a LinkedIn developer app (linkedin.com/developers) and add the
//!      products "Sign In with LinkedIn using OpenID Connect" and
//!      "Share on LinkedIn". Add redirect URL: http://localhost:8092/callback
//!   2. Write linkedin_credentials.json:  {"client_id":"...","client_secret":"..."}
//!   3. Run:  cargo run --release --bin datacenter_chart -- --auth
//!      (opens a browser, captures the code, writes linkedin_token.json)
//!
//! Then post the generated chart with:
//!   cargo run --release --bin datacenter_chart -- --post-linkedin

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

const LINKEDIN_VERSION: &str = "202603";
const REDIRECT_URI: &str = "http://localhost:8092/callback";

#[derive(Serialize, Deserialize)]
struct Credentials {
    client_id: String,
    client_secret: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct Token {
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    person_id: String,
    #[serde(default)]
    expires_in: u64,
}

fn find_file(name: &str) -> Option<PathBuf> {
    let cwd = PathBuf::from(name);
    if cwd.exists() {
        return Some(cwd);
    }
    if let Some(home) = std::env::var_os("HOME") {
        let p = PathBuf::from(home).join(name);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn load_credentials() -> Result<(PathBuf, Credentials), Box<dyn Error>> {
    let path = find_file("linkedin_credentials.json").ok_or(
        "linkedin_credentials.json not found (looked in cwd and $HOME). \
         Create it with {\"client_id\":\"...\",\"client_secret\":\"...\"}",
    )?;
    let creds = serde_json::from_str(&fs::read_to_string(&path)?)?;
    Ok((path, creds))
}

fn load_token() -> Result<(PathBuf, Token), Box<dyn Error>> {
    let path = find_file("linkedin_token.json")
        .ok_or("linkedin_token.json not found. Run: datacenter_chart -- --auth")?;
    let token: Token = serde_json::from_str(&fs::read_to_string(&path)?)?;
    Ok((path, token))
}

fn save_token(path: &Path, token: &Token) -> Result<(), Box<dyn Error>> {
    fs::write(path, serde_json::to_string_pretty(token)?)?;
    Ok(())
}

fn refresh_token(
    client: &reqwest::blocking::Client,
    creds: &Credentials,
    token: &Token,
    token_path: &Path,
) -> Token {
    if token.refresh_token.is_empty() {
        return token.clone();
    }
    eprintln!("[linkedin] Refreshing access token...");
    let body = format!(
        "grant_type=refresh_token&refresh_token={}&client_id={}&client_secret={}",
        token.refresh_token, creds.client_id, creds.client_secret
    );
    let resp = client
        .post("https://www.linkedin.com/oauth/v2/accessToken")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send();
    let Ok(resp) = resp else {
        eprintln!("[linkedin] Refresh request failed; using existing token");
        return token.clone();
    };
    let Ok(text) = resp.text() else { return token.clone() };
    let Ok(data): Result<serde_json::Value, _> = serde_json::from_str(&text) else {
        return token.clone();
    };
    let Some(at) = data["access_token"].as_str().filter(|s| !s.is_empty()) else {
        eprintln!("[linkedin] Refresh response had no access_token; using existing");
        return token.clone();
    };
    let new_token = Token {
        access_token: at.to_string(),
        refresh_token: data["refresh_token"]
            .as_str()
            .unwrap_or(&token.refresh_token)
            .to_string(),
        person_id: token.person_id.clone(),
        expires_in: data["expires_in"].as_u64().unwrap_or(0),
    };
    if let Err(e) = save_token(token_path, &new_token) {
        eprintln!("[linkedin] Could not persist refreshed token: {}", e);
    } else {
        eprintln!("[linkedin] Token refreshed");
    }
    new_token
}

fn decode_person_id_from_jwt(id_token: &str) -> Option<String> {
    use base64::Engine;
    let payload = id_token.split('.').nth(1)?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    claims["sub"].as_str().map(|s| s.to_string())
}

/// Run the OAuth authorization-code flow and persist `linkedin_token.json`
/// next to the credentials file.
pub fn authenticate() -> Result<(), Box<dyn Error>> {
    let (creds_path, creds) = load_credentials()?;
    eprintln!("[linkedin] Using credentials: {}", creds_path.display());

    let auth_url = format!(
        "https://www.linkedin.com/oauth/v2/authorization?response_type=code\
         &client_id={}&redirect_uri={}&scope=openid%20profile%20w_member_social",
        creds.client_id,
        urlencoding::encode(REDIRECT_URI)
    );
    eprintln!("[linkedin] Opening browser for authorization...");
    eprintln!("If it does not open, visit:\n{}\n", auth_url);
    let _ = open::that(&auth_url);

    let listener = TcpListener::bind("127.0.0.1:8092")
        .map_err(|e| format!("cannot bind 127.0.0.1:8092 ({e}); free the port and retry"))?;
    eprintln!("[linkedin] Waiting for callback on {} ...", REDIRECT_URI);
    let (mut socket, _) = listener.accept()?;
    let mut buf = [0u8; 8192];
    let n = socket.read(&mut buf)?;
    let request = String::from_utf8_lossy(&buf[..n]);

    let code = request
        .split('?')
        .nth(1)
        .and_then(|q| q.split(' ').next())
        .unwrap_or("")
        .split('&')
        .find_map(|p| p.strip_prefix("code="))
        .map(|c| urlencoding::decode(c).map(|s| s.into_owned()).unwrap_or_else(|_| c.to_string()))
        .unwrap_or_default();

    let _ = socket.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
          <h1>gigacrawl: LinkedIn authorized.</h1><p>You can close this window.</p>",
    );
    if code.is_empty() {
        return Err("no authorization code received in callback".into());
    }
    eprintln!("[linkedin] Code received; exchanging for token...");

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    let body = format!(
        "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&client_secret={}",
        urlencoding::encode(&code),
        urlencoding::encode(REDIRECT_URI),
        urlencoding::encode(&creds.client_id),
        urlencoding::encode(&creds.client_secret),
    );
    let resp = client
        .post("https://www.linkedin.com/oauth/v2/accessToken")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()?;
    let status = resp.status();
    let text = resp.text()?;
    if !status.is_success() {
        return Err(format!("token exchange failed ({status}): {text}").into());
    }
    let data: serde_json::Value = serde_json::from_str(&text)?;
    let access_token = data["access_token"].as_str().unwrap_or("").to_string();
    if access_token.is_empty() {
        return Err(format!("no access_token in response: {text}").into());
    }
    let refresh = data["refresh_token"].as_str().unwrap_or("").to_string();
    let expires_in = data["expires_in"].as_u64().unwrap_or(0);

    // person_id: prefer id_token JWT, fall back to /v2/userinfo.
    let mut person_id = data["id_token"]
        .as_str()
        .and_then(decode_person_id_from_jwt)
        .unwrap_or_default();
    if person_id.is_empty() {
        if let Ok(r) = client
            .get("https://api.linkedin.com/v2/userinfo")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
        {
            if let Ok(v) = r.json::<serde_json::Value>() {
                if let Some(sub) = v["sub"].as_str() {
                    person_id = sub.to_string();
                }
            }
        }
    }
    if person_id.is_empty() {
        return Err("could not determine person_id (need openid+profile scope)".into());
    }

    let token = Token { access_token, refresh_token: refresh, person_id: person_id.clone(), expires_in };
    let token_path = creds_path.with_file_name("linkedin_token.json");
    save_token(&token_path, &token)?;
    eprintln!("[linkedin] Token saved to {}", token_path.display());
    eprintln!("[linkedin] person_id: {} · expires_in: {}s", person_id, expires_in);
    Ok(())
}

/// LinkedIn's "Little Text" format truncates at the first unescaped control
/// char — escape before sending the commentary.
fn escape_little_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(
            c,
            '(' | ')' | '<' | '>' | '@' | '|' | '{' | '}' | '[' | ']' | '*' | '_' | '~' | '\\'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn caption() -> String {
    "Data center power capacity (GW) — operational vs. planned, with FY2025 capex from SEC 10-K filings.\n\
     \n\
     Compared: Amazon (AWS ~10–15+ GW, $128.3B capex), Microsoft (Azure ~5–8+ GW, $64.6B), Google ($91.4B), Meta ($69.7B), xAI (~2 GW, Colossus), OpenAI (Stargate ~10 GW target), Anthropic ($50B plan).\n\
     \n\
     Each row links to its source: public companies to their FY2025 10-K on sec.gov; private to their announcements. Capex/PP&E from SEC EDGAR; GW figures are press/analyst-sourced (filings don't state capacity in gigawatts).\n\
     \n\
     Clickable PDF (each figure links to its filing): github.com/zdavatz/gigacrawl/blob/main/pdf/datacenter_sources.pdf\n\
     Code & sources: github.com/zdavatz/gigacrawl\n\
     #AI #DataCenters #CapEx #CloudInfrastructure #SEC"
        .to_string()
}

/// Upload `png_path` to LinkedIn as a public image post.
pub fn publish_image(png_path: &Path) -> Result<String, Box<dyn Error>> {
    let (creds_path, creds) = load_credentials()?;
    eprintln!("[linkedin] Using credentials: {}", creds_path.display());
    let (token_path, token) = load_token()?;
    if token.person_id.is_empty() {
        return Err("linkedin_token.json has empty person_id (run --auth)".into());
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    let token = refresh_token(&client, &creds, &token, &token_path);

    let owner = format!("urn:li:person:{}", token.person_id);
    let auth = format!("Bearer {}", token.access_token);
    let bytes = fs::read(png_path)?;
    eprintln!(
        "[linkedin] Uploading {} ({:.1} KB)",
        png_path.display(),
        bytes.len() as f64 / 1024.0
    );

    // Step 1 — initialize image upload
    let init_resp = client
        .post("https://api.linkedin.com/rest/images?action=initializeUpload")
        .header("Authorization", &auth)
        .header("Content-Type", "application/json")
        .header("LinkedIn-Version", LINKEDIN_VERSION)
        .header("X-Restli-Protocol-Version", "2.0.0")
        .json(&serde_json::json!({ "initializeUploadRequest": { "owner": owner } }))
        .send()?;
    let status = init_resp.status();
    let text = init_resp.text()?;
    if !status.is_success() {
        return Err(format!("initializeUpload failed ({status}): {text}").into());
    }
    let init: serde_json::Value = serde_json::from_str(&text)?;
    let value = &init["value"];
    let image_urn = value["image"]
        .as_str()
        .ok_or_else(|| format!("no image URN in response: {text}"))?
        .to_string();
    let upload_url = value["uploadUrl"]
        .as_str()
        .ok_or_else(|| format!("no uploadUrl in response: {text}"))?
        .to_string();

    // Step 2 — PUT the bytes
    let put_resp = client
        .put(&upload_url)
        .header("Authorization", &auth)
        .header("Content-Type", "application/octet-stream")
        .body(bytes)
        .send()?;
    if !put_resp.status().is_success() {
        let s = put_resp.status();
        return Err(format!("image PUT failed ({s}): {}", put_resp.text().unwrap_or_default()).into());
    }
    eprintln!("[linkedin] Image uploaded: {}", image_urn);

    // Step 3 — create the post
    let post_body = serde_json::json!({
        "author": owner,
        "commentary": escape_little_text(&caption()),
        "visibility": "PUBLIC",
        "distribution": {
            "feedDistribution": "MAIN_FEED",
            "targetEntities": [],
            "thirdPartyDistributionChannels": []
        },
        "content": { "media": { "title": "Data Center Power Capacity (GW)", "id": image_urn } },
        "lifecycleState": "PUBLISHED",
        "isReshareDisabledByAuthor": false
    });
    let post_resp = client
        .post("https://api.linkedin.com/rest/posts")
        .header("Authorization", &auth)
        .header("Content-Type", "application/json")
        .header("LinkedIn-Version", LINKEDIN_VERSION)
        .header("X-Restli-Protocol-Version", "2.0.0")
        .json(&post_body)
        .send()?;
    let post_status = post_resp.status();
    let headers = post_resp.headers().clone();
    let post_text = post_resp.text().unwrap_or_default();
    if !post_status.is_success() {
        return Err(format!("create post failed ({post_status}): {post_text}").into());
    }
    let post_id = headers
        .get("x-restli-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("(unknown)");
    let url = format!("https://www.linkedin.com/feed/update/{post_id}/");
    eprintln!("[linkedin] Published: {url}");
    Ok(url)
}
