//! `usagetray --probe`: find credentials, hit the usage endpoint once, print
//! the raw JSON. Output must match the PowerShell one-liner in SPEC.md section 9.

use crate::{credentials, usage_api};

pub fn run() -> i32 {
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("tokio runtime: {e}");
            return 2;
        }
    };
    rt.block_on(run_async())
}

async fn run_async() -> i32 {
    eprintln!("== UsageTray probe ==");
    eprintln!("candidate files:");
    for p in credentials::candidate_paths() {
        eprintln!("  {} ({})", p.display(), if p.exists() { "exists" } else { "missing" });
    }

    let cred = match credentials::load() {
        Ok(c) => c,
        Err(credentials::CredentialError::Expired { expires_at_ms, .. }) => {
            let exp = chrono::DateTime::from_timestamp_millis(expires_at_ms)
                .map(|d| d.with_timezone(&chrono::Local).to_rfc3339())
                .unwrap_or_else(|| expires_at_ms.to_string());
            eprintln!("status: token_expired (expired at {exp})");
            eprintln!("Run `claude` once in a terminal so Claude Code refreshes the token.");
            return 3;
        }
        Err(e) => {
            eprintln!("status: {e}");
            return 3;
        }
    };

    eprintln!("source:  {}", cred.source);
    eprintln!("token:   {}", credentials::mask(&cred.access_token));
    eprintln!("plan:    {}", cred.subscription_type.as_deref().unwrap_or("-"));
    if let Some(ms) = cred.expires_at_ms {
        let mins = (ms - credentials::now_ms()) / 60_000;
        eprintln!("expires: in {mins} min");
    }
    eprintln!("GET {}", crate::config::USAGE_ENDPOINT);

    let client = match usage_api::build_client() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("client: {e}");
            return 2;
        }
    };

    match usage_api::fetch_raw(&client, &cred.access_token).await {
        Ok(raw) => {
            // Raw body on stdout, pretty printed (comparable with ConvertTo-Json).
            println!("{}", serde_json::to_string_pretty(&raw).unwrap_or_default());
            match usage_api::UsageResponse::from_value(&raw) {
                Ok(parsed) => {
                    let show = |name: &str, w: &Option<usage_api::UsageWindow>| {
                        if let Some(w) = w {
                            let reset = w
                                .resets_at_utc()
                                .map(|d| {
                                    d.with_timezone(&chrono::Local)
                                        .format("%Y-%m-%d %H:%M %Z")
                                        .to_string()
                                })
                                .unwrap_or_else(|| "?".into());
                            eprintln!("{name}: {:.1}%  resets {reset}", w.utilization.unwrap_or(0.0));
                        }
                    };
                    show("five_hour", &parsed.five_hour);
                    show("seven_day", &parsed.seven_day);
                    0
                }
                Err(e) => {
                    eprintln!("parsed: {e}");
                    1
                }
            }
        }
        Err(e) => {
            eprintln!("request failed: {e}");
            1
        }
    }
}
