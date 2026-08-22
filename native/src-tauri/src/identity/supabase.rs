use crate::diagnostics::DiagnosticPayload;
use crate::identity::models::{InstallationInfo, RelayAccount};
use crate::updates::UpdateInfo;
use chrono::Utc;
use serde::{Deserialize, Serialize};

// Relay Cloud Supabase defaults (can be overridden via settings or environment)
pub const DEFAULT_SUPABASE_URL: &str = "https://app.relay.local"; // Fallback URL or env variable
pub const DEFAULT_SUPABASE_ANON_KEY: &str = "relay_anon_key_placeholder";

#[derive(Debug, Clone)]
pub struct SupabaseConfig {
    pub url: String,
    pub anon_key: String,
}

impl Default for SupabaseConfig {
    fn default() -> Self {
        let url = std::env::var("RELAY_SUPABASE_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_SUPABASE_URL.to_string());

        let anon_key = std::env::var("RELAY_SUPABASE_ANON_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_SUPABASE_ANON_KEY.to_string());

        Self { url, anon_key }
    }
}

pub struct SupabaseClient {
    config: SupabaseConfig,
    client: reqwest::Client,
}

impl SupabaseClient {
    pub fn new(custom_url: Option<String>, custom_anon_key: Option<String>) -> Self {
        let mut config = SupabaseConfig::default();
        if let Some(u) = custom_url.filter(|s| !s.trim().is_empty()) {
            config.url = u;
        }
        if let Some(k) = custom_anon_key.filter(|s| !s.trim().is_empty()) {
            config.anon_key = k;
        }

        Self {
            config,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(4))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Upserts user account profile to Supabase `relay_accounts` table.
    pub async fn sync_account_profile(
        &self,
        account: &RelayAccount,
        jwt: Option<&str>,
    ) -> Result<(), String> {
        let user_id = match &account.user_id {
            Some(id) => id,
            None => return Ok(()), // Unauthenticated
        };

        let endpoint = format!("{}/rest/v1/relay_accounts", self.config.url.trim_end_matches('/'));

        #[derive(Serialize)]
        struct AccountRow<'a> {
            id: &'a str,
            email: Option<&'a str>,
            display_name: Option<&'a str>,
            profile_image: Option<&'a str>,
            provider: Option<&'a str>,
            account_mode: &'a str,
            subscription_plan: &'a str,
            subscription_status: &'a str,
            updated_at: String,
        }

        let mode_str = match account.account_mode {
            crate::identity::models::AccountMode::Local => "local",
            crate::identity::models::AccountMode::Hybrid => "hybrid",
        };
        let plan_str = match account.subscription.plan {
            crate::identity::models::SubscriptionPlan::Free => "free",
            crate::identity::models::SubscriptionPlan::Hybrid => "hybrid",
        };

        let row = AccountRow {
            id: user_id.as_str(),
            email: account.email.as_deref(),
            display_name: account.display_name.as_deref(),
            profile_image: account.profile_image.as_deref(),
            provider: account.provider.as_deref(),
            account_mode: mode_str,
            subscription_plan: plan_str,
            subscription_status: &account.subscription.status,
            updated_at: Utc::now().to_rfc3339(),
        };

        let mut req = self
            .client
            .post(&endpoint)
            .header("apikey", &self.config.anon_key)
            .header("Prefer", "resolution=merge-duplicates")
            .json(&row);

        if let Some(token) = jwt {
            req = req.bearer_auth(token);
        } else {
            req = req.bearer_auth(&self.config.anon_key);
        }

        let resp = req.send().await.map_err(|e| format!("Supabase account sync error: {}", e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Supabase sync failed (HTTP {}): {}", status, body));
        }

        Ok(())
    }

    /// Upserts installation record via secure RPC function `register_installation_heartbeat`.
    pub async fn record_installation(
        &self,
        installation: &InstallationInfo,
        _user_id: Option<&str>,
        jwt: Option<&str>,
    ) -> Result<(), String> {
        let endpoint = format!(
            "{}/rest/v1/rpc/register_installation_heartbeat",
            self.config.url.trim_end_matches('/')
        );

        #[derive(Serialize)]
        struct HeartbeatParams<'a> {
            p_installation_id: &'a str,
            p_app_version: &'a str,
            p_platform: &'a str,
            p_os_version: &'a str,
        }

        let params = HeartbeatParams {
            p_installation_id: &installation.installation_id,
            p_app_version: &installation.app_version,
            p_platform: &installation.platform,
            p_os_version: &installation.os_version,
        };

        let mut req = self
            .client
            .post(&endpoint)
            .header("apikey", &self.config.anon_key)
            .json(&params);

        if let Some(token) = jwt {
            req = req.bearer_auth(token);
        } else {
            req = req.bearer_auth(&self.config.anon_key);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("Supabase installation heartbeat error: {}", e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Supabase installation heartbeat failed (HTTP {}): {}", status, body));
        }

        Ok(())
    }

    /// Dispatches privacy-safe diagnostic event via secure RPC function `ingest_diagnostic_event`.
    pub async fn send_diagnostic_event(
        &self,
        payload: &DiagnosticPayload,
        jwt: Option<&str>,
    ) -> Result<(), String> {
        let endpoint = format!(
            "{}/rest/v1/rpc/ingest_diagnostic_event",
            self.config.url.trim_end_matches('/')
        );

        #[derive(Serialize)]
        struct IngestParams<'a> {
            p_installation_id: &'a str,
            p_relay_version: &'a str,
            p_platform: &'a str,
            p_os_version: &'a str,
            p_event_type: &'a str,
            p_metadata: &'a std::collections::HashMap<String, String>,
        }

        let params = IngestParams {
            p_installation_id: &payload.installation_id,
            p_relay_version: &payload.relay_version,
            p_platform: &payload.platform,
            p_os_version: &payload.os_version,
            p_event_type: &payload.event_type,
            p_metadata: &payload.metadata,
        };

        let mut req = self
            .client
            .post(&endpoint)
            .header("apikey", &self.config.anon_key)
            .json(&params);

        if let Some(token) = jwt {
            req = req.bearer_auth(token);
        } else {
            req = req.bearer_auth(&self.config.anon_key);
        }

        let resp = req.send().await.map_err(|e| format!("Supabase telemetry post error: {}", e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Supabase telemetry post failed (HTTP {}): {}", status, body));
        }

        Ok(())
    }

    /// Deletes the cloud account record from `relay_accounts`.
    pub async fn delete_account_profile(&self, user_id: &str, jwt: &str) -> Result<(), String> {
        let endpoint = format!(
            "{}/rest/v1/relay_accounts?id=eq.{}",
            self.config.url.trim_end_matches('/'),
            user_id
        );

        let resp = self
            .client
            .delete(&endpoint)
            .header("apikey", &self.config.anon_key)
            .bearer_auth(jwt)
            .send()
            .await
            .map_err(|e| format!("Supabase delete account request error: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Supabase delete account failed (HTTP {}): {}", status, body));
        }

        Ok(())
    }

    /// Queries latest release from Supabase `app_releases` table.
    pub async fn fetch_latest_release(&self, current_version: &str) -> Option<UpdateInfo> {
        let endpoint = format!(
            "{}/rest/v1/app_releases?is_active=eq.true&order=published_at.desc&limit=1",
            self.config.url.trim_end_matches('/')
        );

        #[derive(Deserialize)]
        struct SupabaseRelease {
            version: String,
            min_supported_version: Option<String>,
            release_notes: Option<String>,
            download_url: Option<String>,
        }

        let resp = self
            .client
            .get(&endpoint)
            .header("apikey", &self.config.anon_key)
            .bearer_auth(&self.config.anon_key)
            .send()
            .await
            .ok()?;

        if !resp.status().is_success() {
            return None;
        }

        let releases: Vec<SupabaseRelease> = resp.json().await.ok()?;
        let latest = releases.into_iter().next()?;

        let update_available = crate::updates::UpdateService::is_newer_version(current_version, &latest.version);

        Some(UpdateInfo {
            current_version: current_version.to_string(),
            latest_version: latest.version,
            update_available,
            release_notes: latest.release_notes,
            minimum_supported_version: latest.min_supported_version.unwrap_or_else(|| "0.8.0".to_string()),
            download_url: latest.download_url,
            is_offline: false,
        })
    }
}
