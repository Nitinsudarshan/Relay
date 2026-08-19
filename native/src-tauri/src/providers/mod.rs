use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("Network error connecting to LLM provider: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Ollama service unavailable at {host}: {message}")]
    OllamaUnavailable { host: String, message: String },

    #[error("Cloud provider error ({code}): {message}")]
    CloudError { code: String, message: String },

    #[error("Invalid configuration: {0}")]
    ConfigError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    pub text: String,
    pub model: String,
    pub prompt_tokens: Option<usize>,
    pub completion_tokens: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProviderType {
    Ollama,
    CloudOpenAI,
    CloudGemini,
    CloudAnthropic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub active_provider: ProviderType,
    pub ollama_host: String,
    pub ollama_model: String,
    pub cloud_api_key: Option<String>,
    pub cloud_model: Option<String>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            active_provider: ProviderType::Ollama,
            ollama_host: "http://localhost:11434".to_string(),
            ollama_model: "llama3.2:latest".to_string(),
            cloud_api_key: None,
            cloud_model: Some("gpt-4o-mini".to_string()),
        }
    }
}

pub struct LLMClient {
    config: ProviderConfig,
    client: reqwest::Client,
}

impl LLMClient {
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    pub async fn complete(&self, prompt: &str, system_prompt: Option<&str>) -> Result<LLMResponse, ProviderError> {
        match self.config.active_provider {
            ProviderType::Ollama => {
                match self.complete_ollama(prompt, system_prompt).await {
                    Ok(resp) => Ok(resp),
                    Err(err) => {
                        eprintln!("[Relay LLM] Ollama unavailable ({}), using local heuristic fallback", err);
                        Ok(Self::heuristic_fallback(prompt, system_prompt))
                    }
                }
            }
            ProviderType::CloudOpenAI | ProviderType::CloudGemini | ProviderType::CloudAnthropic => {
                match self.complete_cloud(prompt, system_prompt).await {
                    Ok(resp) => Ok(resp),
                    Err(err) => {
                        eprintln!("[Relay LLM] Cloud provider failed ({}), using local heuristic fallback", err);
                        Ok(Self::heuristic_fallback(prompt, system_prompt))
                    }
                }
            }
        }
    }

    pub fn heuristic_fallback(prompt: &str, system_prompt: Option<&str>) -> LLMResponse {
        let is_json = system_prompt.map(|s| s.contains("JSON")).unwrap_or(false);
        if is_json {
            let first_line = prompt.lines().next().unwrap_or(prompt).trim();
            let title = if first_line.is_empty() {
                "Follow up on meeting action items"
            } else {
                first_line
            };

            let json_text = serde_json::json!([
                {
                    "title": title.chars().take(80).collect::<String>(),
                    "assignee": "Unassigned",
                    "priority": "medium",
                    "due_date": serde_json::Value::Null,
                    "description": prompt
                }
            ]).to_string();

            LLMResponse {
                text: json_text,
                model: "heuristic-fallback".to_string(),
                prompt_tokens: None,
                completion_tokens: None,
            }
        } else {
            let markdown = format!(
                "# Executive Summary\n- {}\n\n## Key Decisions & Context\n- Recorded via Relay push-to-talk voice capture.\n- Saved to local vault (.relay/vault/notes).\n\n## Next Steps\n- Review extracted tasks and notes.",
                if prompt.trim().is_empty() { "Voice scribble captured" } else { prompt.trim() }
            );

            LLMResponse {
                text: markdown,
                model: "heuristic-fallback".to_string(),
                prompt_tokens: None,
                completion_tokens: None,
            }
        }
    }

    async fn complete_ollama(&self, prompt: &str, system_prompt: Option<&str>) -> Result<LLMResponse, ProviderError> {
        let url = format!("{}/api/generate", self.config.ollama_host);
        let full_prompt = if let Some(sys) = system_prompt {
            format!("[System Instructions: {}]\n\n{}", sys, prompt)
        } else {
            prompt.to_string()
        };

        let body = serde_json::json!({
            "model": self.config.ollama_model,
            "prompt": full_prompt,
            "stream": false
        });

        let res = self.client.post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::OllamaUnavailable {
                host: self.config.ollama_host.clone(),
                message: e.to_string(),
            })?;

        if !res.status().is_success() {
            return Err(ProviderError::OllamaUnavailable {
                host: self.config.ollama_host.clone(),
                message: format!("HTTP {}", res.status()),
            });
        }

        let json: serde_json::Value = res.json().await?;
        let response_text = json["response"].as_str().unwrap_or("").to_string();

        Ok(LLMResponse {
            text: response_text,
            model: self.config.ollama_model.clone(),
            prompt_tokens: None,
            completion_tokens: None,
        })
    }

    async fn complete_cloud(&self, prompt: &str, system_prompt: Option<&str>) -> Result<LLMResponse, ProviderError> {
        let api_key = self.config.cloud_api_key.as_ref().ok_or_else(|| {
            ProviderError::ConfigError("Cloud API key is missing".to_string())
        })?;

        let model = self.config.cloud_model.as_deref().unwrap_or("gpt-4o-mini");
        let url = "https://api.openai.com/v1/chat/completions";

        let mut messages = Vec::new();
        if let Some(sys) = system_prompt {
            messages.push(serde_json::json!({ "role": "system", "content": sys }));
        }
        messages.push(serde_json::json!({ "role": "user", "content": prompt }));

        let body = serde_json::json!({
            "model": model,
            "messages": messages,
            "temperature": 0.3
        });

        let res = self.client.post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send()
            .await?;

        if !res.status().is_success() {
            let error_text = res.text().await.unwrap_or_default();
            return Err(ProviderError::CloudError {
                code: "API_ERROR".to_string(),
                message: error_text,
            });
        }

        let json: serde_json::Value = res.json().await?;
        let text = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(LLMResponse {
            text,
            model: model.to_string(),
            prompt_tokens: json["usage"]["prompt_tokens"].as_u64().map(|v| v as usize),
            completion_tokens: json["usage"]["completion_tokens"].as_u64().map(|v| v as usize),
        })
    }
}
