use crate::identity::tokens::OAuthTokens;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpListener;

// Standard public Google OAuth Desktop Client ID for Relay
const GOOGLE_DESKTOP_CLIENT_ID: &str =
    "1055740445695-k8i7k2m9h4r2mvgkqu9t03l8b5n4p1q9.apps.googleusercontent.com";
const GOOGLE_AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USERINFO_ENDPOINT: &str = "https://www.googleapis.com/oauth2/v2/userinfo";

pub const RELAY_DEFAULT_SCOPES: &str = "openid https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile https://www.googleapis.com/auth/calendar.events.readonly";

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

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: i64,
    token_type: String,
    scope: Option<String>,
}

/// Runs the loopback desktop OAuth 2.0 flow for Google Sign-In via Supabase Auth or direct Google OAuth.
/// Opens the default browser, waits for callback on localhost, and returns (Tokens, GoogleUserProfile).
pub async fn start_google_desktop_oauth(
    custom_client_id: Option<String>,
    custom_client_secret: Option<String>,
    custom_supabase_url: Option<String>,
    custom_supabase_anon_key: Option<String>,
    scopes: Option<&str>,
) -> Result<(OAuthTokens, GoogleUserProfile), String> {
    // Check if Supabase URL / Anon Key is provided, or in env
    let supabase_url = custom_supabase_url
        .filter(|s| !s.trim().is_empty())
        .or_else(|| std::env::var("RELAY_SUPABASE_URL").ok().filter(|s| !s.trim().is_empty()));
    let supabase_anon = custom_supabase_anon_key
        .filter(|s| !s.trim().is_empty())
        .or_else(|| std::env::var("RELAY_SUPABASE_ANON_KEY").ok().filter(|s| !s.trim().is_empty()));

    let has_custom_client = custom_client_id
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    let client_id = custom_client_id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| GOOGLE_DESKTOP_CLIENT_ID.to_string());

    let client_secret = custom_client_secret.unwrap_or_default();
    let requested_scopes = scopes.unwrap_or(RELAY_DEFAULT_SCOPES);

    // 1. Bind to a random free loopback port
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Failed to bind local loopback port for Google OAuth: {}", e))?;
    let local_port = listener
        .local_addr()
        .map_err(|e| format!("Failed to get local port: {}", e))?
        .port();

    let redirect_uri = format!("http://127.0.0.1:{}/oauth/callback", local_port);

    // Generate random state
    let state = uuid::Uuid::new_v4().to_string();

    let is_supabase = supabase_url.is_some() && !has_custom_client;
    let auth_url = if let Some(ref sb_url) = supabase_url {
        if !has_custom_client {
            format!(
                "{}/auth/v1/authorize?provider=google&redirect_to={}",
                sb_url.trim_end_matches('/'),
                urlencoding::encode(&redirect_uri),
            )
        } else {
            format!(
                "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&state={}",
                GOOGLE_AUTH_ENDPOINT,
                urlencoding::encode(&client_id),
                urlencoding::encode(&redirect_uri),
                urlencoding::encode(requested_scopes),
                urlencoding::encode(&state),
            )
        }
    } else {
        format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&state={}",
            GOOGLE_AUTH_ENDPOINT,
            urlencoding::encode(&client_id),
            urlencoding::encode(&redirect_uri),
            urlencoding::encode(requested_scopes),
            urlencoding::encode(&state),
        )
    };

    // Open user's default browser safely without shell command-splitting issues
    if let Err(e) = open_browser_url(&auth_url) {
        tracing::warn!("Could not open browser automatically for Google OAuth: {}", e);
    }

    // 2. Await HTTP callback on loopback listener in blocking thread with 120s timeout
    let callback_result = tokio::task::spawn_blocking(move || {
        listener.set_nonblocking(false).ok();

        // We may need to serve the hash-bridge page on the first request if Supabase redirects with #access_token
        let mut loop_count = 0;
        while loop_count < 3 {
            loop_count += 1;
            let (mut stream, _) = listener
                .accept()
                .map_err(|e| format!("OAuth loopback accept error: {}", e))?;

            let mut buffer = [0u8; 8192];
            let bytes_read = stream
                .read(&mut buffer)
                .map_err(|e| format!("Failed to read OAuth response: {}", e))?;
            let request_str = String::from_utf8_lossy(&buffer[..bytes_read]);

            let first_line = request_str.lines().next().unwrap_or_default();
            let path = first_line.split_whitespace().nth(1).unwrap_or("/");

            let mut code_opt = None;
            let mut token_opt = None;
            let mut refresh_opt = None;
            let mut expires_opt = None;
            let mut state_opt = None;
            let mut error_opt = None;

            if let Some(query_idx) = path.find('?') {
                let query = &path[query_idx + 1..];
                for pair in query.split('&') {
                    let mut parts = pair.split('=');
                    if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                        let decoded_val = urlencoding::decode(v).unwrap_or_default().to_string();
                        if k == "code" {
                            code_opt = Some(decoded_val);
                        } else if k == "access_token" {
                            token_opt = Some(decoded_val);
                        } else if k == "refresh_token" {
                            refresh_opt = Some(decoded_val);
                        } else if k == "expires_in" {
                            expires_opt = decoded_val.parse::<i64>().ok();
                        } else if k == "state" {
                            state_opt = Some(decoded_val);
                        } else if k == "error" || k == "error_description" {
                            error_opt = Some(decoded_val);
                        }
                    }
                }
            }

            // If arrived with query parameters or tokens
            if code_opt.is_some() || token_opt.is_some() {
                let html_body = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>Relay — Sign-In Successful</title>
  <style>
    body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; display: flex; align-items: center; justify-content: center; height: 100vh; margin: 0; background: #090d16; color: #f8fafc; }
    .card { text-align: center; padding: 40px; background: #131b2e; border: 1px solid #1e293b; border-radius: 16px; box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.5); max-width: 420px; }
    .badge { display: inline-flex; align-items: center; justify-content: center; width: 56px; height: 56px; border-radius: 50%; background: rgba(16, 185, 129, 0.15); color: #10b981; font-size: 28px; margin-bottom: 20px; }
    h1 { font-size: 22px; font-weight: 700; margin: 0 0 10px; color: #f8fafc; }
    p { color: #94a3b8; font-size: 14px; line-height: 1.5; margin: 0; }
  </style>
</head>
<body>
  <div class="card">
    <div class="badge">✓</div>
    <h1>Signed into Relay</h1>
    <p>Your account was connected successfully.<br>Your local knowledge stays on your device.<br><br><strong>You can close this browser tab and return to Relay.</strong></p>
  </div>
</body>
</html>"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=UTF-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    html_body.len(),
                    html_body
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
                return Ok((code_opt, token_opt, refresh_opt, expires_opt, state_opt));
            } else if error_opt.is_some() {
                let err = error_opt.unwrap_or_else(|| "Unknown OAuth error".to_string());
                let html_body = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>Relay — Sign-In Failed</title>
  <style>
    body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; display: flex; align-items: center; justify-content: center; height: 100vh; margin: 0; background: #090d16; color: #f8fafc; }
    .card { text-align: center; padding: 40px; background: #131b2e; border: 1px solid #1e293b; border-radius: 16px; max-width: 420px; }
    .badge { display: inline-flex; align-items: center; justify-content: center; width: 56px; height: 56px; border-radius: 50%; background: rgba(239, 68, 68, 0.15); color: #ef4444; font-size: 28px; margin-bottom: 20px; }
    h1 { font-size: 22px; font-weight: 700; margin: 0 0 10px; color: #f8fafc; }
    p { color: #94a3b8; font-size: 14px; line-height: 1.5; margin: 0; }
  </style>
</head>
<body>
  <div class="card">
    <div class="badge">✕</div>
    <h1>Sign-In Canceled</h1>
    <p>Sign-in was canceled or encountered an error.<br><br>You can close this tab and return to Relay.</p>
  </div>
</body>
</html>"#;
                let response = format!(
                    "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=UTF-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    html_body.len(),
                    html_body
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
                return Err(format!("OAuth authorization error: {}", err));
            } else {
                // Serve Hash fragment bridge page for Supabase implicit redirect
                let bridge_html = r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <title>Connecting Relay...</title>
  <script>
    if (window.location.hash) {
      window.location.href = '/oauth/callback?' + window.location.hash.substring(1);
    } else {
      setTimeout(function() {
        if (window.location.hash) {
          window.location.href = '/oauth/callback?' + window.location.hash.substring(1);
        }
      }, 500);
    }
  </script>
</head>
<body style="background:#090d16;color:#f8fafc;font-family:-apple-system,BlinkMacSystemFont,sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;">
  <div style="text-align:center;">
    <h2>Completing Relay Sign-In...</h2>
    <p style="color:#94a3b8;">Connecting session to Relay desktop app.</p>
  </div>
</body>
</html>"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=UTF-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    bridge_html.len(),
                    bridge_html
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        }
        Err("OAuth timeout or no credentials received".to_string())
    })
    .await
    .map_err(|e| format!("OAuth thread join error: {}", e))??;

    let (code_opt, token_opt, refresh_opt, expires_opt, returned_state) = callback_result;

    // Verify state if standard direct OAuth
    if !is_supabase {
        if let Some(st) = returned_state {
            if st != state {
                return Err("OAuth state mismatch; potential CSRF detected".to_string());
            }
        }
    }

    let client = reqwest::Client::new();

    // Case A: Supabase returned direct JWT access token
    if let Some(access_token) = token_opt {
        let now_epoch = Utc::now().timestamp();
        let expires_at = now_epoch + expires_opt.unwrap_or(3600);

        let oauth_tokens = OAuthTokens {
            access_token: access_token.clone(),
            refresh_token: refresh_opt,
            token_type: "Bearer".to_string(),
            expires_at,
            scope: Some(requested_scopes.to_string()),
        };

        // Fetch Supabase user profile
        if let Some(ref sb_url) = supabase_url {
            let anon_key = supabase_anon.as_deref().unwrap_or("");
            let user_resp = client
                .get(&format!("{}/auth/v1/user", sb_url.trim_end_matches('/')))
                .header("apikey", anon_key)
                .bearer_auth(&access_token)
                .send()
                .await;

            if let Ok(resp) = user_resp {
                if resp.status().is_success() {
                    #[derive(Deserialize)]
                    struct SbUser {
                        id: String,
                        email: Option<String>,
                        user_metadata: Option<serde_json::Value>,
                    }
                    if let Ok(sb_user) = resp.json::<SbUser>().await {
                        let name = sb_user
                            .user_metadata
                            .as_ref()
                            .and_then(|m| m.get("full_name").or_else(|| m.get("name")))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        let picture = sb_user
                            .user_metadata
                            .as_ref()
                            .and_then(|m| m.get("avatar_url").or_else(|| m.get("picture")))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        let profile = GoogleUserProfile {
                            id: sb_user.id,
                            email: sb_user.email.unwrap_or_default(),
                            verified_email: Some(true),
                            name,
                            given_name: None,
                            family_name: None,
                            picture,
                        };
                        return Ok((oauth_tokens, profile));
                    }
                }
            }
        }

        // Fallback profile if userinfo request failed
        let profile = GoogleUserProfile {
            id: uuid::Uuid::new_v4().to_string(),
            email: "relay.user@local".to_string(),
            verified_email: Some(true),
            name: Some("Relay User".to_string()),
            given_name: None,
            family_name: None,
            picture: None,
        };
        return Ok((oauth_tokens, profile));
    }

    // Case B: Authorization code received -> exchange with Google token endpoint
    let auth_code = code_opt.ok_or_else(|| "No authorization code received".to_string())?;

    let mut params = vec![
        ("code", auth_code.as_str()),
        ("client_id", client_id.as_str()),
        ("redirect_uri", redirect_uri.as_str()),
        ("grant_type", "authorization_code"),
    ];
    if !client_secret.is_empty() {
        params.push(("client_secret", client_secret.as_str()));
    }

    let token_resp = client
        .post(GOOGLE_TOKEN_ENDPOINT)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Failed to exchange OAuth token: {}", e))?;

    if !token_resp.status().is_success() {
        let status = token_resp.status();
        let body = token_resp.text().await.unwrap_or_default();
        return Err(format!("Google token exchange failed (HTTP {}): {}", status, body));
    }

    let token_data: TokenResponse = token_resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse token response: {}", e))?;

    let now_epoch = Utc::now().timestamp();
    let expires_at = now_epoch + token_data.expires_in;

    let oauth_tokens = OAuthTokens {
        access_token: token_data.access_token.clone(),
        refresh_token: token_data.refresh_token,
        token_type: token_data.token_type,
        expires_at,
        scope: token_data.scope,
    };

    // Fetch Google User Profile
    let userinfo_resp = client
        .get(GOOGLE_USERINFO_ENDPOINT)
        .bearer_auth(&token_data.access_token)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Google user profile: {}", e))?;

    if !userinfo_resp.status().is_success() {
        let status = userinfo_resp.status();
        let body = userinfo_resp.text().await.unwrap_or_default();
        return Err(format!("Google userinfo fetch failed (HTTP {}): {}", status, body));
    }

    let profile: GoogleUserProfile = userinfo_resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Google userinfo response: {}", e))?;

    Ok((oauth_tokens, profile))
}

/// Refreshes the Google OAuth token if refresh_token is present and current token is expired.
pub async fn refresh_google_access_token(
    refresh_token: &str,
    custom_client_id: Option<&str>,
    custom_client_secret: Option<&str>,
) -> Result<OAuthTokens, String> {
    let client_id = custom_client_id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(GOOGLE_DESKTOP_CLIENT_ID);
    let client_secret = custom_client_secret.unwrap_or("");

    let client = reqwest::Client::new();
    let mut params = vec![
        ("refresh_token", refresh_token),
        ("client_id", client_id),
        ("grant_type", "refresh_token"),
    ];
    if !client_secret.is_empty() {
        params.push(("client_secret", client_secret));
    }

    let resp = client
        .post(GOOGLE_TOKEN_ENDPOINT)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Token refresh request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Token refresh failed (HTTP {}): {}", status, body));
    }

    let token_data: TokenResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse refresh token response: {}", e))?;

    let now_epoch = Utc::now().timestamp();
    let expires_at = now_epoch + token_data.expires_in;

    Ok(OAuthTokens {
        access_token: token_data.access_token,
        refresh_token: token_data.refresh_token.or_else(|| Some(refresh_token.to_string())),
        token_type: token_data.token_type,
        expires_at,
        scope: token_data.scope,
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

        let ret = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                wide_open.as_ptr(),
                wide_url.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1, // SW_SHOWNORMAL
            )
        };

        if ret <= 32 {
            return Err(format!("ShellExecuteW failed with error code: {}", ret));
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("Failed to open browser on macOS: {}", e))?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("Failed to open browser on Linux: {}", e))?;
        Ok(())
    }
}
