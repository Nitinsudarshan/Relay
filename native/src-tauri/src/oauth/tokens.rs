use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub expires_at: i64, // Unix timestamp in seconds
    pub scope: Option<String>,
    #[serde(default)]
    pub account_email: Option<String>,
    #[serde(default)]
    pub account_name: Option<String>,
    #[serde(default)]
    pub last_synced_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenNamespace {
    Identity,
    Calendar,
}

impl TokenNamespace {
    pub fn service_name(&self) -> &'static str {
        match self {
            Self::Identity => "com.relay.app.identity",
            Self::Calendar => "com.relay.app.calendar",
        }
    }

    pub fn username(&self) -> &'static str {
        match self {
            Self::Identity => "google_account_tokens",
            Self::Calendar => "google_calendar_tokens",
        }
    }

    pub fn fallback_filename(&self) -> &'static str {
        match self {
            Self::Identity => "auth_tokens.bin",
            Self::Calendar => "calendar_tokens.bin",
        }
    }
}

pub struct KeyringTokenStore;

impl KeyringTokenStore {
    fn get_fallback_path(config_dir: &Path, fallback_filename: &str) -> PathBuf {
        config_dir.join(fallback_filename)
    }

    fn obfuscate_bytes(data: &[u8]) -> Vec<u8> {
        let key = b"relay_secure_oauth_store_key_2026";
        data.iter()
            .enumerate()
            .map(|(i, &b)| b ^ key[i % key.len()])
            .collect()
    }

    /// Saves OAuth tokens to the OS Keyring under the standard namespace.
    pub fn save(config_dir: &Path, namespace: TokenNamespace, tokens: &OAuthTokens) -> Result<(), String> {
        Self::save_explicit(
            config_dir,
            namespace.service_name(),
            namespace.username(),
            namespace.fallback_filename(),
            tokens,
        )
    }

    /// Loads OAuth tokens from the OS Keyring for the standard namespace.
    pub fn load(config_dir: &Path, namespace: TokenNamespace) -> Option<OAuthTokens> {
        Self::load_explicit(
            config_dir,
            namespace.service_name(),
            namespace.username(),
            namespace.fallback_filename(),
        )
    }

    /// Deletes OAuth tokens from the OS Keyring and fallback store for the standard namespace.
    pub fn delete(config_dir: &Path, namespace: TokenNamespace) -> Result<(), String> {
        Self::delete_explicit(
            config_dir,
            namespace.service_name(),
            namespace.username(),
            namespace.fallback_filename(),
        )
    }

    pub fn save_explicit(
        config_dir: &Path,
        service: &str,
        username: &str,
        fallback_filename: &str,
        tokens: &OAuthTokens,
    ) -> Result<(), String> {
        let json = serde_json::to_string(tokens)
            .map_err(|e| format!("Failed to serialize tokens: {}", e))?;

        // 1. Attempt to store in OS Keyring
        if let Ok(entry) = Entry::new(service, username) {
            if entry.set_password(&json).is_ok() {
                let fallback = Self::get_fallback_path(config_dir, fallback_filename);
                if fallback.exists() {
                    let _ = fs::remove_file(fallback);
                }
                return Ok(());
            }
        }

        // 2. Fallback to obfuscated storage in .relay/config/ (outside vault)
        let fallback = Self::get_fallback_path(config_dir, fallback_filename);
        if let Some(parent) = fallback.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let encrypted = Self::obfuscate_bytes(json.as_bytes());
        fs::write(fallback, encrypted)
            .map_err(|e| format!("Failed to write secure tokens fallback: {}", e))?;

        Ok(())
    }

    pub fn load_explicit(
        config_dir: &Path,
        service: &str,
        username: &str,
        fallback_filename: &str,
    ) -> Option<OAuthTokens> {
        // 1. Check OS Keyring
        if let Ok(entry) = Entry::new(service, username) {
            if let Ok(password) = entry.get_password() {
                if let Ok(tokens) = serde_json::from_str::<OAuthTokens>(&password) {
                    return Some(tokens);
                }
            }
        }

        // 2. Check fallback file in config directory
        let fallback = Self::get_fallback_path(config_dir, fallback_filename);
        if fallback.exists() {
            if let Ok(bytes) = fs::read(&fallback) {
                let decrypted = Self::obfuscate_bytes(&bytes);
                if let Ok(json_str) = String::from_utf8(decrypted) {
                    if let Ok(tokens) = serde_json::from_str::<OAuthTokens>(&json_str) {
                        return Some(tokens);
                    }
                }
            }
        }

        None
    }

    pub fn delete_explicit(
        config_dir: &Path,
        service: &str,
        username: &str,
        fallback_filename: &str,
    ) -> Result<(), String> {
        if let Ok(entry) = Entry::new(service, username) {
            let _ = entry.delete_password();
        }

        let fallback = Self::get_fallback_path(config_dir, fallback_filename);
        if fallback.exists() {
            let _ = fs::remove_file(fallback);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_token_store_namespace_isolation() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_oauth_store_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        let unique_id = uuid::Uuid::new_v4().to_string();
        let service_id = format!("com.relay.test.identity.{}", unique_id);
        let service_cal = format!("com.relay.test.calendar.{}", unique_id);

        let identity_tokens = OAuthTokens {
            access_token: "identity_access_123".to_string(),
            refresh_token: Some("identity_refresh_123".to_string()),
            token_type: "Bearer".to_string(),
            expires_at: Utc::now().timestamp() + 3600,
            scope: Some("openid email".to_string()),
            account_email: Some("user@example.com".to_string()),
            account_name: Some("User Name".to_string()),
            last_synced_at: None,
        };

        let calendar_tokens = OAuthTokens {
            access_token: "calendar_access_456".to_string(),
            refresh_token: Some("calendar_refresh_456".to_string()),
            token_type: "Bearer".to_string(),
            expires_at: Utc::now().timestamp() + 3600,
            scope: Some("calendar.events.readonly".to_string()),
            account_email: Some("user@example.com".to_string()),
            account_name: Some("User Name".to_string()),
            last_synced_at: Some("2026-08-23T00:00:00Z".to_string()),
        };

        // Save under separate namespaces
        KeyringTokenStore::save_explicit(&temp_dir, &service_id, "user1", "auth.bin", &identity_tokens).unwrap();
        KeyringTokenStore::save_explicit(&temp_dir, &service_cal, "user2", "cal.bin", &calendar_tokens).unwrap();

        // Load and verify isolation
        let loaded_id = KeyringTokenStore::load_explicit(&temp_dir, &service_id, "user1", "auth.bin").unwrap();
        let loaded_cal = KeyringTokenStore::load_explicit(&temp_dir, &service_cal, "user2", "cal.bin").unwrap();

        assert_eq!(loaded_id.access_token, "identity_access_123");
        assert_eq!(loaded_cal.access_token, "calendar_access_456");

        // Deleting Calendar tokens must NOT delete Identity tokens
        KeyringTokenStore::delete_explicit(&temp_dir, &service_cal, "user2", "cal.bin").unwrap();
        assert!(KeyringTokenStore::load_explicit(&temp_dir, &service_cal, "user2", "cal.bin").is_none());
        assert!(KeyringTokenStore::load_explicit(&temp_dir, &service_id, "user1", "auth.bin").is_some());

        // Cleanup
        KeyringTokenStore::delete_explicit(&temp_dir, &service_id, "user1", "auth.bin").unwrap();
        let _ = fs::remove_dir_all(temp_dir);
    }
}
