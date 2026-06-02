//! X / Twitter image posting for gigacrawl.
//!
//! Posts a tweet with the chart image. Image upload requires the v1.1
//! `media/upload` endpoint with OAuth 1.0a (v2 has no media endpoint and media
//! upload does not accept OAuth 2.0 tokens); the tweet itself is created via
//! the v2 `/2/tweets` endpoint, also OAuth 1.0a-signed.
//!
//! Credentials are read from `twitter_credentials.json` (cwd, then $HOME):
//!   {"consumer_key":"...","consumer_secret":"...","token":"...","secret":"..."}
//! If absent, falls back to the first profile in `~/.twurlrc` (the `twurl` CLI
//! config), reusing the same OAuth 1.0a key set.
//!
//! Note: X discontinued the free tier in Feb 2026 — posting is pay-per-use or a
//! legacy paid plan. A 403 here means the account's API access lacks write.

use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha1::Sha1;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha1 = Hmac<Sha1>;

#[derive(Deserialize, Clone)]
pub struct Creds {
    pub consumer_key: String,
    pub consumer_secret: String,
    pub token: String,
    pub secret: String,
}

fn find_file(name: &str) -> Option<PathBuf> {
    let cwd = PathBuf::from(name);
    if cwd.exists() {
        return Some(cwd);
    }
    std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(name))
        .filter(|p| p.exists())
}

/// Minimal extractor for the four OAuth fields from a (single-profile)
/// `~/.twurlrc` YAML file — avoids pulling in a YAML dependency.
fn parse_twurlrc(text: &str) -> Option<Creds> {
    let field = |key: &str| -> Option<String> {
        text.lines()
            .map(str::trim)
            .find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix(':')))
            .map(|v| v.trim().trim_matches('"').to_string())
            .filter(|s| !s.is_empty())
    };
    Some(Creds {
        consumer_key: field("consumer_key")?,
        consumer_secret: field("consumer_secret")?,
        token: field("token")?,
        secret: field("secret")?,
    })
}

fn load_creds() -> Result<Creds, Box<dyn Error>> {
    if let Some(p) = find_file("twitter_credentials.json") {
        eprintln!("[twitter] Using credentials: {}", p.display());
        return Ok(serde_json::from_str(&fs::read_to_string(&p)?)?);
    }
    if let Some(home) = std::env::var_os("HOME") {
        let rc = PathBuf::from(home).join(".twurlrc");
        if rc.exists() {
            eprintln!("[twitter] Using credentials: {}", rc.display());
            return parse_twurlrc(&fs::read_to_string(&rc)?)
                .ok_or_else(|| "could not parse OAuth fields from ~/.twurlrc".into());
        }
    }
    Err("no Twitter credentials found (twitter_credentials.json or ~/.twurlrc)".into())
}

/// RFC 3986 percent-encoding (unreserved chars left as-is).
fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn nonce() -> String {
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let pid = std::process::id();
    format!("{:x}{:x}{:x}", t.as_nanos(), pid, t.subsec_nanos())
}

/// Build the OAuth 1.0a `Authorization` header. Only the oauth_* params are
/// signed — multipart and JSON bodies are excluded from the signature base,
/// which is correct for both endpoints we call.
fn auth_header(creds: &Creds, method: &str, url: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();
    let nonce = nonce();
    let mut params: Vec<(&str, &str)> = vec![
        ("oauth_consumer_key", &creds.consumer_key),
        ("oauth_nonce", &nonce),
        ("oauth_signature_method", "HMAC-SHA1"),
        ("oauth_timestamp", &ts),
        ("oauth_token", &creds.token),
        ("oauth_version", "1.0"),
    ];
    params.sort_by(|a, b| a.0.cmp(b.0));

    let param_str = params
        .iter()
        .map(|(k, v)| format!("{}={}", enc(k), enc(v)))
        .collect::<Vec<_>>()
        .join("&");
    let base = format!("{}&{}&{}", method, enc(url), enc(&param_str));
    let key = format!("{}&{}", enc(&creds.consumer_secret), enc(&creds.secret));

    let mut mac = HmacSha1::new_from_slice(key.as_bytes()).expect("hmac key");
    mac.update(base.as_bytes());
    let sig = {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
    };

    let mut header_params = params.clone();
    let sig_pair = ("oauth_signature", sig.as_str());
    header_params.push(sig_pair);
    header_params.sort_by(|a, b| a.0.cmp(b.0));
    let inner = header_params
        .iter()
        .map(|(k, v)| format!("{}=\"{}\"", enc(k), enc(v)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("OAuth {}", inner)
}

fn tweet_text() -> String {
    "Data center power capacity (GW): operational vs. planned, with FY2025 capex from SEC 10-Ks — \
     Amazon, Microsoft, Google, Meta, xAI, OpenAI, Anthropic.\n\
     Clickable PDF (each figure links to its 10-K): github.com/zdavatz/gigacrawl/blob/main/pdf/datacenter_sources.pdf\n\
     #AI #DataCenters #CapEx"
        .to_string()
}

/// Upload `png_path` and post a tweet with it. Returns the tweet URL.
pub fn publish_image(png_path: &Path) -> Result<String, Box<dyn Error>> {
    let creds = load_creds()?;
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    // Step 1 — upload media (v2 media/upload, multipart, OAuth 1.0a).
    // (v1.1 upload.twitter.com was retired on 2025-03-31.)
    let upload_url = "https://api.twitter.com/2/media/upload";
    let bytes = fs::read(png_path)?;
    eprintln!(
        "[twitter] Uploading {} ({:.1} KB)",
        png_path.display(),
        bytes.len() as f64 / 1024.0
    );
    let form = reqwest::blocking::multipart::Form::new()
        .text("media_category", "tweet_image")
        .text("media_type", "image/png")
        .part(
            "media",
            reqwest::blocking::multipart::Part::bytes(bytes)
                .file_name("datacenter_capacity.png")
                .mime_str("image/png")?,
        );
    let resp = client
        .post(upload_url)
        .header("Authorization", auth_header(&creds, "POST", upload_url))
        .multipart(form)
        .send()?;
    let status = resp.status();
    let text = resp.text()?;
    if !status.is_success() {
        return Err(format!("media upload failed ({status}): {text}").into());
    }
    let v: serde_json::Value = serde_json::from_str(&text)?;
    // v2 returns {"data":{"id":"..."}}; v1.1 returned {"media_id_string":"..."}.
    let media_id = v["data"]["id"]
        .as_str()
        .or_else(|| v["media_id_string"].as_str())
        .or_else(|| v["id"].as_str())
        .ok_or_else(|| format!("no media id in upload response: {text}"))?
        .to_string();
    eprintln!("[twitter] media_id: {media_id}");

    // Step 2 — create tweet (v2, JSON, OAuth 1.0a).
    let tweets_url = "https://api.twitter.com/2/tweets";
    let body = serde_json::json!({
        "text": tweet_text(),
        "media": { "media_ids": [media_id] }
    });
    let resp = client
        .post(tweets_url)
        .header("Authorization", auth_header(&creds, "POST", tweets_url))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()?;
    let status = resp.status();
    let text = resp.text()?;
    if !status.is_success() {
        return Err(format!("create tweet failed ({status}): {text}").into());
    }
    let v: serde_json::Value = serde_json::from_str(&text)?;
    let id = v["data"]["id"].as_str().unwrap_or("(unknown)");
    let url = format!("https://x.com/i/web/status/{id}");
    eprintln!("[twitter] Published: {url}");
    Ok(url)
}
