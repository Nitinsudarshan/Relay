use serde::Serialize;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::sleep;

/// Result of trying to make a locally-configured Ollama backend usable
/// without asking the user to open a terminal and run `ollama serve`
/// themselves first.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum OllamaStatus {
    /// Already running — nothing to do.
    Running,
    /// Wasn't running; Relay spawned `ollama serve` itself and it's now up.
    Started,
    /// The `ollama` binary isn't on PATH — Relay can manage the *process*,
    /// but can't conjure the install itself.
    NotInstalled,
    /// A non-local host was configured (or a local one that never came up).
    Unreachable { message: String },
}

const READY_POLL_ATTEMPTS: u32 = 10;
const READY_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Makes sure an Ollama server is reachable at `host`, spawning one
/// ourselves if it's configured as local and just isn't running yet, then
/// kicks off a background pull of `model` if it isn't already present.
/// This is the "bundled" experience for local mode: the user installs
/// Ollama once, and Relay takes care of starting it and fetching models —
/// no manual `ollama serve` / `ollama pull` required.
pub async fn ensure_ollama_ready(host: &str, model: &str) -> OllamaStatus {
    if ping(host).await {
        spawn_background_pull(host, model);
        return OllamaStatus::Running;
    }

    if !is_local_host(host) {
        return OllamaStatus::Unreachable {
            message: format!("{} is unreachable", host),
        };
    }

    match Command::new("ollama")
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(_) => {
            for _ in 0..READY_POLL_ATTEMPTS {
                sleep(READY_POLL_INTERVAL).await;
                if ping(host).await {
                    spawn_background_pull(host, model);
                    return OllamaStatus::Started;
                }
            }
            OllamaStatus::Unreachable {
                message: "Started `ollama serve` but it didn't respond in time".to_string(),
            }
        }
        // NotFound is by far the common case (binary missing); still treat
        // any spawn failure as "not installed" — retrying won't help.
        Err(_) => OllamaStatus::NotInstalled,
    }
}

async fn ping(host: &str) -> bool {
    reqwest::Client::new()
        .get(format!("{}/api/version", host))
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

fn is_local_host(host: &str) -> bool {
    host.contains("localhost") || host.contains("127.0.0.1")
}

/// Fire-and-forget: if `model` isn't already pulled, ask Ollama to pull it.
/// Runs detached from the caller so `ensure_ollama_ready` doesn't block on a
/// multi-gigabyte download every time it's called.
fn spawn_background_pull(host: &str, model: &str) {
    let host = host.to_string();
    let model = model.to_string();
    tauri::async_runtime::spawn(async move {
        let already_present = model_is_present(&host, &model).await;
        if already_present {
            return;
        }

        tracing::info!("Pulling Ollama model '{}' in the background…", model);
        let client = reqwest::Client::new();
        let result = client
            .post(format!("{}/api/pull", host))
            .json(&serde_json::json!({ "name": model, "stream": false }))
            .timeout(Duration::from_secs(20 * 60))
            .send()
            .await;

        match result {
            Ok(res) if res.status().is_success() => {
                tracing::info!("Ollama model '{}' is ready", model);
            }
            Ok(res) => tracing::warn!("Ollama pull for '{}' failed: HTTP {}", model, res.status()),
            Err(e) => tracing::warn!("Ollama pull for '{}' failed: {}", model, e),
        }
    });
}

async fn model_is_present(host: &str, model: &str) -> bool {
    let Ok(res) = reqwest::Client::new()
        .get(format!("{}/api/tags", host))
        .timeout(Duration::from_secs(5))
        .send()
        .await
    else {
        return false;
    };
    let Ok(json) = res.json::<serde_json::Value>().await else {
        return false;
    };
    json["models"]
        .as_array()
        .map(|models| {
            models
                .iter()
                .any(|m| m["name"].as_str() == Some(model))
        })
        .unwrap_or(false)
}
