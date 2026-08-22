pub mod installation;
pub mod models;
pub mod oauth;
pub mod supabase;
pub mod tokens;

use chrono::Utc;
pub use installation::{get_or_create_installation_info, mask_installation_id};
pub use models::{AccountMode, InstallationInfo, RelayAccount, SubscriptionInfo, SubscriptionPlan};
pub use oauth::{
    refresh_google_access_token, start_google_desktop_oauth, GoogleUserProfile,
    RELAY_DEFAULT_SCOPES,
};
use std::fs;
use std::path::{Path, PathBuf};
pub use supabase::SupabaseClient;
pub use tokens::{delete_oauth_tokens, load_oauth_tokens, save_oauth_tokens, OAuthTokens};

const ACCOUNT_METADATA_FILE: &str = "account.json";

fn get_account_metadata_path(config_dir: &Path) -> PathBuf {
    config_dir.join(ACCOUNT_METADATA_FILE)
}

/// Loads the persistent public Relay account state from `.relay/config/account.json`.
/// Returns an unauthenticated default account if no file exists.
pub fn load_relay_account(config_dir: &Path) -> RelayAccount {
    let path = get_account_metadata_path(config_dir);
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(account) = serde_json::from_str::<RelayAccount>(&content) {
                // Verify if tokens actually exist in secure store
                if account.authenticated && load_oauth_tokens(config_dir).is_none() {
                    // Tokens were cleared or missing; degrade safely to unauthenticated
                    let mut unauthed = account.clone();
                    unauthed.authenticated = false;
                    return unauthed;
                }
                return account;
            }
        }
    }
    RelayAccount::default()
}

/// Persists the public Relay account metadata to `.relay/config/account.json`.
/// Invariant: Secrets (tokens) are NEVER written to this file.
pub fn save_relay_account(config_dir: &Path, account: &RelayAccount) -> Result<(), String> {
    let path = get_account_metadata_path(config_dir);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(account)
        .map_err(|e| format!("Failed to serialize account metadata: {}", e))?;
    fs::write(path, json)
        .map_err(|e| format!("Failed to write account metadata file: {}", e))?;
    Ok(())
}

/// Performs Google Sign-In: initiates OAuth, saves secure tokens, updates account metadata,
/// and synchronizes account profile + installation metadata with the Supabase backend.
pub async fn sign_in_with_google(
    config_dir: &Path,
    custom_client_id: Option<String>,
    custom_client_secret: Option<String>,
    custom_supabase_url: Option<String>,
    custom_supabase_anon_key: Option<String>,
) -> Result<RelayAccount, String> {
    let (tokens, profile) = start_google_desktop_oauth(
        custom_client_id,
        custom_client_secret,
        custom_supabase_url.clone(),
        custom_supabase_anon_key.clone(),
        Some(RELAY_DEFAULT_SCOPES),
    )
    .await?;

    // Save tokens in secure store
    save_oauth_tokens(config_dir, &tokens)?;

    // Load or initialize existing account
    let existing = load_relay_account(config_dir);
    let now = Utc::now().to_rfc3339();

    let created_at = existing.created_at.or_else(|| Some(now.clone()));

    let updated_account = RelayAccount {
        authenticated: true,
        user_id: Some(profile.id),
        email: Some(profile.email),
        display_name: profile.name.or(existing.display_name),
        profile_image: profile.picture.or(existing.profile_image),
        provider: Some("google".to_string()),
        created_at,
        last_authenticated_at: Some(now),
        subscription: existing.subscription,
        account_mode: existing.account_mode,
        capabilities: vec![
            "local_vault".to_string(),
            "local_transcription".to_string(),
            "local_meetings".to_string(),
            "local_scribbles".to_string(),
            "google_calendar_sync".to_string(),
        ],
    };

    save_relay_account(config_dir, &updated_account)?;

    // Sync with Supabase Cloud backend (non-blocking / resilient to network offline)
    let supabase = SupabaseClient::new(None, None);
    let _ = supabase.sync_account_profile(&updated_account, Some(&tokens.access_token)).await;

    let inst = get_or_create_installation_info(config_dir, env!("CARGO_PKG_VERSION"));
    let _ = supabase
        .record_installation(&inst, updated_account.user_id.as_deref(), Some(&tokens.access_token))
        .await;

    Ok(updated_account)
}

/// Signs out the user: purges secure tokens and marks account as unauthenticated.
/// Critical Invariant: Never touches or deletes the local vault!
pub fn sign_out_account(config_dir: &Path) -> Result<RelayAccount, String> {
    delete_oauth_tokens(config_dir)?;

    let mut account = load_relay_account(config_dir);
    account.authenticated = false;
    account.user_id = None;
    account.email = None;
    account.display_name = None;
    account.profile_image = None;
    account.provider = None;
    account.last_authenticated_at = None;
    account.account_mode = AccountMode::Local;

    save_relay_account(config_dir, &account)?;
    Ok(account)
}

/// Permanently deletes the Relay Cloud account record and purges local secure tokens.
/// Invariant: Account ≠ Vault. The user's local markdown files, voice recordings,
/// meetings, and scribbles remain 100% untouched on this computer.
pub async fn delete_relay_account(config_dir: &Path) -> Result<RelayAccount, String> {
    let existing = load_relay_account(config_dir);
    let tokens = load_oauth_tokens(config_dir);

    // 1. Delete cloud profile in Supabase if authenticated
    if let (Some(user_id), Some(toks)) = (&existing.user_id, &tokens) {
        let supabase = SupabaseClient::new(None, None);
        let _ = supabase.delete_account_profile(user_id, &toks.access_token).await;
    }

    // 2. Wipe secure keyring credentials
    delete_oauth_tokens(config_dir)?;

    // 3. Reset local account metadata to anonymous default
    let reset_account = RelayAccount::default();
    save_relay_account(config_dir, &reset_account)?;

    Ok(reset_account)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_delete_relay_account_leaves_vault_intact() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_identity_{}", uuid::Uuid::new_v4()));
        let config_dir = temp_dir.join("config");
        let vault_dir = temp_dir.join("vault");
        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&vault_dir).unwrap();

        // 1. Create dummy local vault files (scribbles, meetings, audio)
        let note_path = vault_dir.join("test_note.md");
        fs::write(&note_path, "# User Local Note\nThis note must NEVER be deleted when an account is deleted.").unwrap();
        assert!(note_path.exists());

        // 2. Setup mock authenticated account
        let sample_account = RelayAccount {
            authenticated: true,
            user_id: Some("user_abc_123".to_string()),
            email: Some("user@example.com".to_string()),
            display_name: Some("Test User".to_string()),
            profile_image: None,
            provider: Some("google".to_string()),
            created_at: Some("2026-08-22T00:00:00Z".to_string()),
            last_authenticated_at: Some("2026-08-22T00:00:00Z".to_string()),
            subscription: SubscriptionInfo::default(),
            account_mode: AccountMode::Local,
            capabilities: vec!["local_vault".to_string()],
        };
        save_relay_account(&config_dir, &sample_account).unwrap();

        // 3. Save sample tokens
        let sample_tokens = OAuthTokens {
            access_token: "mock_jwt_token".to_string(),
            refresh_token: Some("mock_refresh_token".to_string()),
            token_type: "Bearer".to_string(),
            expires_at: Utc::now().timestamp() + 3600,
            scope: None,
        };
        save_oauth_tokens(&config_dir, &sample_tokens).unwrap();

        // 4. Delete account
        let reset = delete_relay_account(&config_dir).await.expect("Should delete account");
        assert!(!reset.authenticated);
        assert_eq!(reset.user_id, None);
        assert_eq!(reset.email, None);
        assert_eq!(reset.account_mode, AccountMode::Local);

        // 5. Invariant check: tokens cleared
        assert!(load_oauth_tokens(&config_dir).is_none());

        // 6. Critical Invariant Check: Local vault is 100% untouched!
        assert!(note_path.exists(), "Local vault files must NEVER be touched when an account is deleted");
        let content = fs::read_to_string(&note_path).unwrap();
        assert!(content.contains("User Local Note"));

        let _ = fs::remove_dir_all(temp_dir);
    }
}
