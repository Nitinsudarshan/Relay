use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_notes: Option<String>,
    pub minimum_supported_version: String,
    pub download_url: Option<String>,
    pub is_offline: bool,
}

pub struct UpdateService;

impl UpdateService {
    /// Compares semver-like version strings "x.y.z".
    pub fn is_newer_version(current: &str, candidate: &str) -> bool {
        let parse_parts = |v: &str| -> Vec<u32> {
            v.trim_start_matches('v')
                .split('.')
                .filter_map(|p| p.parse::<u32>().ok())
                .collect()
        };

        let curr_parts = parse_parts(current);
        let cand_parts = parse_parts(candidate);

        for (c, cand) in curr_parts.iter().zip(cand_parts.iter()) {
            if cand > c {
                return true;
            } else if cand < c {
                return false;
            }
        }

        cand_parts.len() > curr_parts.len()
    }

    /// Checks if a newer update is available.
    /// Checks Supabase app_releases table first, then GitHub, and degrades gracefully offline.
    pub async fn check_for_updates(current_version: &str) -> UpdateInfo {
        // 1. Try checking Supabase app_releases table
        let supabase = crate::identity::SupabaseClient::new(None, None);
        if let Some(info) = supabase.fetch_latest_release(current_version).await {
            return info;
        }

        // 2. Fallback to GitHub releases API
        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build()
        {
            Ok(c) => c,
            Err(_) => {
                return UpdateInfo {
                    current_version: current_version.to_string(),
                    latest_version: current_version.to_string(),
                    update_available: false,
                    release_notes: None,
                    minimum_supported_version: "0.8.0".to_string(),
                    download_url: None,
                    is_offline: true,
                };
            }
        };

        // Try checking latest release tag from GitHub or Relay endpoint
        let endpoint = "https://api.github.com/repos/Nitinsudarshan/Relay/releases/latest";
        let resp = client
            .get(endpoint)
            .header("User-Agent", "Relay-Desktop-App")
            .send()
            .await;

        match resp {
            Ok(res) if res.status().is_success() => {
                #[derive(Deserialize)]
                struct GhRelease {
                    tag_name: String,
                    body: Option<String>,
                    html_url: Option<String>,
                }
                if let Ok(gh) = res.json::<GhRelease>().await {
                    let latest = gh.tag_name.trim_start_matches('v').to_string();
                    let update_available = Self::is_newer_version(current_version, &latest);
                    return UpdateInfo {
                        current_version: current_version.to_string(),
                        latest_version: latest,
                        update_available,
                        release_notes: gh.body,
                        minimum_supported_version: "0.8.0".to_string(),
                        download_url: gh.html_url,
                        is_offline: false,
                    };
                }
            }
            _ => {
                // Offline or rate-limited; gracefully return up-to-date offline status
            }
        }

        UpdateInfo {
            current_version: current_version.to_string(),
            latest_version: current_version.to_string(),
            update_available: false,
            release_notes: Some("You are running the latest installed version of Relay.".to_string()),
            minimum_supported_version: "0.8.0".to_string(),
            download_url: None,
            is_offline: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(UpdateService::is_newer_version("0.8.2", "0.8.3"));
        assert!(UpdateService::is_newer_version("0.8.2", "0.9.0"));
        assert!(UpdateService::is_newer_version("0.8.2", "1.0.0"));
        assert!(!UpdateService::is_newer_version("0.8.2", "0.8.2"));
        assert!(!UpdateService::is_newer_version("0.8.3", "0.8.2"));
    }
}
