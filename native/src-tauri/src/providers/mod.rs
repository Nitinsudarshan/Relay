mod ollama_manager;

pub use ollama_manager::{
    ensure_ollama_ready, list_installed_models, test_ollama_prompt, OllamaModelDetails,
    OllamaPromptTestResult, OllamaStatus,
};
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

    /// The request completed, but nothing a model produced came back — the
    /// client substituted filler, or the provider answered with nothing.
    /// Separate from the transport errors above because the network was fine;
    /// what is missing is the completion.
    #[error("No completion available: {0}")]
    NoCompletion(String),
}

/// The `model` value [`LLMClient::complete`] reports when it has silently
/// substituted its own canned text for a real completion.
///
/// Public because it is a property of this layer that callers must be able to
/// detect. Filler is the right answer for dictation, where some output beats
/// none, and the wrong one for anything persisted as understanding.
pub const HEURISTIC_FALLBACK_MODEL: &str = "heuristic-fallback";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    pub text: String,
    pub model: String,
    pub prompt_tokens: Option<usize>,
    pub completion_tokens: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProviderType {
    #[serde(rename = "ollama")]
    Ollama,
    #[serde(rename = "cloud_openai")]
    CloudOpenAI,
    #[serde(rename = "cloud_gemini")]
    CloudGemini,
    #[serde(rename = "cloud_anthropic")]
    CloudAnthropic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub active_provider: ProviderType,
    pub ollama_host: String,
    pub ollama_model: String,
    pub cloud_api_key: Option<String>,
    pub cloud_model: Option<String>,
    /// Prompt+output window, in tokens, the local model is told to allocate.
    ///
    /// This exists because Ollama's default is 4096 (2048 on older builds) and
    /// it silently discards whatever does not fit — from the *front* of the
    /// prompt. A meeting transcript is the longest prompt Relay ever sends, so
    /// before this setting existed the first half of any meeting past roughly a
    /// quarter of an hour was dropped before the model read a word of it, with
    /// nothing in the response to say so. Raise it for a model that supports a
    /// larger window; the meeting pipeline sizes its own chunking from this
    /// number rather than assuming one.
    #[serde(default = "default_context_tokens", alias = "contextTokens")]
    pub context_tokens: u32,
}

/// Chosen to fit a ~20-minute meeting in one pass on the 8k-window models
/// Relay's default local provider actually runs, without demanding memory a
/// laptop does not have.
fn default_context_tokens() -> u32 {
    8192
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            active_provider: ProviderType::Ollama,
            ollama_host: "http://localhost:11434".to_string(),
            ollama_model: "llama3.2:latest".to_string(),
            cloud_api_key: None,
            cloud_model: Some("gpt-4o-mini".to_string()),
            context_tokens: default_context_tokens(),
        }
    }
}

/// Sampling and budget for one completion.
///
/// Every field here used to be a provider default that Relay never expressed.
/// The consequential one is `temperature`: Ollama defaults to 0.8, so strict
/// JSON extraction from a meeting transcript was running at a creative-writing
/// setting, which is exactly the condition under which a model invents an owner
/// for an action item.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompletionOptions {
    pub temperature: f32,
    /// Cap on generated tokens. Bounds latency and stops a looping local model
    /// from running until the request times out.
    pub max_output_tokens: u32,
    /// The window the provider is asked to allocate for prompt plus output.
    pub context_tokens: u32,
    /// Wall-clock ceiling on the request. Without one a stalled local model
    /// leaves the UI on "Generating…" indefinitely.
    pub timeout_secs: u64,
}

impl Default for CompletionOptions {
    fn default() -> Self {
        Self {
            temperature: 0.3,
            max_output_tokens: 1_500,
            context_tokens: default_context_tokens(),
            timeout_secs: 300,
        }
    }
}

/// Where a cloud completion is sent, and how it authenticates.
///
/// Split out from the sender so the routing itself is testable. It has to be:
/// selecting Gemini or Anthropic used to post an OpenAI-shaped body to
/// `api.openai.com` with the user's key in an OpenAI header. That failed
/// authentication and fell through to canned filler, which is why those two
/// providers appeared to "work badly" rather than not at all — and it meant the
/// setting the user chose was not the service their meeting was sent to.
#[derive(Debug, Clone, PartialEq)]
pub struct CloudRoute {
    pub url: String,
    /// Header name and value carrying the API key.
    pub auth_header: (String, String),
    /// Extra headers the provider requires.
    pub extra_headers: Vec<(String, String)>,
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

    /// Completes without masking a provider failure.
    ///
    /// [`complete`](Self::complete) substitutes canned filler when a provider is
    /// unreachable, which suits dictation — some output beats none — and is
    /// actively wrong for a meeting summary, where filler presented as a model's
    /// work would be validated, persisted, and shown as an AI summary. Callers
    /// that need to know whether a model actually answered use this.
    pub async fn complete_with(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        options: CompletionOptions,
    ) -> Result<LLMResponse, ProviderError> {
        match self.config.active_provider {
            ProviderType::Ollama => self.complete_ollama_with(prompt, system_prompt, options).await,
            _ => self.complete_cloud_with(prompt, system_prompt, options).await,
        }
    }

    /// The Ollama request body.
    ///
    /// The `options` object is the whole point. Without it Ollama applies its own
    /// defaults — a 4096-token window (2048 on older builds) and temperature 0.8
    /// — and silently drops the overflowing *front* of the prompt. For a meeting
    /// transcript, the front is the agenda.
    pub fn ollama_request_body(
        model: &str,
        prompt: &str,
        system_prompt: Option<&str>,
        options: CompletionOptions,
    ) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false,
            "options": {
                "temperature": options.temperature,
                "num_ctx": options.context_tokens,
                "num_predict": options.max_output_tokens,
            }
        });
        if let Some(sys) = system_prompt {
            body["system"] = serde_json::Value::String(sys.to_string());
        }
        body
    }

    /// The OpenAI chat-completions body.
    pub fn openai_request_body(
        model: &str,
        prompt: &str,
        system_prompt: Option<&str>,
        options: CompletionOptions,
    ) -> serde_json::Value {
        let mut messages = Vec::new();
        if let Some(sys) = system_prompt {
            messages.push(serde_json::json!({ "role": "system", "content": sys }));
        }
        messages.push(serde_json::json!({ "role": "user", "content": prompt }));

        serde_json::json!({
            "model": model,
            "messages": messages,
            "temperature": options.temperature,
            "max_completion_tokens": options.max_output_tokens,
        })
    }

    /// The Anthropic messages body. The system prompt is a top-level field here,
    /// not a message role.
    pub fn anthropic_request_body(
        model: &str,
        prompt: &str,
        system_prompt: Option<&str>,
        options: CompletionOptions,
    ) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": model,
            "max_tokens": options.max_output_tokens,
            "temperature": options.temperature,
            "messages": [{ "role": "user", "content": prompt }],
        });
        if let Some(sys) = system_prompt {
            body["system"] = serde_json::Value::String(sys.to_string());
        }
        body
    }

    /// The Gemini `generateContent` body. System instructions and generation
    /// config are both separate objects here.
    pub fn gemini_request_body(
        prompt: &str,
        system_prompt: Option<&str>,
        options: CompletionOptions,
    ) -> serde_json::Value {
        let mut body = serde_json::json!({
            "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
            "generationConfig": {
                "temperature": options.temperature,
                "maxOutputTokens": options.max_output_tokens,
            }
        });
        if let Some(sys) = system_prompt {
            body["systemInstruction"] = serde_json::json!({ "parts": [{ "text": sys }] });
        }
        body
    }

    /// Resolves which service a cloud completion goes to.
    pub fn cloud_route(
        provider: &ProviderType,
        model: &str,
        api_key: &str,
    ) -> Result<CloudRoute, ProviderError> {
        match provider {
            ProviderType::CloudOpenAI => Ok(CloudRoute {
                url: "https://api.openai.com/v1/chat/completions".to_string(),
                auth_header: (
                    "Authorization".to_string(),
                    format!("Bearer {}", api_key),
                ),
                extra_headers: Vec::new(),
            }),
            ProviderType::CloudAnthropic => Ok(CloudRoute {
                url: "https://api.anthropic.com/v1/messages".to_string(),
                auth_header: ("x-api-key".to_string(), api_key.to_string()),
                extra_headers: vec![(
                    "anthropic-version".to_string(),
                    "2023-06-01".to_string(),
                )],
            }),
            ProviderType::CloudGemini => Ok(CloudRoute {
                url: format!(
                    "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
                    model
                ),
                auth_header: ("x-goog-api-key".to_string(), api_key.to_string()),
                extra_headers: Vec::new(),
            }),
            ProviderType::Ollama => Err(ProviderError::ConfigError(
                "Ollama is not a cloud provider".to_string(),
            )),
        }
    }

    /// Pulls the completion text out of whichever response shape came back.
    pub fn extract_cloud_text(provider: &ProviderType, json: &serde_json::Value) -> String {
        match provider {
            ProviderType::CloudAnthropic => json["content"]
                .as_array()
                .map(|blocks| {
                    blocks
                        .iter()
                        .filter_map(|b| b["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default(),
            ProviderType::CloudGemini => json["candidates"][0]["content"]["parts"]
                .as_array()
                .map(|parts| {
                    parts
                        .iter()
                        .filter_map(|p| p["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default(),
            _ => json["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string(),
        }
    }

    /// The default model for a cloud provider, used when settings name none.
    fn default_cloud_model(provider: &ProviderType) -> &'static str {
        match provider {
            ProviderType::CloudAnthropic => "claude-sonnet-4-5",
            ProviderType::CloudGemini => "gemini-2.0-flash",
            _ => "gpt-4o-mini",
        }
    }

    async fn complete_ollama_with(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        options: CompletionOptions,
    ) -> Result<LLMResponse, ProviderError> {
        let url = format!("{}/api/generate", self.config.ollama_host);
        let body = Self::ollama_request_body(
            &self.config.ollama_model,
            prompt,
            system_prompt,
            options,
        );

        let res = self
            .client
            .post(&url)
            .timeout(std::time::Duration::from_secs(options.timeout_secs))
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
        // Ollama reports how much of the prompt it actually evaluated. When that
        // is short of the window it was given, the prompt was truncated — the
        // failure this whole options object exists to prevent — so say so
        // rather than letting a half-read transcript pass as a full one.
        if let Some(evaluated) = json["prompt_eval_count"].as_u64() {
            if evaluated as u32 >= options.context_tokens.saturating_sub(options.max_output_tokens)
            {
                tracing::warn!(
                    evaluated_tokens = evaluated,
                    context_tokens = options.context_tokens,
                    "provider: prompt filled the model's context window; input may have been truncated"
                );
            }
        }

        Ok(LLMResponse {
            text: json["response"].as_str().unwrap_or("").to_string(),
            model: self.config.ollama_model.clone(),
            prompt_tokens: json["prompt_eval_count"].as_u64().map(|v| v as usize),
            completion_tokens: json["eval_count"].as_u64().map(|v| v as usize),
        })
    }

    async fn complete_cloud_with(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        options: CompletionOptions,
    ) -> Result<LLMResponse, ProviderError> {
        let api_key = self
            .config
            .cloud_api_key
            .as_ref()
            .filter(|k| !k.trim().is_empty())
            .ok_or_else(|| ProviderError::ConfigError("Cloud API key is missing".to_string()))?;

        let provider = &self.config.active_provider;
        let model = self
            .config
            .cloud_model
            .as_deref()
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(|| Self::default_cloud_model(provider));

        let route = Self::cloud_route(provider, model, api_key)?;
        let body = match provider {
            ProviderType::CloudAnthropic => {
                Self::anthropic_request_body(model, prompt, system_prompt, options)
            }
            ProviderType::CloudGemini => {
                Self::gemini_request_body(prompt, system_prompt, options)
            }
            _ => Self::openai_request_body(model, prompt, system_prompt, options),
        };

        let mut request = self
            .client
            .post(&route.url)
            .timeout(std::time::Duration::from_secs(options.timeout_secs))
            .header(&route.auth_header.0, &route.auth_header.1);
        for (name, value) in &route.extra_headers {
            request = request.header(name, value);
        }

        let res = request.json(&body).send().await?;
        if !res.status().is_success() {
            let code = res.status().as_u16().to_string();
            let error_text = res.text().await.unwrap_or_default();
            return Err(ProviderError::CloudError {
                code,
                message: error_text,
            });
        }

        let json: serde_json::Value = res.json().await?;
        Ok(LLMResponse {
            text: Self::extract_cloud_text(provider, &json),
            model: model.to_string(),
            prompt_tokens: json["usage"]["prompt_tokens"]
                .as_u64()
                .or_else(|| json["usage"]["input_tokens"].as_u64())
                .map(|v| v as usize),
            completion_tokens: json["usage"]["completion_tokens"]
                .as_u64()
                .or_else(|| json["usage"]["output_tokens"].as_u64())
                .map(|v| v as usize),
        })
    }

    pub async fn complete(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
    ) -> Result<LLMResponse, ProviderError> {
        match self.config.active_provider {
            ProviderType::Ollama => match self.complete_ollama(prompt, system_prompt).await {
                Ok(resp) => Ok(resp),
                Err(err) => {
                    tracing::warn!(
                        "Ollama unavailable ({}), using local heuristic fallback",
                        err
                    );
                    Ok(Self::heuristic_fallback(prompt, system_prompt))
                }
            },
            ProviderType::CloudOpenAI | ProviderType::CloudGemini | ProviderType::CloudAnthropic => {
                match self.complete_cloud(prompt, system_prompt).await {
                    Ok(resp) => Ok(resp),
                    Err(err) => {
                        tracing::warn!(
                            "Cloud provider failed ({}), using local heuristic fallback",
                            err
                        );
                        Ok(Self::heuristic_fallback(prompt, system_prompt))
                    }
                }
            }
        }
    }

    /// Completes, and fails when no model actually answered.
    ///
    /// [`complete_with`](Self::complete_with) already reports a transport
    /// failure honestly, but that is only half the problem: a caller that built
    /// its own [`LLMClient`] elsewhere, or a future path that routes through
    /// [`complete`](Self::complete), can still be handed filler tagged
    /// [`HEURISTIC_FALLBACK_MODEL`]. This is the one entry point analysis code
    /// should use, because for analysis the two cases are the same case —
    /// nothing was understood, and saying otherwise puts invented content into
    /// the vault under a model's name.
    pub async fn complete_verified(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        options: CompletionOptions,
    ) -> Result<LLMResponse, ProviderError> {
        let response = self.complete_with(prompt, system_prompt, options).await?;
        if response.model == HEURISTIC_FALLBACK_MODEL {
            return Err(ProviderError::NoCompletion(
                "provider unreachable; the client substituted heuristic filler".to_string(),
            ));
        }
        if response.text.trim().is_empty() {
            return Err(ProviderError::NoCompletion(
                "provider returned an empty completion".to_string(),
            ));
        }
        Ok(response)
    }

    pub fn heuristic_fallback(prompt: &str, system_prompt: Option<&str>) -> LLMResponse {
        let is_json = system_prompt.map(|s| s.contains("JSON")).unwrap_or(false);
        if is_json {
            let sys = system_prompt.unwrap_or("");
            if sys.contains("Knowledge & Thinking Assistant") || sys.contains("thought/scribble") || sys.contains("topics") || sys.contains("entities") {
                let structured = crate::pipeline::extract_deterministic_knowledge(prompt);
                let json_text = serde_json::to_string_pretty(&structured).unwrap_or_else(|_| "{}".to_string());
                return LLMResponse {
                    text: json_text,
                    model: HEURISTIC_FALLBACK_MODEL.to_string(),
                    prompt_tokens: None,
                    completion_tokens: None,
                };
            }

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
                model: HEURISTIC_FALLBACK_MODEL.to_string(),
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
                model: HEURISTIC_FALLBACK_MODEL.to_string(),
                prompt_tokens: None,
                completion_tokens: None,
            }
        }
    }

    /// The default-options path, kept so existing callers read unchanged.
    ///
    /// Delegates rather than duplicating the request: dictation and Scribble
    /// benefit from the same window, temperature, and timeout the meeting
    /// pipeline needed, and one request builder cannot drift from another.
    async fn complete_ollama(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
    ) -> Result<LLMResponse, ProviderError> {
        self.complete_ollama_with(prompt, system_prompt, self.default_options())
            .await
    }

    async fn complete_cloud(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
    ) -> Result<LLMResponse, ProviderError> {
        self.complete_cloud_with(prompt, system_prompt, self.default_options())
            .await
    }

    /// Completion options that honour the user's configured window.
    pub fn default_options(&self) -> CompletionOptions {
        CompletionOptions {
            context_tokens: self.config.context_tokens.max(2_048),
            ..CompletionOptions::default()
        }
    }

    /// The window the caller may fill, in tokens.
    pub fn context_tokens(&self) -> u32 {
        self.config.context_tokens.max(2_048)
    }

    /// The active provider, so callers can report which service answered.
    pub fn provider_type(&self) -> &ProviderType {
        &self.config.active_provider
    }

    /// The model that will actually be asked, for observability.
    pub fn model_name(&self) -> String {
        match self.config.active_provider {
            ProviderType::Ollama => self.config.ollama_model.clone(),
            ref provider => self
                .config
                .cloud_model
                .clone()
                .filter(|m| !m.trim().is_empty())
                .unwrap_or_else(|| Self::default_cloud_model(provider).to_string()),
        }
    }

    /// Streams a completion, calling `on_delta` with each fragment as it
    /// arrives.
    ///
    /// This exists for Talkback, where waiting for a whole answer before
    /// speaking any of it is the difference between a conversation and a
    /// form submission. Everything else in Relay is batch work — a
    /// meeting summary has nobody waiting on its first sentence — and
    /// keeps using [`complete_with`](Self::complete_with).
    ///
    /// `on_delta` returns `false` to abandon the stream, which is how
    /// barge-in stops a generation the user has already talked over. A
    /// cancelled stream returns whatever had accumulated so far rather
    /// than an error: the partial answer was really said, and the caller
    /// still needs it for the transcript.
    pub async fn complete_streaming<F>(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        options: CompletionOptions,
        mut on_delta: F,
    ) -> Result<LLMResponse, ProviderError>
    where
        F: FnMut(&str) -> bool,
    {
        let provider = self.config.active_provider.clone();
        let model = self.model_name();

        let (url, body, headers) = self.streaming_request(&model, prompt, system_prompt, options)?;

        let mut request = self
            .client
            .post(&url)
            .timeout(std::time::Duration::from_secs(options.timeout_secs));
        for (name, value) in &headers {
            request = request.header(name, value);
        }

        let mut response = request.json(&body).send().await.map_err(|e| match provider {
            ProviderType::Ollama => ProviderError::OllamaUnavailable {
                host: self.config.ollama_host.clone(),
                message: e.to_string(),
            },
            _ => ProviderError::Network(e),
        })?;

        if !response.status().is_success() {
            let code = response.status().as_u16().to_string();
            let message = response.text().await.unwrap_or_default();
            return match provider {
                ProviderType::Ollama => Err(ProviderError::OllamaUnavailable {
                    host: self.config.ollama_host.clone(),
                    message: format!("HTTP {code}: {message}"),
                }),
                _ => Err(ProviderError::CloudError { code, message }),
            };
        }

        let mut text = String::new();
        // Bytes, not a String: a chunk boundary can fall inside a
        // multi-byte character, and decoding each chunk on its own would
        // corrupt Devanagari mid-word.
        let mut pending: Vec<u8> = Vec::new();
        let mut finished = false;

        // `Response::chunk` rather than `bytes_stream` deliberately: it
        // needs no `stream` feature on reqwest and no futures-util
        // dependency, so streaming costs Relay zero new crates.
        while let Some(chunk) = response.chunk().await? {
            pending.extend_from_slice(&chunk);
            while let Some(newline) = pending.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = pending.drain(..=newline).collect();
                let line = String::from_utf8_lossy(&line);
                match parse_stream_line(&provider, line.trim_end_matches(['\r', '\n'])) {
                    StreamEvent::Delta(delta) => {
                        text.push_str(&delta);
                        if !on_delta(&delta) {
                            return Ok(LLMResponse {
                                text,
                                model,
                                prompt_tokens: None,
                                completion_tokens: None,
                            });
                        }
                    }
                    StreamEvent::Done => finished = true,
                    StreamEvent::Ignore => {}
                }
            }
            if finished {
                break;
            }
        }

        // A final line with no trailing newline still carries a delta.
        if !pending.is_empty() {
            let line = String::from_utf8_lossy(&pending);
            if let StreamEvent::Delta(delta) =
                parse_stream_line(&provider, line.trim_end_matches(['\r', '\n']))
            {
                text.push_str(&delta);
                on_delta(&delta);
            }
        }

        Ok(LLMResponse {
            text,
            model,
            prompt_tokens: None,
            completion_tokens: None,
        })
    }

    /// URL, body and headers for a streaming request.
    fn streaming_request(
        &self,
        model: &str,
        prompt: &str,
        system_prompt: Option<&str>,
        options: CompletionOptions,
    ) -> Result<StreamingRequest, ProviderError> {
        match self.config.active_provider {
            ProviderType::Ollama => {
                let mut body =
                    Self::ollama_request_body(model, prompt, system_prompt, options);
                body["stream"] = serde_json::Value::Bool(true);
                Ok((
                    format!("{}/api/generate", self.config.ollama_host),
                    body,
                    Vec::new(),
                ))
            }
            ref provider => {
                let api_key = self
                    .config
                    .cloud_api_key
                    .as_ref()
                    .filter(|k| !k.trim().is_empty())
                    .ok_or_else(|| {
                        ProviderError::ConfigError("Cloud API key is missing".to_string())
                    })?;
                let route = Self::cloud_route(provider, model, api_key)?;

                let mut body = match provider {
                    ProviderType::CloudAnthropic => {
                        Self::anthropic_request_body(model, prompt, system_prompt, options)
                    }
                    ProviderType::CloudGemini => {
                        Self::gemini_request_body(prompt, system_prompt, options)
                    }
                    _ => Self::openai_request_body(model, prompt, system_prompt, options),
                };

                // Gemini signals streaming in the URL, everyone else in
                // the body.
                let url = if matches!(provider, ProviderType::CloudGemini) {
                    format!(
                        "https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent?alt=sse"
                    )
                } else {
                    body["stream"] = serde_json::Value::Bool(true);
                    route.url.clone()
                };

                let mut headers = vec![route.auth_header.clone()];
                headers.extend(route.extra_headers.clone());
                Ok((url, body, headers))
            }
        }
    }
}

/// A streaming request: where to post, what to post, and the headers it
/// needs. Named because the four providers disagree on all three.
type StreamingRequest = (String, serde_json::Value, Vec<(String, String)>);

/// One parsed line of a provider's response stream.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    /// Text to append and speak.
    Delta(String),
    /// The provider said the generation is complete.
    Done,
    /// Keep-alive, metadata, or a line this provider does not use.
    Ignore,
}

/// Parses one line of a provider's stream.
///
/// Pure, and public, because the four providers disagree about everything
/// — NDJSON versus SSE, where the text lives, how completion is signalled
/// — and the only way to be sure Relay reads all four correctly without a
/// network is to test this function directly.
pub fn parse_stream_line(provider: &ProviderType, line: &str) -> StreamEvent {
    let line = line.trim();
    if line.is_empty() {
        return StreamEvent::Ignore;
    }

    match provider {
        // Ollama streams newline-delimited JSON with no `data:` prefix.
        ProviderType::Ollama => {
            let Ok(json) = serde_json::from_str::<serde_json::Value>(line) else {
                return StreamEvent::Ignore;
            };
            if let Some(text) = json["response"].as_str() {
                if !text.is_empty() {
                    return StreamEvent::Delta(text.to_string());
                }
            }
            if json["done"].as_bool().unwrap_or(false) {
                return StreamEvent::Done;
            }
            StreamEvent::Ignore
        }
        // The rest are server-sent events.
        provider => {
            // SSE carries `event:` and comment lines too; only `data:`
            // frames hold content.
            let Some(payload) = line.strip_prefix("data:") else {
                return StreamEvent::Ignore;
            };
            let payload = payload.trim();
            if payload == "[DONE]" {
                return StreamEvent::Done;
            }
            let Ok(json) = serde_json::from_str::<serde_json::Value>(payload) else {
                return StreamEvent::Ignore;
            };

            match provider {
                ProviderType::CloudAnthropic => {
                    match json["type"].as_str() {
                        Some("content_block_delta") => json["delta"]["text"]
                            .as_str()
                            .filter(|t| !t.is_empty())
                            .map(|t| StreamEvent::Delta(t.to_string()))
                            .unwrap_or(StreamEvent::Ignore),
                        Some("message_stop") => StreamEvent::Done,
                        _ => StreamEvent::Ignore,
                    }
                }
                ProviderType::CloudGemini => json["candidates"][0]["content"]["parts"]
                    .as_array()
                    .map(|parts| {
                        parts
                            .iter()
                            .filter_map(|p| p["text"].as_str())
                            .collect::<String>()
                    })
                    .filter(|t| !t.is_empty())
                    .map(StreamEvent::Delta)
                    .unwrap_or(StreamEvent::Ignore),
                // OpenAI and anything else OpenAI-shaped.
                _ => json["choices"][0]["delta"]["content"]
                    .as_str()
                    .filter(|t| !t.is_empty())
                    .map(|t| StreamEvent::Delta(t.to_string()))
                    .unwrap_or(StreamEvent::Ignore),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> CompletionOptions {
        CompletionOptions {
            temperature: 0.1,
            max_output_tokens: 900,
            context_tokens: 16_384,
            timeout_secs: 60,
        }
    }

    #[test]
    fn the_ollama_body_states_the_window_instead_of_accepting_the_default() {
        // The regression this pins: without `options`, Ollama used a 4096-token
        // window and silently dropped the front of any longer prompt.
        let body = LLMClient::ollama_request_body("llama3.2", "transcript", Some("rules"), options());
        assert_eq!(body["options"]["num_ctx"], 16_384);
        assert_eq!(body["options"]["num_predict"], 900);
        assert_eq!(body["options"]["temperature"].as_f64().unwrap(), 0.1_f32 as f64);
        assert_eq!(body["system"], "rules");
        assert_eq!(body["stream"], false);
    }

    #[test]
    fn the_system_prompt_is_a_field_not_a_prefix_on_the_user_prompt() {
        // It used to be folded into the prompt as "[System Instructions: ...]",
        // which put Relay's rules and the meeting's own words in one
        // undifferentiated string.
        let body = LLMClient::ollama_request_body("m", "transcript", Some("rules"), options());
        assert_eq!(body["prompt"], "transcript");
        assert!(!body["prompt"].as_str().unwrap().contains("System Instructions"));
    }

    #[test]
    fn each_cloud_provider_is_routed_to_its_own_service() {
        let openai = LLMClient::cloud_route(&ProviderType::CloudOpenAI, "gpt-4o-mini", "k").unwrap();
        assert!(openai.url.contains("api.openai.com"));
        assert_eq!(openai.auth_header.0, "Authorization");

        let anthropic =
            LLMClient::cloud_route(&ProviderType::CloudAnthropic, "claude-sonnet-4-5", "k").unwrap();
        assert!(anthropic.url.contains("api.anthropic.com"));
        assert_eq!(anthropic.auth_header.0, "x-api-key");
        assert!(anthropic
            .extra_headers
            .iter()
            .any(|(n, _)| n == "anthropic-version"));

        let gemini =
            LLMClient::cloud_route(&ProviderType::CloudGemini, "gemini-2.0-flash", "k").unwrap();
        assert!(gemini.url.contains("generativelanguage.googleapis.com"));
        assert!(gemini.url.contains("gemini-2.0-flash"));
        assert_eq!(gemini.auth_header.0, "x-goog-api-key");
    }

    #[test]
    fn no_cloud_provider_is_routed_to_another_ones_endpoint() {
        for (provider, host) in [
            (ProviderType::CloudOpenAI, "api.openai.com"),
            (ProviderType::CloudAnthropic, "api.anthropic.com"),
            (ProviderType::CloudGemini, "generativelanguage.googleapis.com"),
        ] {
            let route = LLMClient::cloud_route(&provider, "model", "key").unwrap();
            assert!(
                route.url.contains(host),
                "{:?} must not be sent to another provider's endpoint",
                provider
            );
        }
    }

    #[test]
    fn each_provider_gets_the_body_shape_it_actually_accepts() {
        let anthropic =
            LLMClient::anthropic_request_body("claude-sonnet-4-5", "user text", Some("rules"), options());
        assert_eq!(anthropic["system"], "rules");
        assert_eq!(anthropic["max_tokens"], 900);
        assert!(anthropic["messages"][0]["content"] == "user text");
        assert!(anthropic.get("messages").is_some() && anthropic["messages"].as_array().unwrap().len() == 1);

        let gemini = LLMClient::gemini_request_body("user text", Some("rules"), options());
        assert_eq!(gemini["systemInstruction"]["parts"][0]["text"], "rules");
        assert_eq!(gemini["generationConfig"]["maxOutputTokens"], 900);
        assert_eq!(gemini["contents"][0]["parts"][0]["text"], "user text");

        let openai = LLMClient::openai_request_body("gpt-4o-mini", "user text", Some("rules"), options());
        assert_eq!(openai["messages"][0]["role"], "system");
        assert_eq!(openai["messages"][1]["content"], "user text");
        assert_eq!(openai["max_completion_tokens"], 900);
    }

    #[test]
    fn text_is_read_out_of_each_providers_response_shape() {
        let anthropic = serde_json::json!({"content": [{"type": "text", "text": "hello "}, {"type": "text", "text": "world"}]});
        assert_eq!(
            LLMClient::extract_cloud_text(&ProviderType::CloudAnthropic, &anthropic),
            "hello world"
        );

        let gemini = serde_json::json!({"candidates": [{"content": {"parts": [{"text": "hi"}]}}]});
        assert_eq!(
            LLMClient::extract_cloud_text(&ProviderType::CloudGemini, &gemini),
            "hi"
        );

        let openai = serde_json::json!({"choices": [{"message": {"content": "hey"}}]});
        assert_eq!(
            LLMClient::extract_cloud_text(&ProviderType::CloudOpenAI, &openai),
            "hey"
        );
    }

    #[test]
    fn a_configured_window_below_the_floor_is_raised_rather_than_honoured() {
        let client = LLMClient::new(ProviderConfig {
            context_tokens: 512,
            ..Default::default()
        });
        assert_eq!(client.context_tokens(), 2_048);
    }

    #[tokio::test]
    async fn an_unreachable_provider_is_an_error_here_rather_than_filler() {
        let client = LLMClient::new(ProviderConfig {
            ollama_host: "http://127.0.0.1:1".to_string(),
            ..Default::default()
        });

        let masked = client.complete("prompt", Some("system")).await.unwrap();
        assert_eq!(masked.model, "heuristic-fallback");

        let honest = client
            .complete_with("prompt", Some("system"), CompletionOptions::default())
            .await;
        assert!(
            honest.is_err(),
            "a summary must never be written from canned filler presented as a model's work"
        );
    }

    #[test]
    fn ollama_ndjson_deltas_are_parsed() {
        let event = parse_stream_line(
            &ProviderType::Ollama,
            r#"{"model":"llama3.2","response":"Flat ","done":false}"#,
        );
        assert_eq!(event, StreamEvent::Delta("Flat ".to_string()));
    }

    #[test]
    fn ollama_signals_completion_with_done() {
        assert_eq!(
            parse_stream_line(&ProviderType::Ollama, r#"{"response":"","done":true}"#),
            StreamEvent::Done
        );
    }

    #[test]
    fn openai_sse_deltas_are_parsed() {
        let event = parse_stream_line(
            &ProviderType::CloudOpenAI,
            r#"data: {"choices":[{"delta":{"content":"seat "}}]}"#,
        );
        assert_eq!(event, StreamEvent::Delta("seat ".to_string()));
        assert_eq!(
            parse_stream_line(&ProviderType::CloudOpenAI, "data: [DONE]"),
            StreamEvent::Done
        );
    }

    #[test]
    fn openai_role_only_frames_are_ignored() {
        assert_eq!(
            parse_stream_line(
                &ProviderType::CloudOpenAI,
                r#"data: {"choices":[{"delta":{"role":"assistant"}}]}"#
            ),
            StreamEvent::Ignore
        );
    }

    #[test]
    fn anthropic_content_block_deltas_are_parsed() {
        let event = parse_stream_line(
            &ProviderType::CloudAnthropic,
            r#"data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"licence"}}"#,
        );
        assert_eq!(event, StreamEvent::Delta("licence".to_string()));
        assert_eq!(
            parse_stream_line(
                &ProviderType::CloudAnthropic,
                r#"data: {"type":"message_stop"}"#
            ),
            StreamEvent::Done
        );
        assert_eq!(
            parse_stream_line(
                &ProviderType::CloudAnthropic,
                r#"data: {"type":"ping"}"#
            ),
            StreamEvent::Ignore
        );
    }

    #[test]
    fn gemini_sse_deltas_are_parsed() {
        let event = parse_stream_line(
            &ProviderType::CloudGemini,
            r#"data: {"candidates":[{"content":{"parts":[{"text":"We shipped"}]}}]}"#,
        );
        assert_eq!(event, StreamEvent::Delta("We shipped".to_string()));
    }

    #[test]
    fn sse_comments_and_event_lines_are_ignored() {
        for provider in [
            ProviderType::CloudOpenAI,
            ProviderType::CloudAnthropic,
            ProviderType::CloudGemini,
        ] {
            assert_eq!(parse_stream_line(&provider, ": keep-alive"), StreamEvent::Ignore);
            assert_eq!(
                parse_stream_line(&provider, "event: content_block_delta"),
                StreamEvent::Ignore
            );
            assert_eq!(parse_stream_line(&provider, ""), StreamEvent::Ignore);
        }
    }

    #[test]
    fn malformed_json_is_skipped_rather_than_fatal() {
        // A truncated frame must not abort a generation the user is
        // already listening to.
        assert_eq!(
            parse_stream_line(&ProviderType::Ollama, r#"{"response":"half"#),
            StreamEvent::Ignore
        );
        assert_eq!(
            parse_stream_line(&ProviderType::CloudOpenAI, "data: {not json"),
            StreamEvent::Ignore
        );
    }

    #[test]
    fn the_streaming_ollama_body_sets_stream_true_and_keeps_the_window() {
        let client = LLMClient::new(ProviderConfig::default());
        let (url, body, headers) = client
            .streaming_request("llama3.2", "question", Some("rules"), options())
            .unwrap();
        assert!(url.ends_with("/api/generate"));
        assert_eq!(body["stream"], true);
        assert_eq!(body["options"]["num_ctx"], 16_384);
        assert_eq!(body["system"], "rules");
        assert!(headers.is_empty());
    }

    #[test]
    fn the_streaming_gemini_request_uses_the_sse_endpoint() {
        let client = LLMClient::new(ProviderConfig {
            active_provider: ProviderType::CloudGemini,
            cloud_api_key: Some("key".to_string()),
            cloud_model: Some("gemini-2.0-flash".to_string()),
            ..Default::default()
        });
        let (url, body, headers) = client
            .streaming_request("gemini-2.0-flash", "q", None, options())
            .unwrap();
        assert!(url.contains(":streamGenerateContent?alt=sse"), "{url}");
        assert!(
            body.get("stream").is_none(),
            "Gemini signals streaming in the URL, not the body"
        );
        assert!(headers.iter().any(|(k, _)| k == "x-goog-api-key"));
    }

    #[test]
    fn the_streaming_anthropic_request_carries_the_version_header() {
        let client = LLMClient::new(ProviderConfig {
            active_provider: ProviderType::CloudAnthropic,
            cloud_api_key: Some("key".to_string()),
            ..Default::default()
        });
        let (url, body, headers) = client
            .streaming_request("claude-sonnet-4-5", "q", Some("sys"), options())
            .unwrap();
        assert_eq!(url, "https://api.anthropic.com/v1/messages");
        assert_eq!(body["stream"], true);
        assert_eq!(body["system"], "sys");
        assert!(headers.iter().any(|(k, v)| k == "anthropic-version" && v == "2023-06-01"));
    }

    #[test]
    fn a_cloud_stream_without_a_key_fails_before_the_request() {
        let client = LLMClient::new(ProviderConfig {
            active_provider: ProviderType::CloudOpenAI,
            cloud_api_key: None,
            ..Default::default()
        });
        assert!(matches!(
            client.streaming_request("gpt-4o-mini", "q", None, options()),
            Err(ProviderError::ConfigError(_))
        ));
    }

    #[test]
    fn the_model_name_falls_back_to_the_provider_default() {
        let ollama = LLMClient::new(ProviderConfig::default());
        assert_eq!(ollama.model_name(), "llama3.2:latest");

        let anthropic = LLMClient::new(ProviderConfig {
            active_provider: ProviderType::CloudAnthropic,
            cloud_model: Some("  ".to_string()),
            ..Default::default()
        });
        assert_eq!(anthropic.model_name(), "claude-sonnet-4-5");
    }

    #[tokio::test]
    async fn an_unreachable_provider_fails_the_stream_rather_than_hanging() {
        let client = LLMClient::new(ProviderConfig {
            ollama_host: "http://127.0.0.1:1".to_string(),
            ..Default::default()
        });
        let result = client
            .complete_streaming("q", None, CompletionOptions::default(), |_| true)
            .await;
        assert!(matches!(
            result,
            Err(ProviderError::OllamaUnavailable { .. })
        ));
    }
}
