//! Multiple calendar accounts management.
//!
//! Allows connecting multiple Google Calendar accounts (e.g. Work, Personal, School),
//! each with a user-defined name, custom color, enabled state, and secure tokens.

use crate::oauth::{KeyringTokenStore, OAuthTokens, TokenNamespace};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const CALENDARS_FILE: &str = "calendars.json";
pub const DEFAULT_COLORS: [&str; 8] = [
    "#3b82f6", // Blue (Work)
    "#10b981", // Emerald (Personal)
    "#8b5cf6", // Purple (School)
    "#f59e0b", // Amber
    "#ef4444", // Red
    "#06b6d4", // Cyan
    "#ec4899", // Pink
    "#84cc16", // Lime
];

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarAccount {
    pub id: String,
    pub name: String,
    pub color: String,
    #[serde(default)]
    pub account_email: Option<String>,
    #[serde(default)]
    pub account_name: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub last_synced_at: Option<String>,
}

impl CalendarAccount {
    pub fn new(name: String, color: Option<String>, tokens: &OAuthTokens) -> Self {
        let id = format!("cal_{}", Uuid::new_v4().simple());
        let color = color.unwrap_or_else(|| DEFAULT_COLORS[0].to_string());
        Self {
            id,
            name,
            color,
            account_email: tokens.account_email.clone(),
            account_name: tokens.account_name.clone(),
            enabled: true,
            last_synced_at: tokens.last_synced_at.clone(),
        }
    }

    pub fn service_name(&self) -> &'static str {
        "com.relay.app.calendar"
    }

    pub fn username(&self) -> String {
        format!("google_calendar_{}", self.id)
    }

    pub fn fallback_filename(&self) -> String {
        format!("calendar_tokens_{}.bin", self.id)
    }

    pub fn save_tokens(&self, config_dir: &Path, tokens: &OAuthTokens) -> Result<(), String> {
        KeyringTokenStore::save_explicit(
            config_dir,
            self.service_name(),
            &self.username(),
            &self.fallback_filename(),
            tokens,
        )
    }

    pub fn load_tokens(&self, config_dir: &Path) -> Option<OAuthTokens> {
        KeyringTokenStore::load_explicit(
            config_dir,
            self.service_name(),
            &self.username(),
            &self.fallback_filename(),
        )
    }

    pub fn delete_tokens(&self, config_dir: &Path) -> Result<(), String> {
        KeyringTokenStore::delete_explicit(
            config_dir,
            self.service_name(),
            &self.username(),
            &self.fallback_filename(),
        )
    }
}

fn calendars_path(config_dir: &Path) -> PathBuf {
    config_dir.join(CALENDARS_FILE)
}

/// Loads all configured calendar accounts.
/// Automatically migrates any legacy single calendar store if `calendars.json` is missing.
pub fn load_calendar_accounts(config_dir: &Path) -> Vec<CalendarAccount> {
    let path = calendars_path(config_dir);
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(accounts) = serde_json::from_str::<Vec<CalendarAccount>>(&content) {
                return accounts;
            }
        }
    }

    // Auto-migrate legacy single calendar if tokens exist
    if let Some(legacy_tokens) = KeyringTokenStore::load(config_dir, TokenNamespace::Calendar) {
        let name = legacy_tokens
            .account_name
            .clone()
            .unwrap_or_else(|| "Personal".to_string());
        let migrated = CalendarAccount {
            id: "primary".to_string(),
            name,
            color: DEFAULT_COLORS[0].to_string(),
            account_email: legacy_tokens.account_email.clone(),
            account_name: legacy_tokens.account_name.clone(),
            enabled: true,
            last_synced_at: legacy_tokens.last_synced_at.clone(),
        };

        let _ = migrated.save_tokens(config_dir, &legacy_tokens);
        let list = vec![migrated];
        let _ = save_calendar_accounts(config_dir, &list);
        return list;
    }

    Vec::new()
}

pub fn save_calendar_accounts(config_dir: &Path, accounts: &[CalendarAccount]) -> Result<(), String> {
    let path = calendars_path(config_dir);
    let json = serde_json::to_string_pretty(accounts)
        .map_err(|e| format!("Failed to serialize calendar accounts: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write calendars.json: {}", e))?;
    Ok(())
}

pub fn add_calendar_account(
    config_dir: &Path,
    name: String,
    color: Option<String>,
    tokens: &OAuthTokens,
) -> Result<CalendarAccount, String> {
    let mut accounts = load_calendar_accounts(config_dir);
    let assigned_color = color.unwrap_or_else(|| {
        let idx = accounts.len() % DEFAULT_COLORS.len();
        DEFAULT_COLORS[idx].to_string()
    });

    let account = CalendarAccount::new(name, Some(assigned_color), tokens);
    account.save_tokens(config_dir, tokens)?;
    accounts.push(account.clone());
    save_calendar_accounts(config_dir, &accounts)?;
    Ok(account)
}

pub fn update_calendar_account(
    config_dir: &Path,
    id: &str,
    name: Option<String>,
    color: Option<String>,
    enabled: Option<bool>,
) -> Result<CalendarAccount, String> {
    let mut accounts = load_calendar_accounts(config_dir);
    let pos = accounts
        .iter()
        .position(|a| a.id == id)
        .ok_or_else(|| format!("Calendar account not found: {}", id))?;

    if let Some(n) = name {
        if !n.trim().is_empty() {
            accounts[pos].name = n.trim().to_string();
        }
    }
    if let Some(c) = color {
        accounts[pos].color = c;
    }
    if let Some(en) = enabled {
        accounts[pos].enabled = en;
    }

    save_calendar_accounts(config_dir, &accounts)?;
    Ok(accounts[pos].clone())
}

pub fn delete_calendar_account(config_dir: &Path, id: &str) -> Result<Vec<CalendarAccount>, String> {
    let mut accounts = load_calendar_accounts(config_dir);
    if let Some(pos) = accounts.iter().position(|a| a.id == id) {
        let removed = accounts.remove(pos);
        let _ = removed.delete_tokens(config_dir);
        save_calendar_accounts(config_dir, &accounts)?;
    }
    Ok(accounts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calendar_accounts_lifecycle() {
        let unique_dir = std::env::temp_dir().join(format!("relay_cal_test_{}", Uuid::new_v4().simple()));
        let _ = fs::create_dir_all(&unique_dir);
        let config_dir = &unique_dir;

        let initial_count = load_calendar_accounts(config_dir).len();

        let tokens = OAuthTokens {
            access_token: "test_access".to_string(),
            refresh_token: Some("test_refresh".to_string()),
            token_type: "Bearer".to_string(),
            expires_at: 9999999999,
            scope: None,
            account_email: Some("work@example.com".to_string()),
            account_name: Some("Work Account".to_string()),
            last_synced_at: None,
        };

        let added = add_calendar_account(config_dir, "Work Test".to_string(), Some("#3b82f6".to_string()), &tokens).unwrap();
        assert_eq!(added.name, "Work Test");
        assert_eq!(added.color, "#3b82f6");
        assert_eq!(added.account_email.as_deref(), Some("work@example.com"));

        let loaded = load_calendar_accounts(config_dir);
        assert_eq!(loaded.len(), initial_count + 1);

        // Update name and color
        let updated = update_calendar_account(
            config_dir,
            &added.id,
            Some("Acme Work".to_string()),
            Some("#ef4444".to_string()),
            Some(false),
        )
        .unwrap();
        assert_eq!(updated.name, "Acme Work");
        assert_eq!(updated.color, "#ef4444");
        assert!(!updated.enabled);

        // Delete
        let remaining = delete_calendar_account(config_dir, &added.id).unwrap();
        assert_eq!(remaining.len(), initial_count);

        let _ = fs::remove_dir_all(&unique_dir);
    }
}
