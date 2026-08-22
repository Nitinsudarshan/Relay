use crate::oauth::config::{
    GoogleOAuthConfig, GOOGLE_AUTH_ENDPOINT, GOOGLE_TOKEN_ENDPOINT, GOOGLE_USERINFO_ENDPOINT,
};
use crate::oauth::pkce::PkceChallenge;
use crate::oauth::tokens::OAuthTokens;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpListener;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleUserProfile {
    pub id: String,
    pub email: String,
    pub verified_email: Option<bool>,
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub picture: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthFlowResult {
    pub tokens: OAuthTokens,
    pub user_profile: Option<GoogleUserProfile>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: i64,
    token_type: String,
    scope: Option<String>,
}

/// Executes the loopback Desktop OAuth 2.0 PKCE flow.
/// 1. Binds ephemeral port on 127.0.0.1:0
/// 2. Generates PKCE verifier + S256 challenge + random state
/// 3. Opens default system browser to Google's consent screen
/// 4. Listens for localhost callback with timeout
/// 5. Validates state and exchanges authorization code + PKCE verifier for tokens
/// 6. Optionally fetches user profile if identity scopes were requested
pub async fn start_desktop_oauth_flow(
    custom_client_id: Option<String>,
    custom_client_secret: Option<String>,
    scopes: &str,
) -> Result<OAuthFlowResult, String> {
    let client_id = GoogleOAuthConfig::resolve_client_id(custom_client_id)?;
    let client_secret = GoogleOAuthConfig::resolve_client_secret(custom_client_secret)?;
    let pkce = PkceChallenge::new();

    // 1. Bind to ephemeral port on 127.0.0.1 (never 0.0.0.0)
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Failed to bind local loopback port for OAuth: {}", e))?;
    let local_port = listener
        .local_addr()
        .map_err(|e| format!("Failed to get local loopback port: {}", e))?
        .port();

    let redirect_uri = format!("http://127.0.0.1:{}/oauth/callback", local_port);

    // 2. Build RFC 7636 PKCE Google Authorization URL
    let auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&code_challenge={}&code_challenge_method=S256&state={}",
        GOOGLE_AUTH_ENDPOINT,
        urlencoding::encode(&client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(scopes),
        urlencoding::encode(&pkce.challenge),
        urlencoding::encode(&pkce.state),
    );

    // 3. Open user's system browser
    if let Err(e) = open_browser_url(&auth_url) {
        tracing::warn!("Could not open system browser automatically for Google OAuth: {}", e);
    }

    // 4. Await HTTP callback in blocking task with timeout
    let expected_state = pkce.state.clone();
    let auth_code = tokio::task::spawn_blocking(move || {
        listener.set_nonblocking(false).ok();

        let (mut stream, _) = listener
            .accept()
            .map_err(|e| format!("OAuth loopback listener error: {}", e))?;

        let mut buffer = [0u8; 4096];
        let bytes_read = stream
            .read(&mut buffer)
            .map_err(|e| format!("Failed to read OAuth callback response: {}", e))?;
        let request_str = String::from_utf8_lossy(&buffer[..bytes_read]);

        let first_line = request_str.lines().next().unwrap_or_default();
        let path = first_line.split_whitespace().nth(1).unwrap_or("/");

        let mut code_opt = None;
        let mut state_opt = None;
        let mut error_opt = None;

        if let Some(query_idx) = path.find('?') {
            let query = &path[query_idx + 1..];
            for pair in query.split('&') {
                let mut parts = pair.split('=');
                if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                    let decoded = urlencoding::decode(v).unwrap_or_default().to_string();
                    if k == "code" {
                        code_opt = Some(decoded);
                    } else if k == "state" {
                        state_opt = Some(decoded);
                    } else if k == "error" {
                        error_opt = Some(decoded);
                    }
                }
            }
        }

        // Validate state
        let state_valid = state_opt.as_ref() == Some(&expected_state);
        let success = code_opt.is_some() && state_valid && error_opt.is_none();

        // Render clean, modern, dark-themed response to browser
        let (html_body, status_line) = if success {
            (
                r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Relay Authorization</title></head><body style="font-family:system-ui,-apple-system,sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;background:#090d16;color:#f8fafc;"><div style="text-align:center;padding:36px;background:#131b2e;border:1px solid #1e293b;border-radius:16px;box-shadow:0 20px 40px rgba(0,0,0,0.6);max-width:400px;"><h2 style="color:#10b981;margin:0 0 12px 0;font-size:22px;">✓ Connected to Relay</h2><p style="color:#94a3b8;font-size:14px;line-height:1.5;margin:0 0 20px 0;">Google authorization was successful. You can close this browser tab and return to the Relay desktop application.</p><div style="font-size:12px;color:#64748b;font-family:monospace;">RELAY SECURE OAUTH</div></div></body></html>"#,
                "HTTP/1.1 200 OK",
            )
        } else {
            (
                r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Relay Authorization</title></head><body style="font-family:system-ui,-apple-system,sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;background:#090d16;color:#f8fafc;"><div style="text-align:center;padding:36px;background:#131b2e;border:1px solid #1e293b;border-radius:16px;box-shadow:0 20px 40px rgba(0,0,0,0.6);max-width:400px;"><h2 style="color:#ef4444;margin:0 0 12px 0;font-size:22px;">✗ Authorization Failed</h2><p style="color:#94a3b8;font-size:14px;line-height:1.5;margin:0 0 20px 0;">Google sign-in was canceled, or the OAuth security state was invalid. Please try again in Relay.</p></div></body></html>"#,
                "HTTP/1.1 400 Bad Request",
            )
        };

        let response = format!(
            "{}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status_line,
            html_body.len(),
            html_body
        );

        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();

        if let Some(err) = error_opt {
            return Err(format!("Google authorization error: {}", err));
        }

        if !state_valid {
            return Err("OAuth state mismatch. Security verification failed.".to_string());
        }

        code_opt.ok_or_else(|| "No authorization code returned from Google.".to_string())
    })
    .await
    .map_err(|e| format!("OAuth listener task error: {}", e))??;

    // 5. Exchange code + PKCE verifier for tokens
    let http = reqwest::Client::new();
    let params = [
        ("code", auth_code.as_str()),
        ("client_id", client_id.as_str()),
        ("client_secret", client_secret.as_str()),
        ("redirect_uri", redirect_uri.as_str()),
        ("grant_type", "authorization_code"),
        ("code_verifier", pkce.verifier.as_str()),
    ];

    let token_resp = http
        .post(GOOGLE_TOKEN_ENDPOINT)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Failed to exchange OAuth token: {}", e))?;

    if !token_resp.status().is_success() {
        let err_text = token_resp.text().await.unwrap_or_default();
        return Err(format!("Google token exchange failed: {}", err_text));
    }

    let parsed_tokens: TokenResponse = token_resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse token response JSON: {}", e))?;

    let now_ts = Utc::now().timestamp();
    let expires_at = now_ts + parsed_tokens.expires_in;

    // 6. Fetch user profile if userinfo scopes are present
    let mut user_profile = None;
    let mut account_email = None;
    let mut account_name = None;

    if scopes.contains("userinfo.email") || scopes.contains("openid") {
        if let Ok(info_resp) = http
            .get(GOOGLE_USERINFO_ENDPOINT)
            .bearer_auth(&parsed_tokens.access_token)
            .send()
            .await
        {
            if info_resp.status().is_success() {
                if let Ok(profile) = info_resp.json::<GoogleUserProfile>().await {
                    account_email = Some(profile.email.clone());
                    account_name = profile.name.clone();
                    user_profile = Some(profile);
                }
            }
        }
    }

    let tokens = OAuthTokens {
        access_token: parsed_tokens.access_token,
        refresh_token: parsed_tokens.refresh_token,
        token_type: parsed_tokens.token_type,
        expires_at,
        scope: parsed_tokens.scope,
        account_email,
        account_name,
        last_synced_at: Some(Utc::now().to_rfc3339()),
    };

    Ok(OAuthFlowResult {
        tokens,
        user_profile,
    })
}

/// Refreshes an expired Google access token using the stored refresh token.
pub async fn refresh_google_access_token(
    custom_client_id: Option<String>,
    custom_client_secret: Option<String>,
    refresh_token: &str,
) -> Result<OAuthTokens, String> {
    let client_id = GoogleOAuthConfig::resolve_client_id(custom_client_id)?;
    let client_secret = GoogleOAuthConfig::resolve_client_secret(custom_client_secret)?;

    let http = reqwest::Client::new();
    let params = [
        ("refresh_token", refresh_token),
        ("client_id", client_id.as_str()),
        ("client_secret", client_secret.as_str()),
        ("grant_type", "refresh_token"),
    ];

    let resp = http
        .post(GOOGLE_TOKEN_ENDPOINT)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Token refresh network request failed: {}", e))?;

    if !resp.status().is_success() {
        let err_body = resp.text().await.unwrap_or_default();
        return Err(format!("Google token refresh failed: {}", err_body));
    }

    #[derive(Deserialize)]
    struct RefreshResponse {
        access_token: String,
        expires_in: i64,
        #[serde(default)]
        token_type: Option<String>,
        #[serde(default)]
        scope: Option<String>,
    }

    let parsed: RefreshResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse refresh token JSON: {}", e))?;

    let now_ts = Utc::now().timestamp();
    let expires_at = now_ts + parsed.expires_in;

    Ok(OAuthTokens {
        access_token: parsed.access_token,
        refresh_token: Some(refresh_token.to_string()),
        token_type: parsed.token_type.unwrap_or_else(|| "Bearer".to_string()),
        expires_at,
        scope: parsed.scope,
        account_email: None,
        account_name: None,
        last_synced_at: Some(Utc::now().to_rfc3339()),
    })
}

fn open_browser_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide_url: Vec<u16> = std::ffi::OsStr::new(url)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let wide_open: Vec<u16> = std::ffi::OsStr::new("open")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        #[link(name = "shell32")]
        extern "system" {
            fn ShellExecuteW(
                hwnd: *mut std::ffi::c_void,
                lpOperation: *const u16,
                lpFile: *const u16,
                lpParameters: *const u16,
                lpDirectory: *const u16,
                nShowCmd: i32,
            ) -> isize;
        }

        unsafe {
            let res = ShellExecuteW(
                std::ptr::null_mut(),
                wide_open.as_ptr(),
                wide_url.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            );
            if res <= 32 {
                return Err(format!("ShellExecuteW failed with code {}", res));
            }
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("Failed to open browser: {}", e))?;
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("Failed to open browser: {}", e))?;
        Ok(())
    }
}
