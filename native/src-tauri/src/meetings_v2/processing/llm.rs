//! The processing pipeline's view of a language model.
//!
//! The pipeline never names a provider. It asks a `MeetingLlm` for a
//! completion and is told honestly whether one happened. Ollama, a cloud
//! endpoint, and a future local runtime are all the same to it, and a test can
//! substitute a scripted implementation without a network.
//!
//! Boxed futures rather than `async_trait` keep this dependency-free; the
//! pipeline makes at most two model calls per meeting, so the allocation is
//! irrelevant.

use std::future::Future;
use std::pin::Pin;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A completion that actually came from a model.
#[derive(Debug, Clone, PartialEq)]
pub struct LlmOutcome {
    pub text: String,
    pub provider: String,
    pub model: String,
}

/// Why no completion is available. Both variants mean the same thing to the
/// pipeline — fall back to the deterministic path — but they are recorded
/// separately so the processing log says which happened.
#[derive(Debug, Clone, PartialEq)]
pub enum LlmError {
    /// The provider could not be reached, or answered with filler.
    Unavailable(String),
    /// The provider answered, but with nothing usable.
    Empty,
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(msg) => write!(f, "model unavailable: {}", msg),
            Self::Empty => write!(f, "model returned an empty response"),
        }
    }
}

/// A language model the meeting pipeline can call.
pub trait MeetingLlm: Send + Sync {
    fn complete<'a>(
        &'a self,
        system_prompt: &'a str,
        user_prompt: &'a str,
    ) -> BoxFuture<'a, Result<LlmOutcome, LlmError>>;

    /// Reported in stage state and the processing log even when the call fails,
    /// so "which model ran?" is answerable for a failed meeting.
    fn provider_name(&self) -> String;
    fn model_name(&self) -> String;
}

/// The marker `providers::LLMClient` returns when it has silently substituted
/// its own canned text for a real completion.
const HEURISTIC_MODEL_MARKER: &str = "heuristic-fallback";

/// Adapts the app's shared `LLMClient` to the pipeline's contract.
///
/// `LLMClient::complete` never returns `Err`: on any provider failure it logs a
/// warning and returns canned filler tagged `model: "heuristic-fallback"`. That
/// is fine for dictation, where some output beats none, but it would let the
/// meeting pipeline present filler as a model's work and would make a validator
/// judge text no model wrote. This adapter therefore treats that marker as the
/// failure it actually is, so the pipeline can choose its own deterministic path
/// and record what happened. `providers/mod.rs` is left untouched — see
/// out-of-scope issue 2 in `docs/meetings/MEETINGS_INTELLIGENCE_AUDIT.md`.
pub struct ProviderLlm {
    client: crate::providers::LLMClient,
    provider: String,
    model: String,
}

impl ProviderLlm {
    pub fn new(config: crate::providers::ProviderConfig) -> Self {
        let (provider, model) = match config.active_provider {
            crate::providers::ProviderType::Ollama => {
                ("ollama".to_string(), config.ollama_model.clone())
            }
            crate::providers::ProviderType::CloudOpenAI => (
                "cloud_openai".to_string(),
                config.cloud_model.clone().unwrap_or_default(),
            ),
            crate::providers::ProviderType::CloudGemini => (
                "cloud_gemini".to_string(),
                config.cloud_model.clone().unwrap_or_default(),
            ),
            crate::providers::ProviderType::CloudAnthropic => (
                "cloud_anthropic".to_string(),
                config.cloud_model.clone().unwrap_or_default(),
            ),
        };

        Self {
            client: crate::providers::LLMClient::new(config),
            provider,
            model,
        }
    }
}

impl MeetingLlm for ProviderLlm {
    fn complete<'a>(
        &'a self,
        system_prompt: &'a str,
        user_prompt: &'a str,
    ) -> BoxFuture<'a, Result<LlmOutcome, LlmError>> {
        Box::pin(async move {
            let response = self
                .client
                .complete(user_prompt, Some(system_prompt))
                .await
                .map_err(|e| LlmError::Unavailable(e.to_string()))?;

            if response.model == HEURISTIC_MODEL_MARKER {
                return Err(LlmError::Unavailable(
                    "provider unreachable; the shared client substituted heuristic filler"
                        .to_string(),
                ));
            }
            if response.text.trim().is_empty() {
                return Err(LlmError::Empty);
            }

            Ok(LlmOutcome {
                text: response.text,
                provider: self.provider.clone(),
                model: response.model,
            })
        })
    }

    fn provider_name(&self) -> String {
        self.provider.clone()
    }

    fn model_name(&self) -> String {
        self.model.clone()
    }
}

#[cfg(test)]
pub mod test_support {
    use super::*;
    use std::sync::Mutex;

    /// A model stand-in that replays queued responses. Lets the pipeline's
    /// failure paths — timeouts, invalid JSON, empty output — be tested without
    /// a network or an Ollama instance.
    pub struct ScriptedLlm {
        responses: Mutex<Vec<Result<String, LlmError>>>,
        pub calls: Mutex<Vec<(String, String)>>,
    }

    impl ScriptedLlm {
        /// Responses are consumed in order. Once exhausted, further calls fail
        /// as unavailable rather than silently repeating the last answer.
        pub fn new(responses: Vec<Result<String, LlmError>>) -> Self {
            Self {
                responses: Mutex::new(responses.into_iter().rev().collect()),
                calls: Mutex::new(Vec::new()),
            }
        }

        pub fn always_unavailable() -> Self {
            Self::new(Vec::new())
        }

        pub fn call_count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }
    }

    impl MeetingLlm for ScriptedLlm {
        fn complete<'a>(
            &'a self,
            system_prompt: &'a str,
            user_prompt: &'a str,
        ) -> BoxFuture<'a, Result<LlmOutcome, LlmError>> {
            self.calls
                .lock()
                .unwrap()
                .push((system_prompt.to_string(), user_prompt.to_string()));

            let next = self.responses.lock().unwrap().pop();
            Box::pin(async move {
                match next {
                    Some(Ok(text)) => Ok(LlmOutcome {
                        text,
                        provider: "scripted".to_string(),
                        model: "scripted-model".to_string(),
                    }),
                    Some(Err(e)) => Err(e),
                    None => Err(LlmError::Unavailable(
                        "no scripted response left".to_string(),
                    )),
                }
            })
        }

        fn provider_name(&self) -> String {
            "scripted".to_string()
        }

        fn model_name(&self) -> String {
            "scripted-model".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::ScriptedLlm;
    use super::*;

    #[tokio::test]
    async fn scripted_responses_are_replayed_in_order_then_fail() {
        let llm = ScriptedLlm::new(vec![Ok("first".into()), Ok("second".into())]);

        assert_eq!(llm.complete("s", "u").await.unwrap().text, "first");
        assert_eq!(llm.complete("s", "u").await.unwrap().text, "second");
        assert!(matches!(
            llm.complete("s", "u").await,
            Err(LlmError::Unavailable(_))
        ));
        assert_eq!(llm.call_count(), 3);
    }

    #[tokio::test]
    async fn a_heuristic_fallback_response_is_reported_as_a_failure() {
        // `LLMClient` masks provider outages by returning canned filler tagged
        // with this model name; the pipeline must not mistake it for a model
        // answer.
        let config = crate::providers::ProviderConfig {
            // Deliberately unreachable, which is what triggers the mask.
            ollama_host: "http://127.0.0.1:1".to_string(),
            ..Default::default()
        };
        let llm = ProviderLlm::new(config);

        let result = llm.complete("Return JSON", "some transcript").await;
        match result {
            Err(LlmError::Unavailable(msg)) => {
                assert!(msg.contains("heuristic"), "unexpected message: {}", msg)
            }
            other => panic!("expected an unavailable error, got {:?}", other),
        }
    }
}
