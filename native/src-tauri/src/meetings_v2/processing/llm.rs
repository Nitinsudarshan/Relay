//! The processing pipeline's view of a language model.
//!
//! The pipeline never names a provider. It asks a `MeetingLlm` for a
//! completion and is told honestly whether one happened. Ollama, a cloud
//! endpoint, and a future local runtime are all the same to it, and a test can
//! substitute a scripted implementation without a network.
//!
//! Boxed futures rather than `async_trait` keep this dependency-free; the
//! pipeline makes a handful of model calls per meeting at most, so the
//! allocation is irrelevant.
//!
//! Two things the trait carries beyond "send a prompt": the sampling each stage
//! needs, and how much prompt the model can actually read. Both were previously
//! left to whatever the provider defaulted to, and both silently cost quality —
//! extraction ran at a creative-writing temperature, and a transcript longer
//! than the model's window was cut off with nothing in the response to say so.

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

/// One call's sampling and budget.
///
/// The pipeline's two stages are different jobs and want different settings:
/// extraction is a strict-JSON read of a transcript and wants near-zero
/// temperature, while writing prose needs a little room. Before this existed
/// neither could be expressed, so both ran at whatever the provider defaulted
/// to — 0.8 on Ollama, which is a creative-writing setting applied to the stage
/// whose failure mode is inventing an owner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LlmRequest<'a> {
    pub system_prompt: &'a str,
    pub user_prompt: &'a str,
    pub temperature: f32,
    pub max_output_tokens: u32,
}

impl<'a> LlmRequest<'a> {
    /// Stage A: comprehension into strict JSON. Temperature from
    /// `Meeting-rules/meeting_transcript_summary.md` §11.
    pub fn extraction(system_prompt: &'a str, user_prompt: &'a str) -> Self {
        Self {
            system_prompt,
            user_prompt,
            temperature: 0.1,
            max_output_tokens: 2_400,
        }
    }

    /// Stage B: writing. Slightly above extraction because rewriting requires
    /// generation, low enough to stay grounded.
    pub fn prose(system_prompt: &'a str, user_prompt: &'a str, max_output_tokens: u32) -> Self {
        Self {
            system_prompt,
            user_prompt,
            temperature: 0.3,
            max_output_tokens,
        }
    }
}

/// A language model the meeting pipeline can call.
pub trait MeetingLlm: Send + Sync {
    fn complete_request<'a>(
        &'a self,
        request: LlmRequest<'a>,
    ) -> BoxFuture<'a, Result<LlmOutcome, LlmError>>;

    /// The convenience form, kept because most call sites want the defaults.
    fn complete<'a>(
        &'a self,
        system_prompt: &'a str,
        user_prompt: &'a str,
    ) -> BoxFuture<'a, Result<LlmOutcome, LlmError>> {
        self.complete_request(LlmRequest {
            system_prompt,
            user_prompt,
            temperature: 0.3,
            max_output_tokens: 1_500,
        })
    }

    /// How many characters of user prompt this model can actually read.
    ///
    /// Not a nicety: a provider that is handed more than its window silently
    /// discards the overflow, and the pipeline has no way to tell from the
    /// response that it happened. Extraction sizes its own chunking from this
    /// so a long meeting is processed in passes rather than half-read.
    fn prompt_budget_chars(&self) -> usize;

    /// Reported in stage state and the processing log even when the call fails,
    /// so "which model ran?" is answerable for a failed meeting.
    fn provider_name(&self) -> String;
    fn model_name(&self) -> String;
}

/// Characters per token, conservatively low.
///
/// Under-estimating wastes a little window; over-estimating silently truncates
/// a transcript, which is the failure this whole mechanism exists to prevent.
const CHARS_PER_TOKEN: usize = 3;

/// The marker `providers::LLMClient` returns when it has silently substituted
/// its own canned text for a real completion.
const HEURISTIC_MODEL_MARKER: &str = "heuristic-fallback";

/// Adapts the app's shared `LLMClient` to the pipeline's contract.
///
/// `LLMClient::complete` never returns `Err`: on any provider failure it logs a
/// warning and returns canned filler tagged `model: "heuristic-fallback"`. That
/// is fine for dictation, where some output beats none, and wrong for a meeting
/// summary, where filler presented as a model's work would be validated,
/// persisted, and shown to the user as an AI summary. This adapter therefore
/// goes through `complete_with`, which reports a provider failure as a failure,
/// and still checks the marker as defence in depth.
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
    fn complete_request<'a>(
        &'a self,
        request: LlmRequest<'a>,
    ) -> BoxFuture<'a, Result<LlmOutcome, LlmError>> {
        Box::pin(async move {
            let options = crate::providers::CompletionOptions {
                temperature: request.temperature,
                max_output_tokens: request.max_output_tokens,
                ..self.client.default_options()
            };
            let response = self
                .client
                .complete_with(request.user_prompt, Some(request.system_prompt), options)
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

    fn prompt_budget_chars(&self) -> usize {
        // Reserve room for the system prompt and the model's own answer; what
        // is left is what a transcript may occupy.
        let reserved_tokens = 2_400 + 1_200;
        (self.client.context_tokens().saturating_sub(reserved_tokens) as usize) * CHARS_PER_TOKEN
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
        /// Sampling recorded per call, so a test can assert that extraction ran
        /// cold and prose did not.
        pub requests: Mutex<Vec<(f32, u32)>>,
        budget_chars: usize,
    }

    impl ScriptedLlm {
        /// Responses are consumed in order. Once exhausted, further calls fail
        /// as unavailable rather than silently repeating the last answer.
        pub fn new(responses: Vec<Result<String, LlmError>>) -> Self {
            Self {
                responses: Mutex::new(responses.into_iter().rev().collect()),
                calls: Mutex::new(Vec::new()),
                requests: Mutex::new(Vec::new()),
                // Large enough that a test only chunks when it asks to.
                budget_chars: 1_000_000,
            }
        }

        /// Shrinks the usable window, so chunked extraction can be exercised on
        /// a fixture small enough to read.
        pub fn with_prompt_budget(mut self, chars: usize) -> Self {
            self.budget_chars = chars;
            self
        }

        pub fn always_unavailable() -> Self {
            Self::new(Vec::new())
        }

        pub fn call_count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }
    }

    impl MeetingLlm for ScriptedLlm {
        fn complete_request<'a>(
            &'a self,
            request: LlmRequest<'a>,
        ) -> BoxFuture<'a, Result<LlmOutcome, LlmError>> {
            self.calls.lock().unwrap().push((
                request.system_prompt.to_string(),
                request.user_prompt.to_string(),
            ));
            self.requests
                .lock()
                .unwrap()
                .push((request.temperature, request.max_output_tokens));

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

        fn prompt_budget_chars(&self) -> usize {
            self.budget_chars
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
    async fn an_unreachable_provider_is_a_failure_not_canned_filler() {
        // `LLMClient::complete` masks provider outages with canned filler; the
        // pipeline goes through `complete_with`, which does not, so an outage
        // reaches the pipeline as the failure it is and the deterministic path
        // is chosen deliberately rather than by accident.
        let config = crate::providers::ProviderConfig {
            ollama_host: "http://127.0.0.1:1".to_string(),
            ..Default::default()
        };
        let llm = ProviderLlm::new(config);

        match llm.complete("Return JSON", "some transcript").await {
            Err(LlmError::Unavailable(_)) => {}
            other => panic!("expected an unavailable error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn each_stage_asks_for_the_sampling_its_job_needs() {
        let llm = ScriptedLlm::new(vec![Ok("a".into()), Ok("b".into())]);
        llm.complete_request(LlmRequest::extraction("s", "u"))
            .await
            .unwrap();
        llm.complete_request(LlmRequest::prose("s", "u", 900))
            .await
            .unwrap();

        let requests = llm.requests.lock().unwrap();
        assert_eq!(requests[0].0, 0.1, "extraction runs cold");
        assert_eq!(requests[1].0, 0.3, "prose gets a little room");
        assert_eq!(requests[1].1, 900);
    }

    #[test]
    fn the_prompt_budget_shrinks_with_the_configured_window() {
        let small = ProviderLlm::new(crate::providers::ProviderConfig {
            context_tokens: 8_192,
            ..Default::default()
        });
        let large = ProviderLlm::new(crate::providers::ProviderConfig {
            context_tokens: 32_768,
            ..Default::default()
        });
        assert!(small.prompt_budget_chars() < large.prompt_budget_chars());
        assert!(
            small.prompt_budget_chars() > 10_000,
            "an 8k window must still fit a meaningful stretch of transcript"
        );
    }
}
