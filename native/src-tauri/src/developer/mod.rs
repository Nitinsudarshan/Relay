use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const DEV_SETTINGS_FILE: &str = "dev_settings.json";

fn get_dev_settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join(DEV_SETTINGS_FILE)
}

/// Developer-only settings (testing & debugging overrides).
///
/// Removability Invariant:
/// This struct and module are isolated. Deleting this module in the future
/// does not touch RelayProfile, display_name, Google authentication, local vaults,
/// or production configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeveloperSettings {
    /// When true, forces the onboarding wizard to display on every launch
    /// without deleting or resetting user data, vault notes, or OAuth tokens.
    #[serde(default)]
    pub force_onboarding_on_launch: bool,
}

pub fn load_developer_settings(config_dir: &Path) -> DeveloperSettings {
    let path = get_dev_settings_path(config_dir);
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<DeveloperSettings>(&content) {
                return settings;
            }
        }
    }
    DeveloperSettings::default()
}

pub fn save_developer_settings(config_dir: &Path, settings: &DeveloperSettings) -> Result<(), String> {
    let path = get_dev_settings_path(config_dir);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize developer settings: {}", e))?;
    fs::write(path, json)
        .map_err(|e| format!("Failed to write developer settings file: {}", e))?;
    Ok(())
}

pub fn set_force_onboarding(config_dir: &Path, force: bool) -> Result<DeveloperSettings, String> {
    let mut settings = load_developer_settings(config_dir);
    settings.force_onboarding_on_launch = force;
    save_developer_settings(config_dir, &settings)?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_developer_settings_persistence() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_dev_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        // 1. Initial state defaults to false
        let initial = load_developer_settings(&temp_dir);
        assert!(!initial.force_onboarding_on_launch);

        // 2. Set to true and verify persistence
        let updated = set_force_onboarding(&temp_dir, true).expect("Should save dev settings");
        assert!(updated.force_onboarding_on_launch);

        let reloaded = load_developer_settings(&temp_dir);
        assert!(reloaded.force_onboarding_on_launch);

        // 3. Set back to false
        let toggled_off = set_force_onboarding(&temp_dir, false).expect("Should toggle off");
        assert!(!toggled_off.force_onboarding_on_launch);

        let _ = fs::remove_dir_all(temp_dir);
    }
}
