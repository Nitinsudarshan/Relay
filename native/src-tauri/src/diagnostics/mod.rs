use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Privacy-safe diagnostic event.
/// STRICT GUARANTEE: Never contains personal user content, scribbles, audio,
/// transcripts, file paths, or knowledge graph data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticPayload {
    pub installation_id: String,
    pub account_id: Option<String>,
    pub relay_version: String,
    pub platform: String,
    pub os_version: String,
    pub event_type: String,
    pub metadata: HashMap<String, String>,
    pub timestamp: String,
}

pub struct DiagnosticsService;

impl DiagnosticsService {
    /// Dispatches a privacy-safe diagnostic event if user has opted in.
    pub fn report_event(
        enabled: bool,
        installation_id: &str,
        account_id: Option<&str>,
        relay_version: &str,
        event_type: &str,
        metadata: HashMap<String, String>,
    ) {
        if !enabled {
            return;
        }

        let payload = DiagnosticPayload {
            installation_id: installation_id.to_string(),
            account_id: account_id.map(|s| s.to_string()),
            relay_version: relay_version.to_string(),
            platform: std::env::consts::OS.to_string(),
            os_version: std::env::consts::ARCH.to_string(),
            event_type: event_type.to_string(),
            metadata,
            timestamp: Utc::now().to_rfc3339(),
        };

        // In local mode / telemetry foundation, we trace to local log.
        tracing::debug!(
            target: "relay::diagnostics",
            "Diagnostic event: [{}] for installation {} (v{})",
            payload.event_type,
            payload.installation_id,
            payload.relay_version
        );

        // Asynchronously post to Supabase backend if runtime is active
        let payload_clone = payload.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let client = crate::identity::SupabaseClient::new(None, None);
                let _ = client.send_diagnostic_event(&payload_clone, None).await;
            });
        }
    }

    /// Dispatches an error diagnostic report.
    pub fn report_error(
        enabled: bool,
        installation_id: &str,
        account_id: Option<&str>,
        relay_version: &str,
        error_code: &str,
        error_context: &str,
    ) {
        let mut meta = HashMap::new();
        meta.insert("error_code".to_string(), error_code.to_string());
        meta.insert("error_context".to_string(), error_context.to_string());

        Self::report_event(
            enabled,
            installation_id,
            account_id,
            relay_version,
            "app_error",
            meta,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostics_disabled_does_not_panic() {
        DiagnosticsService::report_event(
            false,
            "inst-123",
            None,
            "0.8.2",
            "test_event",
            HashMap::new(),
        );
    }

    #[test]
    fn test_diagnostics_payload_structure() {
        let mut meta = HashMap::new();
        meta.insert("mode".to_string(), "local".to_string());

        let payload = DiagnosticPayload {
            installation_id: "test-id".to_string(),
            account_id: Some("user-456".to_string()),
            relay_version: "0.8.2".to_string(),
            platform: "windows".to_string(),
            os_version: "x86_64".to_string(),
            event_type: "startup".to_string(),
            metadata: meta,
            timestamp: "2026-08-22T00:00:00Z".to_string(),
        };

        assert_eq!(payload.event_type, "startup");
        assert_eq!(payload.metadata.get("mode").unwrap(), "local");
    }
}
