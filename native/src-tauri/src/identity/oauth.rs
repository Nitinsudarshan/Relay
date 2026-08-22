pub use crate::oauth::{
    refresh_google_access_token, start_desktop_oauth_flow, GoogleUserProfile, OAuthFlowResult,
    OAuthTokens, SCOPE_IDENTITY as RELAY_DEFAULT_SCOPES,
};

/// Runs the loopback desktop OAuth 2.0 PKCE flow for Google Sign-In.
/// Requests strictly identity scopes (openid, email, profile) — never calendar permissions.
pub async fn start_google_desktop_oauth(
    custom_client_id: Option<String>,
    custom_client_secret: Option<String>,
    _custom_supabase_url: Option<String>,
    _custom_supabase_anon_key: Option<String>,
    scopes: Option<&str>,
) -> Result<(OAuthTokens, GoogleUserProfile), String> {
    let requested_scopes = scopes.unwrap_or(RELAY_DEFAULT_SCOPES);
    let result = start_desktop_oauth_flow(custom_client_id, custom_client_secret, requested_scopes).await?;

    let profile = result.user_profile.ok_or_else(|| {
        "Failed to retrieve Google user profile information. Please ensure identity permissions are granted.".to_string()
    })?;

    Ok((result.tokens, profile))
}
