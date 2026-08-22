use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const KEYRING_SERVICE: &str = "com.relay.app";
const KEYRING_USER: &str = "google_oauth_tokens";
const FALLBACK_TOKEN_FILE: &str = "auth_tokens.bin";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub expires_at: i64, // Unix timestamp in seconds
    pub scope: Option<String>,
}

fn get_fallback_path(config_dir: &Path) -> PathBuf {
    config_dir.join(FALLBACK_TOKEN_FILE)
}

// Simple XOR obfuscation with a local key for the fallback file when keyring is unavailable
fn obfuscate_bytes(data: &[u8]) -> Vec<u8> {
    let key = b"relay_secure_auth_fallback_key_2026";
    data.iter()
        .enumerate()
        .map(|(i, &b)| b ^ key[i % key.len()])
        .collect()
}

pub fn save_oauth_tokens(config_dir: &Path, tokens: &OAuthTokens) -> Result<(), String> {
    let json = serde_json::to_string(tokens)
        .map_err(|e| format!("Failed to serialize tokens: {}", e))?;

    // 1. Attempt to store in OS Keyring / Credential Manager
    if let Ok(entry) = Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        if entry.set_password(&json).is_ok() {
            // If keyring succeeded, remove any leftover fallback file
            let fallback = get_fallback_path(config_dir);
            if fallback.exists() {
                let _ = fs::remove_file(fallback);
            }
            return Ok(());
        }
    }

    // 2. Fallback to obfuscated storage in .relay/config/
    let fallback = get_fallback_path(config_dir);
    if let Some(parent) = fallback.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let encrypted = obfuscate_bytes(json.as_bytes());
    fs::write(fallback, encrypted)
        .map_err(|e| format!("Failed to write secure tokens fallback: {}", e))?;

    Ok(())
}

pub fn load_oauth_tokens(config_dir: &Path) -> Option<OAuthTokens> {
    // 1. Check OS Keyring
    if let Ok(entry) = Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        if let Ok(password) = entry.get_password() {
            if let Ok(tokens) = serde_json::from_str::<OAuthTokens>(&password) {
                return Some(tokens);
            }
        }
    }

    // 2. Check fallback file
    let fallback = get_fallback_path(config_dir);
    if fallback.exists() {
        if let Ok(bytes) = fs::read(&fallback) {
            let decrypted = obfuscate_bytes(&bytes);
            if let Ok(json_str) = String::from_utf8(decrypted) {
                if let Ok(tokens) = serde_json::from_str::<OAuthTokens>(&json_str) {
                    return Some(tokens);
                }
            }
        }
    }

    None
}

pub fn delete_oauth_tokens(config_dir: &Path) -> Result<(), String> {
    // 1. Delete from keyring
    if let Ok(entry) = Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        let _ = entry.delete_password();
    }

    // 2. Delete from fallback file
    let fallback = get_fallback_path(config_dir);
    if fallback.exists() {
        let _ = fs::remove_file(fallback);
    }

    Ok(())
}
