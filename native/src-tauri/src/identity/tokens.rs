pub use crate::oauth::{KeyringTokenStore, OAuthTokens, TokenNamespace};
use std::path::Path;

pub fn save_oauth_tokens(config_dir: &Path, tokens: &OAuthTokens) -> Result<(), String> {
    KeyringTokenStore::save(config_dir, TokenNamespace::Identity, tokens)
}

pub fn load_oauth_tokens(config_dir: &Path) -> Option<OAuthTokens> {
    KeyringTokenStore::load(config_dir, TokenNamespace::Identity)
}

pub fn delete_oauth_tokens(config_dir: &Path) -> Result<(), String> {
    KeyringTokenStore::delete(config_dir, TokenNamespace::Identity)
}
