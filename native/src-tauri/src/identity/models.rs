use serde::{Deserialize, Serialize};

/// Mode of operation for the Relay account.
/// The account mode is completely decoupled from the local vault.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AccountMode {
    #[default]
    Local,
    Hybrid,
}

/// Plan type for subscription scaffolding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionPlan {
    #[default]
    Free,
    Hybrid,
}

/// Subscription state abstraction (future-ready scaffolding without active billing).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionInfo {
    pub plan: SubscriptionPlan,
    pub status: String,
    pub renewal_date: Option<String>,
    pub capabilities: Vec<String>,
}

impl Default for SubscriptionInfo {
    fn default() -> Self {
        Self {
            plan: SubscriptionPlan::Free,
            status: "active".to_string(),
            renewal_date: None,
            capabilities: vec![
                "local_vault".to_string(),
                "local_transcription".to_string(),
                "local_meetings".to_string(),
                "local_scribbles".to_string(),
                "google_calendar_sync".to_string(),
            ],
        }
    }
}

/// Central Relay Account model.
///
/// Invariant: Account != Local Vault.
/// An authenticated RelayAccount identifies the user and installation, but does NOT
/// upload or sync any local notes, recordings, audio, scribbles, or meetings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayAccount {
    pub authenticated: bool,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub profile_image: Option<String>,
    pub provider: Option<String>,
    pub created_at: Option<String>,
    pub last_authenticated_at: Option<String>,
    pub subscription: SubscriptionInfo,
    pub account_mode: AccountMode,
    pub capabilities: Vec<String>,
}

impl Default for RelayAccount {
    fn default() -> Self {
        Self {
            authenticated: false,
            user_id: None,
            email: None,
            display_name: None,
            profile_image: None,
            provider: None,
            created_at: None,
            last_authenticated_at: None,
            subscription: SubscriptionInfo::default(),
            account_mode: AccountMode::Local,
            capabilities: vec![
                "local_vault".to_string(),
                "local_transcription".to_string(),
                "local_meetings".to_string(),
                "local_scribbles".to_string(),
            ],
        }
    }
}

/// Stable anonymous installation metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallationInfo {
    pub installation_id: String,
    pub first_installed_at: String,
    pub platform: String,
    pub os_version: String,
    pub app_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_account_is_unauthenticated_and_local() {
        let account = RelayAccount::default();
        assert!(!account.authenticated);
        assert_eq!(account.account_mode, AccountMode::Local);
        assert_eq!(account.subscription.plan, SubscriptionPlan::Free);
    }
}
