use serde::{Deserialize, Serialize};

pub const GOOGLE_AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const GOOGLE_TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
pub const GOOGLE_USERINFO_ENDPOINT: &str = "https://www.googleapis.com/oauth2/v2/userinfo";

// Scope Separation Constants
pub const SCOPE_IDENTITY: &str = "openid https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile";
pub const SCOPE_CALENDAR_READONLY: &str = "https://www.googleapis.com/auth/calendar.events.readonly";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoogleOAuthConfig {
    /// Canonical Google OAuth 2.0 Desktop Client ID
    pub client_id: Option<String>,
    /// Google OAuth 2.0 Desktop Client Secret (required by Google's token endpoint even for desktop apps)
    pub client_secret: Option<String>,
}

impl GoogleOAuthConfig {
    /// Resolves the Google Client ID in priority order:
    /// 1. Custom developer override if provided
    /// 2. Compile-time environment variable `RELAY_GOOGLE_CLIENT_ID`
    /// 3. Runtime environment variable `RELAY_GOOGLE_CLIENT_ID`
    ///
    /// If no valid Client ID is configured, returns a clean typed error rather than sending a broken placeholder to Google.
    pub fn resolve_client_id(custom_client_id: Option<String>) -> Result<String, String> {
        // 1. Explicit custom override
        if let Some(c_id) = custom_client_id {
            let clean = c_id.trim().to_string();
            if !clean.is_empty() && !clean.contains("1055740445695") {
                return Ok(clean);
            }
        }

        // 2. Compile-time env
        if let Some(c_id) = option_env!("RELAY_GOOGLE_CLIENT_ID") {
            let clean = c_id.trim().to_string();
            if !clean.is_empty() && !clean.contains("1055740445695") {
                return Ok(clean);
            }
        }

        // 3. Runtime env
        if let Ok(c_id) = std::env::var("RELAY_GOOGLE_CLIENT_ID") {
            let clean = c_id.trim().to_string();
            if !clean.is_empty() && !clean.contains("1055740445695") {
                return Ok(clean);
            }
        }

        tracing::warn!("Google OAuth Client ID is not configured. Set RELAY_GOOGLE_CLIENT_ID environment variable at build or runtime.");
        Err("Google service isn't configured for this installation.".to_string())
    }

    /// Resolves the Google Client Secret in priority order:
    /// 1. Custom developer override if provided
    /// 2. Compile-time environment variable `RELAY_GOOGLE_CLIENT_SECRET`
    /// 3. Runtime environment variable `RELAY_GOOGLE_CLIENT_SECRET`
    pub fn resolve_client_secret(custom_client_secret: Option<String>) -> Result<String, String> {
        // 1. Explicit custom override
        if let Some(c_secret) = custom_client_secret {
            let clean = c_secret.trim().to_string();
            if !clean.is_empty() {
                return Ok(clean);
            }
        }

        // 2. Compile-time env
        if let Some(c_secret) = option_env!("RELAY_GOOGLE_CLIENT_SECRET") {
            let clean = c_secret.trim().to_string();
            if !clean.is_empty() {
                return Ok(clean);
            }
        }

        // 3. Runtime env
        if let Ok(c_secret) = std::env::var("RELAY_GOOGLE_CLIENT_SECRET") {
            let clean = c_secret.trim().to_string();
            if !clean.is_empty() {
                return Ok(clean);
            }
        }

        Err("Google OAuth Client Secret is missing.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_separation_invariants() {
        // Identity scope must NOT contain calendar permissions
        assert!(!SCOPE_IDENTITY.contains("calendar"));
        assert!(SCOPE_IDENTITY.contains("userinfo.email"));
        assert!(SCOPE_IDENTITY.contains("userinfo.profile"));

        // Calendar scope must contain calendar.events.readonly
        assert!(SCOPE_CALENDAR_READONLY.contains("calendar.events.readonly"));
    }

    #[test]
    fn test_resolve_client_id_rejects_placeholders_and_missing() {
        // Explicit placeholder must be rejected
        let res = GoogleOAuthConfig::resolve_client_id(Some("1055740445695-k8i7k2m9h4r2mvgkqu9t03l8b5n4p1q9.apps.googleusercontent.com".to_string()));
        // If no env is set in test runner, it should return Err
        if std::env::var("RELAY_GOOGLE_CLIENT_ID").is_err() && option_env!("RELAY_GOOGLE_CLIENT_ID").is_none() {
            assert!(res.is_err());
            assert!(res.unwrap_err().contains("isn't configured"));
        }

        // Valid custom client ID must be accepted
        let custom_valid = "valid-client-id-123.apps.googleusercontent.com".to_string();
        let res_valid = GoogleOAuthConfig::resolve_client_id(Some(custom_valid.clone()));
        assert_eq!(res_valid.unwrap(), custom_valid);
    }
}
