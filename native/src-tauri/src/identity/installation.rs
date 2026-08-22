use crate::identity::models::InstallationInfo;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const INSTALLATION_FILE_NAME: &str = "installation.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstallationFileSchema {
    pub installation_id: String,
    pub first_installed_at: String,
}

pub fn get_installation_file_path(config_dir: &Path) -> PathBuf {
    config_dir.join(INSTALLATION_FILE_NAME)
}

/// Retrieves or initializes a stable, anonymous installation identity.
/// This ID is generated once per installation, persists across updates,
/// and is never tied to hardware identifiers.
pub fn get_or_create_installation_info(config_dir: &Path, app_version: &str) -> InstallationInfo {
    let path = get_installation_file_path(config_dir);

    let (id, installed_at) = if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(schema) = serde_json::from_str::<InstallationFileSchema>(&content) {
                (schema.installation_id, schema.first_installed_at)
            } else {
                generate_and_save(&path)
            }
        } else {
            generate_and_save(&path)
        }
    } else {
        generate_and_save(&path)
    };

    let platform = std::env::consts::OS.to_string();
    let os_version = std::env::consts::ARCH.to_string();

    InstallationInfo {
        installation_id: id,
        first_installed_at: installed_at,
        platform,
        os_version,
        app_version: app_version.to_string(),
    }
}

fn generate_and_save(path: &Path) -> (String, String) {
    let new_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let schema = InstallationFileSchema {
        installation_id: new_id.clone(),
        first_installed_at: now.clone(),
    };

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if let Ok(json) = serde_json::to_string_pretty(&schema) {
        let _ = fs::write(path, json);
    }

    (new_id, now)
}

/// Helper to mask an installation ID for UI display (e.g. "••••••••-••••-5a9f")
pub fn mask_installation_id(id: &str) -> String {
    if id.len() <= 4 {
        return "••••".to_string();
    }
    let suffix = &id[id.len() - 4..];
    format!("••••••••-••••-{}", suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_installation_id_persists() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_{}", Uuid::new_v4()));
        let _ = fs::create_dir_all(&temp_dir);

        let info1 = get_or_create_installation_info(&temp_dir, "0.8.2");
        let info2 = get_or_create_installation_info(&temp_dir, "0.8.2");

        assert_eq!(info1.installation_id, info2.installation_id);
        assert_eq!(info1.first_installed_at, info2.first_installed_at);
        assert!(!info1.installation_id.is_empty());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_mask_installation_id() {
        let id = "550e8400-e29b-41d4-a716-446655440000";
        let masked = mask_installation_id(id);
        assert_eq!(masked, "••••••••-••••-0000");
    }
}
