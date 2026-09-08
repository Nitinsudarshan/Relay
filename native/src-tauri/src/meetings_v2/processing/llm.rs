//! The processing pipeline's view of a language model.
//!
//! The pipeline never names a provider. It asks for a stage to be run and is
//! told honestly whether a model answered. Ollama, a cloud endpoint, and a
//! future local runtime are all the same to it, and a test can substitute a
//! scripted [`Completer`] without a network.
//!
//! # What this module is now
//!
//! It used to be a second pipeline. `MeetingLlm` was a trait over "send a
//! prompt", `ProviderLlm` adapted the shared client to it, and everything the
//! analysis layer knows about a call — which prompt this is, what version, what
//! it may answer with, what sampling it needs, where the source boundary goes —
//! was re-decided here in prose. That is the duplication M1 set out to remove.
//!
//! What is left is an adapter, and a thin one: it holds a
//! [`AnalysisService`](crate::pipeline::analysis::AnalysisService), names the
//! two registered meeting prompts, and translates the shared failure vocabulary
//! into the one this pipeline's repair loop already speaks. The prompts
//! themselves, the sampling, the JSON gate and the boundary now come from the
//! registry, which means they are the same mechanisms every other analysis in
//! Relay runs on.
//!
//! # What deliberately stayed
//!
//! [`LlmError`] and the two stage methods, because the repair loop and the
//! deterministic floor in `super::mod` are written against them and are the
//! part of this pipeline worth protecting. Migrating the transport did not
//! require rewriting the recovery.

use crate::pipeline::analysis::{
    AnalysisFailure, AnalysisRequest, AnalysisService, AnalysisStage, AnalysisType,
    CanonicalContent, PromptId, SourceDescriptor,
};
use crate::providers::Completer;

/// A completion that actually came from a model.
#[derive(Debug, Clone, PartialEq)]
pub struct LlmOutcome {
    pub text: String,
    pub provider: String,
    pub model: String,
    /// The response parsed as JSON, for a stage whose contract asks for it.
    ///
    /// Parsed by the shared service rather than here. Stage A used to run its
    /// own brace-scan; that rule now lives in
    /// `pipeline::analysis::parse_json_response`, which is where the claim that
    /// there is one definition of "parseable" finally became true.
    pub json: Option<serde_json::Value>,
}

/// Why no completion is available.
///
/// Three variants where the transport has two, because the pipeline acts on the
/// difference. `Empty` earns a second attempt behind a shorter contract;
/// `Unavailable` does not, since a provider that could not be reached will not
/// be reached by a differently-worded prompt either.
#[derive(Debug, Clone, PartialEq)]
pub enum LlmError {
    /// The provider could not be reached, or answered with filler.
    Unavailable(String),
    /// The provider answered, but with nothing usable.
    Empty,
    /// The provider answered in the wrong shape for the stage's contract.
    Unparseable(String),
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(msg) => write!(f, "model unavailable: {}", msg),
            Self::Empty => write!(f, "model returned an empty response"),
            Self::Unparseable(msg) => write!(f, "model returned an unusable response: {}", msg),
        }
    }
}

impl From<AnalysisFailure> for LlmError {
    fn from(failure: AnalysisFailure) -> Self {
        match failure {
            AnalysisFailure::EmptyCompletion => Self::Empty,
            AnalysisFailure::Unparseable(preview) => Self::Unparseable(preview),
            // The rest are all "nothing came back", and the pipeline's answer to
            // every one of them is the same: fall through to the deterministic
            // path and record why.
            other => Self::Unavailable(other.to_string()),
        }
    }
}

/// Runs a meeting's analysis stages on the shared spine.
///
/// Borrows the completer rather than building a client, for the same reason
/// [`AnalysisService`] does: §39 says there is one authoritative provider
/// abstraction, and a type that constructs its own is the first step towards a
/// second one.
///
/// Holds the source descriptor because it is constant for the whole meeting —
/// both stages describe the same recording — and because building it once is
/// what stops the two stages from describing it differently.
pub struct MeetingAnalyst<'a> {
    service: AnalysisService<'a>,
    source: SourceDescriptor<'a>,
    provider: String,
    model: String,
}

impl<'a> MeetingAnalyst<'a> {
    pub fn new(completer: &'a dyn Completer, source: SourceDescriptor<'a>) -> Self {
        Self {
            service: AnalysisService::new(completer),
            source,
            provider: crate::pipeline::analysis::provider_name(completer.provider_type()),
            model: completer.model_name(),
        }
    }

    /// How many characters of transcript one extraction pass may carry.
    ///
    /// Delegates to the service, which sizes it from the configured window and
    /// the prompt's own output allowance. This is the method whose absence on
    /// the shared service was the last thing keeping meetings off it.
    pub fn prompt_budget_chars(&self) -> usize {
        self.service.prompt_budget_chars(PromptId::MeetingFacts)
    }

    /// Reported in stage state and the processing log even when the call fails,
    /// so "which model ran?" is answerable for a failed meeting.
    pub fn provider_name(&self) -> String {
        self.provider.clone()
    }

    pub fn model_name(&self) -> String {
        self.model.clone()
    }

    /// Stage A: one window of transcript becomes facts.
    ///
    /// The response is JSON by contract, so the service parses and rejects
    /// prose before this returns.
    pub async fn extract_facts(
        &self,
        instructions: &str,
        transcript_window: &str,
    ) -> Result<LlmOutcome, LlmError> {
        let request = self
            .request(AnalysisType::Extraction, PromptId::MeetingFacts)
            .at_stage(AnalysisStage::FACTS);
        self.run(&request, instructions, transcript_window).await
    }

    /// Stage B: the facts become prose, with the transcript closed.
    ///
    /// `max_output_tokens` is computed from the length budget the user chose,
    /// so it overrides the registry's default rather than being decided here.
    pub async fn write_prose(
        &self,
        instructions: &str,
        facts: &str,
        max_output_tokens: u32,
    ) -> Result<LlmOutcome, LlmError> {
        let prompt = PromptId::MeetingSummary.definition();
        let base = crate::providers::CompletionOptions {
            max_output_tokens,
            ..prompt.options(self.default_options())
        };
        let request = self
            .request(AnalysisType::Summary, PromptId::MeetingSummary)
            .at_stage(AnalysisStage::PROSE)
            .with_options(base);
        self.run(&request, instructions, facts).await
    }

    fn request(&self, analysis_type: AnalysisType, prompt_id: PromptId) -> AnalysisRequest<'_> {
        AnalysisRequest::new(&self.source, analysis_type, prompt_id)
    }

    fn default_options(&self) -> crate::providers::CompletionOptions {
        self.service.provider_defaults()
    }

    async fn run(
        &self,
        request: &AnalysisRequest<'_>,
        instructions: &str,
        body: &str,
    ) -> Result<LlmOutcome, LlmError> {
        // No title: a meeting's prompt body is the rendered transcript or the
        // rendered facts, and `CanonicalContent` prepends nothing to markdown
        // with no segments. The stage prompts already say what they are reading.
        let content = CanonicalContent::from_markdown("", body);

        let executed = self
            .service
            .execute_computed(request, &self.source, &content, instructions)
            .await?;

        Ok(LlmOutcome {
            text: executed.response.text,
            provider: executed.provider,
            model: executed.response.model,
            json: executed.json,
        })
    }
}

#[cfg(test)]
pub mod test_support {
    use super::*;
    use crate::pipeline::analysis::SourceType;
    use crate::providers::{
        BoxFuture, CompletionOptions, LLMResponse, ProviderError, ProviderType,
    };
    use std::sync::Mutex;

    /// A model stand-in that replays queued responses.
    ///
    /// Still here, and still driving the three meeting suites, but pointed at
    /// [`Completer`] rather than at a meetings-only trait. That is the whole
    /// difference M1 makes to the tests: they now exercise the same service
    /// production runs, including its prompt registry, its boundary and its
    /// JSON gate, instead of a parallel path that merely resembled it.
    pub struct ScriptedLlm {
        responses: Mutex<Vec<Result<String, LlmError>>>,
        /// System and user prompt per call, so a test can assert what the model
        /// was actually shown.
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
        ///
        /// Expressed as a context window rather than a character count now,
        /// because the budget is the service's to compute. The arithmetic is
        /// inverted here so a test can still say "give me N characters".
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

        /// An analyst backed by this script, over a synthetic meeting.
        pub fn analyst(&self) -> MeetingAnalyst<'_> {
            MeetingAnalyst::new(self, SourceDescriptor::synthetic("meeting", SourceType::Meeting))
        }
    }

    /// The context window that yields `chars` of Stage A prompt budget.
    ///
    /// The inverse of [`AnalysisService::prompt_budget_chars`], so a test can
    /// go on asking for a character count now that the budget is the service's
    /// to compute. `the_test_helper_inverts_the_budget_it_stands_in_for` is
    /// what stops the two drifting apart.
    fn budget_to_window(chars: usize) -> u32 {
        const CHARS_PER_TOKEN: usize = 3;
        const INSTRUCTION_RESERVE_TOKENS: u32 = 1_200;
        let source_tokens = chars.div_ceil(CHARS_PER_TOKEN) as u32;
        source_tokens
            .saturating_add(PromptId::MeetingFacts.definition().max_output_tokens)
            .saturating_add(INSTRUCTION_RESERVE_TOKENS)
    }

    impl Completer for ScriptedLlm {
        fn default_options(&self) -> CompletionOptions {
            // The window the service will subtract the prompt's output
            // allowance from. Chosen so `prompt_budget_chars` lands on the
            // character count the test asked for.
            CompletionOptions {
                context_tokens: budget_to_window(self.budget_chars),
                ..CompletionOptions::default()
            }
        }

        fn provider_type(&self) -> &ProviderType {
            &ProviderType::Ollama
        }

        fn model_name(&self) -> String {
            "scripted-model".to_string()
        }

        fn complete_verified<'a>(
            &'a self,
            prompt: &'a str,
            system_prompt: Option<&'a str>,
            options: CompletionOptions,
        ) -> BoxFuture<'a, Result<LLMResponse, ProviderError>> {
            self.calls
                .lock()
                .unwrap()
                .push((system_prompt.unwrap_or_default().to_string(), prompt.to_string()));
            self.requests
                .lock()
                .unwrap()
                .push((options.temperature, options.max_output_tokens));

            let next = self.responses.lock().unwrap().pop();
            Box::pin(async move {
                match next {
                    Some(Ok(text)) => Ok(LLMResponse {
                        text,
                        model: "scripted-model".to_string(),
                        prompt_tokens: None,
                        completion_tokens: None,
                    }),
                    Some(Err(LlmError::Empty)) => Err(ProviderError::EmptyCompletion),
                    Some(Err(e)) => Err(ProviderError::NoCompletion(e.to_string())),
                    None => Err(ProviderError::NoCompletion(
                        "no scripted response left".to_string(),
                    )),
                }
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::ScriptedLlm;
    use super::*;
    use crate::pipeline::analysis::SourceType;
    use crate::providers::{LLMClient, ProviderConfig};

    fn meeting() -> SourceDescriptor<'static> {
        SourceDescriptor::synthetic("meeting", SourceType::Meeting)
    }

    #[tokio::test]
    async fn scripted_responses_are_replayed_in_order_then_fail() {
        let llm = ScriptedLlm::new(vec![Ok("first".into()), Ok("second".into())]);
        let analyst = llm.analyst();

        assert_eq!(analyst.write_prose("s", "u", 600).await.unwrap().text, "first");
        assert_eq!(analyst.write_prose("s", "u", 600).await.unwrap().text, "second");
        assert!(matches!(
            analyst.write_prose("s", "u", 600).await,
            Err(LlmError::Unavailable(_))
        ));
        assert_eq!(llm.call_count(), 3);
    }

    #[tokio::test]
    async fn an_unreachable_provider_is_a_failure_not_canned_filler() {
        // `LLMClient::complete` masks provider outages with canned filler; the
        // service goes through `complete_verified`, which does not, so an outage
        // reaches the pipeline as the failure it is and the deterministic path
        // is chosen deliberately rather than by accident.
        let client = LLMClient::new(ProviderConfig {
            ollama_host: "http://127.0.0.1:1".to_string(),
            ..Default::default()
        });
        let analyst = MeetingAnalyst::new(&client, meeting());

        match analyst.write_prose("Write prose", "some facts", 600).await {
            Err(LlmError::Unavailable(_)) => {}
            other => panic!("expected an unavailable error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn each_stage_asks_for_the_sampling_its_job_needs() {
        // Now a statement about the prompt registry rather than about this
        // module: neither number is written here any more, and this is the test
        // that would catch the registry drifting away from what ships.
        let llm = ScriptedLlm::new(vec![Ok("{\"title\":\"x\"}".into()), Ok("prose".into())]);
        let analyst = llm.analyst();
        analyst.extract_facts("s", "u").await.unwrap();
        analyst.write_prose("s", "u", 900).await.unwrap();

        let requests = llm.requests.lock().unwrap();
        assert_eq!(requests[0].0, 0.1, "extraction runs cold");
        assert_eq!(requests[0].1, 2_400);
        assert_eq!(requests[1].0, 0.3, "prose gets a little room");
        assert_eq!(requests[1].1, 900, "and the caller's computed budget wins");
    }

    #[tokio::test]
    async fn a_prose_answer_to_stage_a_is_refused_by_the_contract() {
        // Stage A's JSON contract is the registry's now, and it is applied
        // before this returns rather than discovered by a parse that fails.
        let llm = ScriptedLlm::new(vec![Ok("I could not find any facts.".into())]);
        assert!(matches!(
            llm.analyst().extract_facts("s", "u").await,
            Err(LlmError::Unparseable(_))
        ));
    }

    #[tokio::test]
    async fn an_empty_answer_stays_distinguishable_from_an_outage() {
        // The distinction Stage B's compact-contract retry is built on. It now
        // travels provider → `ProviderError` → `AnalysisFailure` → here, and
        // every one of those hops had to preserve it.
        let empty = ScriptedLlm::new(vec![Err(LlmError::Empty)]);
        assert!(matches!(
            empty.analyst().write_prose("s", "u", 600).await,
            Err(LlmError::Empty)
        ));

        let down = ScriptedLlm::new(vec![Err(LlmError::Unavailable("refused".into()))]);
        assert!(matches!(
            down.analyst().write_prose("s", "u", 600).await,
            Err(LlmError::Unavailable(_))
        ));
    }

    #[test]
    fn the_prompt_budget_shrinks_with_the_configured_window() {
        let small = LLMClient::new(ProviderConfig {
            context_tokens: 8_192,
            ..Default::default()
        });
        let large = LLMClient::new(ProviderConfig {
            context_tokens: 32_768,
            ..Default::default()
        });
        let small_budget = MeetingAnalyst::new(&small, meeting()).prompt_budget_chars();
        let large_budget = MeetingAnalyst::new(&large, meeting()).prompt_budget_chars();

        assert!(small_budget < large_budget);
        assert!(
            small_budget > 10_000,
            "an 8k window must still fit a meaningful stretch of transcript"
        );
    }

    #[test]
    fn the_migrated_budget_is_the_one_the_pipeline_always_had() {
        // The number this replaced: window, less 2,400 tokens of answer and
        // 1,200 of contract, at three characters a token. Computed from the
        // registry now, and it has to come out the same — a migration that
        // silently re-chunks every long meeting is not a migration.
        let client = LLMClient::new(ProviderConfig {
            context_tokens: 8_192,
            ..Default::default()
        });
        assert_eq!(
            MeetingAnalyst::new(&client, meeting()).prompt_budget_chars(),
            (8_192 - (2_400 + 1_200)) * 3
        );
    }

    #[test]
    fn the_test_helper_inverts_the_budget_it_stands_in_for() {
        // `with_prompt_budget` lets a test ask for characters while the service
        // computes them from a window. If the two ever disagree, chunked
        // extraction is being exercised at a size nobody chose.
        for chars in [12_000usize, 30_000, 120_000] {
            let llm = ScriptedLlm::new(Vec::new()).with_prompt_budget(chars);
            assert_eq!(llm.analyst().prompt_budget_chars(), chars);
        }
    }
}
