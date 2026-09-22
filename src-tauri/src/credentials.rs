//! Read-only access to Claude Code's OAuth credentials.
//!
//! NEVER write to `.credentials.json`. Claude Code rotates the refresh token
//! on every use; touching the file breaks the user's session.

use std::path::PathBuf;

use serde::Deserialize;
use thiserror::Error;

use crate::config;

#[derive(Debug, Clone)]
pub struct Credentials {
    pub access_token: String,
    /// Epoch milliseconds. `None` when the token came from the environment.
    pub expires_at_ms: Option<i64>,
    /// "pro" | "max" | ... as reported by Claude Code. Lowercased.
    pub subscription_type: Option<String>,
    /// Human readable origin for logs / probe output (never the token itself).
    pub source: String,
}

#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("no Claude Code credentials found")]
    NotFound,
    #[error("access token expired")]
    Expired {
        expires_at_ms: i64,
        subscription_type: Option<String>,
    },
    #[error("credentials file malformed: {0}")]
    Malformed(String),
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CredFile {
    claude_ai_oauth: Option<OauthBlock>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct OauthBlock {
    access_token: Option<String>,
    expires_at: Option<i64>,
    subscription_type: Option<String>,
}

/// Candidate credential files in lookup order (env var is handled separately).
pub fn candidate_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        if !dir.is_empty() {
            out.push(PathBuf::from(dir).join(".credentials.json"));
        }
    }
    if let Some(home) = config::home_dir() {
        let p = home.join(".claude").join(".credentials.json");
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Locate and parse credentials. Re-read on every call: Claude Code rewrites
/// the file whenever it refreshes the token.
pub fn load() -> Result<Credentials, CredentialError> {
    if let Ok(tok) = std::env::var("CLAUDE_CODE_OAUTH_TOKEN") {
        let tok = tok.trim().to_string();
        if !tok.is_empty() {
            return Ok(Credentials {
                access_token: tok,
                expires_at_ms: None,
                subscription_type: None,
                source: "env:CLAUDE_CODE_OAUTH_TOKEN".into(),
            });
        }
    }

    let mut last_err: Option<CredentialError> = None;
    for path in candidate_paths() {
        match std::fs::read(&path) {
            Ok(bytes) => match parse(&bytes) {
                Ok(mut c) => {
                    c.source = path.display().to_string();
                    return finalize(c);
                }
                Err(e) => last_err = Some(e),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => last_err = Some(CredentialError::Malformed(format!("{}: {e}", path.display()))),
        }
    }
    Err(last_err.unwrap_or(CredentialError::NotFound))
}

fn parse(bytes: &[u8]) -> Result<Credentials, CredentialError> {
    let file: CredFile =
        serde_json::from_slice(bytes).map_err(|e| CredentialError::Malformed(e.to_string()))?;
    let block = file
        .claude_ai_oauth
        .ok_or_else(|| CredentialError::Malformed("missing claudeAiOauth".into()))?;
    let token = block
        .access_token
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .ok_or_else(|| CredentialError::Malformed("missing accessToken".into()))?;
    Ok(Credentials {
        access_token: token,
        expires_at_ms: block.expires_at,
        subscription_type: block.subscription_type.map(|s| s.to_lowercase()),
        source: String::new(),
    })
}

fn finalize(c: Credentials) -> Result<Credentials, CredentialError> {
    if let Some(exp) = c.expires_at_ms {
        if exp <= now_ms() {
            return Err(CredentialError::Expired {
                expires_at_ms: exp,
                subscription_type: c.subscription_type,
            });
        }
    }
    Ok(c)
}

/// `sk-ant-oat01-****` : safe for logs.
pub fn mask(token: &str) -> String {
    let keep = token
        .char_indices()
        .filter(|(_, ch)| *ch == '-')
        .nth(2)
        .map(|(i, _)| i)
        .unwrap_or_else(|| token.len().min(8));
    format!("{}-****", &token[..keep])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_expected_schema() {
        let json = br#"{"claudeAiOauth":{"accessToken":"sk-ant-oat01-abc","refreshToken":"sk-ant-ort01-x","expiresAt":1758556800000,"scopes":["user:inference"],"subscriptionType":"Max"}}"#;
        let c = parse(json).unwrap();
        assert_eq!(c.access_token, "sk-ant-oat01-abc");
        assert_eq!(c.expires_at_ms, Some(1758556800000));
        assert_eq!(c.subscription_type.as_deref(), Some("max"));
    }

    #[test]
    fn tolerates_missing_optional_fields() {
        let json = br#"{"claudeAiOauth":{"accessToken":"sk-ant-oat01-abc"}}"#;
        let c = parse(json).unwrap();
        assert!(c.expires_at_ms.is_none());
        assert!(c.subscription_type.is_none());
    }

    #[test]
    fn rejects_missing_token() {
        assert!(matches!(
            parse(br#"{"claudeAiOauth":{}}"#),
            Err(CredentialError::Malformed(_))
        ));
        assert!(matches!(parse(b"not json"), Err(CredentialError::Malformed(_))));
    }

    #[test]
    fn expired_token_is_detected() {
        let c = Credentials {
            access_token: "sk-ant-oat01-x".into(),
            expires_at_ms: Some(1),
            subscription_type: None,
            source: String::new(),
        };
        assert!(matches!(finalize(c), Err(CredentialError::Expired { .. })));
    }

    #[test]
    fn mask_hides_secret() {
        assert_eq!(mask("sk-ant-oat01-verysecretvalue"), "sk-ant-oat01-****");
        assert_eq!(mask("short"), "short-****");
        assert!(!mask("sk-ant-oat01-verysecretvalue").contains("verysecret"));
    }
}
