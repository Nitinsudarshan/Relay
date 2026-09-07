//! The one place an analysis is executed.
//!
//! Everything that is the same for every analysis lives here: resolving the
//! prompt, applying the source boundary, choosing sampling, calling the
//! provider, parsing, and building the result metadata. What is *not* here is
//! any source-specific meaning — the service never knows what a repository
//! stack is. It hands validated text or JSON back, and the caller's own
//! builder turns that into a `RepositoryContext` or a `ConversationContext`.
//!
//! That split is the whole design. Adding a new analysis should mean writing a
//! prompt, a payload type and a builder — not another capture → normalize →
//! prompt → LLM → parse → persist → provenance pipeline.

use crate::providers::{Completer, ProviderError, ProviderType};

use super::content::CanonicalContent;
use super::contract::{
    AnalysisFailure, AnalysisRequest, AnalysisResult, AnalysisType, MetadataBuilder,
};
use super::prompts::PromptId;
use super::source::SourceDescriptor;
use crate::pipeline::source_boundary;

/// Characters per token, deliberately on the low side.
///
/// Under-estimating wastes a little of the window. Over-estimating silently
/// truncates the source, which is the failure the budget exists to prevent, so
/// the error is taken in the harmless direction.
const CHARS_PER_TOKEN: usize = 3;

/// Window reserved for the instructions themselves.
///
/// The instructions are not free: a meeting's extraction contract is a
/// substantial prompt in its own right, and a budget that forgot it would hand
/// the provider a prompt that overflows by exactly the size of the contract.
const INSTRUCTION_RESERVE_TOKENS: u32 = 1_200;

/// Executes analyses against a provider.
///
/// Holds a borrowed client rather than building its own: §39 says there is one
/// authoritative provider abstraction, and a service that constructs its own
/// `LLMClient` is the first step towards a second one.
pub struct AnalysisService<'a> {
    llm: Option<&'a dyn Completer>,
}

impl<'a> AnalysisService<'a> {
    /// A service backed by a provider.
    ///
    /// Takes the [`Completer`] seam rather than the concrete client, so a
    /// caller with a scripted stand-in can exercise this service's failure
    /// paths. `&LLMClient` still coerces, so production call sites read
    /// unchanged.
    pub fn new(llm: &'a dyn Completer) -> Self {
        Self { llm: Some(llm) }
    }

    /// A service with no provider at all.
    ///
    /// Every analysis fails with `NoCompletion`, which is what lets callers
    /// exercise the fallback path — and lets tests do so without a network.
    pub fn offline() -> Self {
        Self { llm: None }
    }

    /// Runs one analysis and returns the raw model text, validated against the
    /// prompt's output contract.
    ///
    /// Returns `Err(AnalysisFailure)` for every reason nothing usable came
    /// back, so the caller decides between falling back and surfacing the
    /// failure — but can no longer fail to notice which happened.
    pub async fn execute(
        &self,
        request: &AnalysisRequest<'_>,
        source: &SourceDescriptor<'_>,
        content: &CanonicalContent,
    ) -> Result<ExecutedAnalysis, AnalysisFailure> {
        self.execute_with(request, source, content, None).await
    }

    /// As [`execute`], for a prompt whose instructions are built per call.
    ///
    /// The escape hatch that lets a staged analysis onto this service without
    /// the registry having to hold a body it cannot express as a constant. The
    /// caller supplies the text and the registry still supplies everything
    /// else — identity, version, output contract, sampling, applicability.
    ///
    /// [`execute`]: Self::execute
    pub async fn execute_computed(
        &self,
        request: &AnalysisRequest<'_>,
        source: &SourceDescriptor<'_>,
        content: &CanonicalContent,
        instructions: &str,
    ) -> Result<ExecutedAnalysis, AnalysisFailure> {
        self.execute_with(request, source, content, Some(instructions))
            .await
    }

    async fn execute_with(
        &self,
        request: &AnalysisRequest<'_>,
        source: &SourceDescriptor<'_>,
        content: &CanonicalContent,
        computed_instructions: Option<&str>,
    ) -> Result<ExecutedAnalysis, AnalysisFailure> {
        let prompt = request.prompt_id.definition();

        // Applicability is checked before the call, not after: sending the
        // repository prompt to a conversation costs a request and returns a
        // confidently wrong answer.
        if !prompt.applies_to_source(request.source_type) {
            return Err(AnalysisFailure::PromptNotApplicable(format!(
                "{} does not apply to a {} source",
                request.prompt_id.as_str(),
                request.source_type.as_str()
            )));
        }

        if content.is_empty() {
            return Err(AnalysisFailure::EmptySource);
        }

        // A computed prompt's instructions come from the caller, because they
        // cannot be a constant — see `PromptBody::Computed`. The registry still
        // decided the output contract and the sampling that got us here.
        let instructions = match (prompt.static_instructions(), computed_instructions) {
            (Some(fixed), None) => fixed.to_string(),
            (None, Some(supplied)) => supplied.to_string(),
            (None, None) => {
                return Err(AnalysisFailure::NoCompletion(format!(
                    "{} is a computed prompt and no instructions were supplied",
                    request.prompt_id.as_str()
                )))
            }
            // Guards the confusing case rather than silently preferring one:
            // a caller passing instructions for a prompt that has its own has
            // misunderstood which prompt they are running.
            (Some(_), Some(_)) => {
                return Err(AnalysisFailure::NoCompletion(format!(
                    "{} has its own instructions; supplied ones would be ignored",
                    request.prompt_id.as_str()
                )))
            }
        };

        let client = self.llm.ok_or_else(|| {
            AnalysisFailure::NoCompletion("no provider is configured".to_string())
        })?;

        // The boundary is applied from the source's trust level, not from the
        // caller's memory. A captured page is external whatever its domain.

        let body = content.as_prompt_body();
        let (user_prompt, mut system_prompt) = if source.trust.is_external() {
            (
                source_boundary::wrap_external_source(&source.describe_origin(), &body).framed,
                format!("{}\n{}", instructions, source_boundary::EXTERNAL_SOURCE_RULE),
            )
        } else {
            (body, instructions)
        };

        // §29: what capture already knows about completeness reaches the model,
        // instead of stopping at the UI.
        if let Some(caveat) = CanonicalContent::coverage_caveat(source) {
            system_prompt.push('\n');
            system_prompt.push_str(&caveat);
        }

        let options = request
            .options
            .unwrap_or_else(|| prompt.options(client.default_options()));

        let response = client
            .complete_verified(&user_prompt, Some(&system_prompt), options)
            .await
            .map_err(|err| match err {
                ProviderError::NoCompletion(msg) => AnalysisFailure::NoCompletion(msg),
                // Kept distinct all the way to the caller: a second, shorter
                // prompt is worth sending after silence and worthless after an
                // outage, and only this variant tells them apart.
                ProviderError::EmptyCompletion => AnalysisFailure::EmptyCompletion,
                other => AnalysisFailure::NoCompletion(other.to_string()),
            })?;

        // §24: structured output is parsed and validated before anything
        // downstream sees it. Prose that happens to be returned for a JSON
        // prompt is a failed analysis, not a successful one with odd content.
        let json = if prompt.expects_json() {
            Some(
                parse_json_response(&response.text)
                    .ok_or_else(|| AnalysisFailure::Unparseable(preview(&response.text)))?,
            )
        } else {
            None
        };

        Ok(ExecutedAnalysis {
            provider: provider_name(client.provider_type()),
            response,
            json,
        })
    }

    /// The provider's own sampling defaults.
    ///
    /// For the caller that has to build `CompletionOptions` itself because one
    /// field of them is computed — a meeting's prose budget comes from the
    /// length the user chose, so it cannot live in the registry.
    pub fn provider_defaults(&self) -> crate::providers::CompletionOptions {
        self.llm
            .map(|client| client.default_options())
            .unwrap_or_default()
    }

    /// How many characters of *source* a prompt may be handed.
    ///
    /// Not a nicety, and the reason this is on the service rather than left to
    /// callers: a provider handed more than its window silently discards the
    /// overflow — from the front, on Ollama — and nothing in the response says
    /// it happened. An analysis over a long source has to be split into passes,
    /// and it can only size those passes if something tells it the window.
    ///
    /// Computed per prompt rather than as one constant, because the answer
    /// genuinely differs: what is left for the source is the window minus what
    /// the model is allowed to write back, and a prompt that may answer with
    /// 2,400 tokens leaves less than one capped at 900.
    ///
    /// This is the shared-spine equivalent of the `prompt_budget_chars` that
    /// `meetings_v2::processing::llm::MeetingLlm` carried, and the last thing
    /// that kept the meeting pipeline off this service.
    pub fn prompt_budget_chars(&self, prompt_id: PromptId) -> usize {
        let window = self
            .llm
            .map(|client| client.default_options().context_tokens)
            .unwrap_or_else(|| crate::providers::CompletionOptions::default().context_tokens);
        prompt_budget_chars_for(prompt_id, window)
    }

    /// Builds the metadata for a result produced by this service.
    pub fn metadata_builder(
        request: &AnalysisRequest<'_>,
        source: &SourceDescriptor<'_>,
    ) -> MetadataBuilder {
        let version = request.prompt_id.definition().version;
        MetadataBuilder::new(request.analysis_type, request.prompt_id, version)
            .with_coverage(source.coverage)
    }

    /// The whole lifecycle for an analysis whose payload the caller can build
    /// from validated JSON, with a deterministic fallback when nothing
    /// answered.
    ///
    /// `build` turns the validated response into the payload. `fallback`
    /// produces one without a model. The result records which happened, so a
    /// fallback can never read back as a model's work.
    pub async fn run_structured<T, B, F>(
        &self,
        request: &AnalysisRequest<'_>,
        source: &SourceDescriptor<'_>,
        content: &CanonicalContent,
        build: B,
        fallback: F,
    ) -> AnalysisResult<T>
    where
        B: FnOnce(&serde_json::Value) -> Option<T>,
        F: FnOnce() -> T,
    {
        let builder = Self::metadata_builder(request, source);

        match self.execute(request, source, content).await {
            Ok(executed) => {
                let json = executed.json.as_ref().expect("a JSON prompt yields JSON");
                match build(json) {
                    Some(payload) => AnalysisResult {
                        source_id: request.source_id.to_string(),
                        metadata: builder.succeeded(&executed.provider, &executed.response),
                        payload: Some(payload),
                    },
                    None => {
                        let failure =
                            AnalysisFailure::ValidationFailed(preview(&executed.response.text));
                        tracing::warn!(
                            "Analysis {} for source {} failed validation; using deterministic fallback",
                            request.prompt_id.as_str(),
                            request.source_id
                        );
                        AnalysisResult {
                            source_id: request.source_id.to_string(),
                            metadata: builder.deterministic(failure),
                            payload: Some(fallback()),
                        }
                    }
                }
            }
            Err(failure) => {
                tracing::warn!(
                    "Analysis {} for source {} did not complete ({}); using deterministic fallback",
                    request.prompt_id.as_str(),
                    request.source_id,
                    failure
                );
                AnalysisResult {
                    source_id: request.source_id.to_string(),
                    metadata: builder.deterministic(failure),
                    payload: Some(fallback()),
                }
            }
        }
    }

    /// The prose equivalent: no JSON contract, and no fallback content
    /// invented here. A failed prose analysis returns a failed result, because
    /// there is no honest substitute for a summary nothing produced.
    pub async fn run_prose(
        &self,
        request: &AnalysisRequest<'_>,
        source: &SourceDescriptor<'_>,
        content: &CanonicalContent,
    ) -> AnalysisResult<String> {
        let builder = Self::metadata_builder(request, source);

        match self.execute(request, source, content).await {
            Ok(executed) => {
                let text = executed.response.text.trim().trim_matches('"').to_string();
                if text.is_empty() {
                    return AnalysisResult {
                        source_id: request.source_id.to_string(),
                        metadata: builder
                            .failed(AnalysisFailure::NoCompletion("empty response".to_string())),
                        payload: None,
                    };
                }
                AnalysisResult {
                    source_id: request.source_id.to_string(),
                    metadata: builder.succeeded(&executed.provider, &executed.response),
                    payload: Some(text),
                }
            }
            Err(failure) => AnalysisResult {
                source_id: request.source_id.to_string(),
                metadata: builder.failed(failure),
                payload: None,
            },
        }
    }
}

/// A completed provider call, with its response parsed if the prompt asked for
/// JSON.
#[derive(Debug)]
pub struct ExecutedAnalysis {
    pub provider: String,
    pub response: crate::providers::LLMResponse,
    pub json: Option<serde_json::Value>,
}

/// Convenience: the context analysis for whatever this source is.
///
/// Returns `None` when no context prompt applies, which is the honest answer
/// for a source type Relay has no context schema for yet — and is what the old
/// `if github { … } else { … }` could not express.
pub fn context_request<'a>(source: &SourceDescriptor<'a>) -> Option<AnalysisRequest<'a>> {
    let prompt_id: PromptId = super::prompts::context_prompt_for(source.source_type)?;
    Some(AnalysisRequest::new(source, AnalysisType::Context, prompt_id))
}

/// [`AnalysisService::prompt_budget_chars`] for a window the caller already
/// knows.
///
/// A free function because not every caller holds a service. Talkback sizes its
/// retrieval from its own latency policy and then bounds it by this, so a
/// surface that decides how much evidence it *wants* still cannot ask for more
/// than the window can carry.
pub fn prompt_budget_chars_for(prompt_id: PromptId, context_tokens: u32) -> usize {
    let reserved = prompt_id
        .definition()
        .max_output_tokens
        .saturating_add(INSTRUCTION_RESERVE_TOKENS);

    context_tokens.saturating_sub(reserved) as usize * CHARS_PER_TOKEN
}

/// The provider family's stable name, for the provenance record.
///
/// Public because a caller that records provenance for a *failed* call has no
/// `ExecutedAnalysis` to read it off, and a second copy of this mapping is a
/// second thing to get wrong.
pub fn provider_name(provider: &ProviderType) -> String {
    match provider {
        ProviderType::Ollama => "ollama",
        ProviderType::CloudOpenAI => "cloud_openai",
        ProviderType::CloudGemini => "cloud_gemini",
        ProviderType::CloudAnthropic => "cloud_anthropic",
    }
    .to_string()
}

/// Extracts a JSON document from a model response, tolerating markdown fences.
///
/// Public because it is the single parse step §24 asks for: every structured
/// analysis goes through this one, so "what counts as a parseable response" is
/// answered in one place rather than re-implemented per analysis.
///
/// Returns `None` for anything that is not a JSON **object** — an array or a
/// bare scalar is not a structured analysis, and accepting one is how filler
/// shaped like `[{...}]` gets treated as an answer.
pub fn parse_json_response(raw: &str) -> Option<serde_json::Value> {
    let text = raw.trim();
    let candidate = if let Some(after) = text.split("```json").nth(1) {
        after.split("```").next()?.trim()
    } else if text.contains("```") {
        text.split("```").nth(1)?.split("```").next()?.trim()
    } else {
        text
    };

    // Whatever the candidate parses as, its shape decides. An array or a scalar
    // is well-formed JSON and still not a structured analysis, and it must not
    // reach the rescue below — digging an object out of `[{…}]` is precisely how
    // filler shaped like a result gets treated as one.
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(candidate) {
        return value.is_object().then_some(value);
    }

    // Only now, with the response established as not-JSON-at-all: the outermost
    // braces in it.
    //
    // A small local model asked for strict JSON sometimes delivers it wrapped in
    // a sentence — "Here is the JSON: {…}" — and refusing that throws away an
    // answer that is entirely present. `meetings_v2::processing::extract` has
    // always been this tolerant privately, which is precisely why the claim
    // above ("answered in one place rather than re-implemented per analysis")
    // was not yet true. Folding the rule in here makes it true. It is narrower
    // than the private version it replaces, which scanned braces unconditionally
    // and so would rescue an object out of an array; the shared contract has
    // always refused that, and goes on refusing it.
    let start = candidate.find('{')?;
    let end = candidate.rfind('}')?;
    if end <= start {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(&candidate[start..=end]).ok()?;
    value.is_object().then_some(value)
}

/// A short excerpt of a bad response, for the failure record. Bounded because
/// this ends up in a stored artifact and a log line.
fn preview(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= 200 {
        return trimmed.to_string();
    }
    let cut: String = trimmed.chars().take(200).collect();
    format!("{cut}…")
}

#[cfg(test)]
mod computed_prompt_tests {
    use super::*;
    use crate::pipeline::analysis::{AnalysisType, PromptId, SourceType};

    use crate::providers::{BoxFuture, CompletionOptions, LLMResponse, ProviderType};
    use std::sync::Mutex;

    /// A [`Completer`] that replays queued answers.
    ///
    /// The reason the trait exists. Before it, this service took a concrete
    /// `LLMClient`, so its failure paths could only be reached with a network
    /// or an Ollama instance — which meant they were not reached. It also
    /// records the system prompt it was handed, which is how a computed
    /// prompt's instructions can be asserted to have actually arrived.
    struct ScriptedCompleter {
        answers: Mutex<Vec<Result<String, String>>>,
        seen_system_prompts: Mutex<Vec<String>>,
    }

    impl ScriptedCompleter {
        fn replying(answers: Vec<Result<String, String>>) -> Self {
            Self {
                answers: Mutex::new(answers),
                seen_system_prompts: Mutex::new(Vec::new()),
            }
        }

        fn last_system_prompt(&self) -> String {
            self.seen_system_prompts
                .lock()
                .expect("not poisoned")
                .last()
                .cloned()
                .unwrap_or_default()
        }
    }

    impl Completer for ScriptedCompleter {
        fn default_options(&self) -> CompletionOptions {
            CompletionOptions::default()
        }

        fn provider_type(&self) -> &ProviderType {
            &ProviderType::Ollama
        }

        fn model_name(&self) -> String {
            "scripted".to_string()
        }

        fn complete_verified<'a>(
            &'a self,
            _prompt: &'a str,
            system_prompt: Option<&'a str>,
            _options: CompletionOptions,
        ) -> BoxFuture<'a, Result<LLMResponse, ProviderError>> {
            self.seen_system_prompts
                .lock()
                .expect("not poisoned")
                .push(system_prompt.unwrap_or_default().to_string());
            let next = self
                .answers
                .lock()
                .expect("not poisoned")
                .pop()
                .unwrap_or_else(|| Err("the script ran out of answers".to_string()));
            Box::pin(async move {
                match next {
                    Ok(text) => Ok(LLMResponse {
                        text,
                        model: "scripted".to_string(),
                        prompt_tokens: None,
                        completion_tokens: None,
                    }),
                    Err(message) => Err(ProviderError::NoCompletion(message)),
                }
            })
        }
    }

    #[tokio::test]
    async fn a_computed_prompt_reaches_the_model_with_the_supplied_instructions() {
        // End to end through the seam: this is what a migrated meeting Stage A
        // does, and what could not be tested before the trait existed.
        let completer = ScriptedCompleter::replying(vec![Ok(
            r#"{"key_points": [], "action_items": []}"#.to_string()
        )]);
        let source = SourceDescriptor::synthetic("m1", SourceType::Meeting);
        let request =
            AnalysisRequest::new(&source, AnalysisType::Extraction, PromptId::MeetingFacts)
                .at_stage(crate::pipeline::analysis::AnalysisStage::FACTS);

        let executed = AnalysisService::new(&completer)
            .execute_computed(
                &request,
                &source,
                &meeting_content(),
                "EXTRACT FACTS AS JSON. Invent nothing.",
            )
            .await
            .expect("the scripted answer is valid JSON for a JSON contract");

        assert!(executed.json.is_some(), "a JSON contract parses its answer");
        assert_eq!(executed.provider, "ollama");
        assert!(
            completer
                .last_system_prompt()
                .contains("EXTRACT FACTS AS JSON"),
            "the supplied instructions must be what the model was sent, got {:?}",
            completer.last_system_prompt()
        );
    }

    #[tokio::test]
    async fn prose_returned_for_a_json_contract_is_a_failed_analysis() {
        // §24. A failure path that needed a network to reach until now.
        let completer =
            ScriptedCompleter::replying(vec![Ok("Three interviews happened.".to_string())]);
        let source = SourceDescriptor::synthetic("m1", SourceType::Meeting);
        let request =
            AnalysisRequest::new(&source, AnalysisType::Extraction, PromptId::MeetingFacts);

        let err = AnalysisService::new(&completer)
            .execute_computed(&request, &source, &meeting_content(), "instructions")
            .await
            .expect_err("prose does not satisfy a JSON contract");
        assert!(matches!(err, AnalysisFailure::Unparseable(_)), "{err:?}");
    }

    #[tokio::test]
    async fn a_provider_failure_is_reported_as_one_rather_than_as_an_answer() {
        let completer = ScriptedCompleter::replying(vec![Err("the model timed out".to_string())]);
        let source = SourceDescriptor::synthetic("m1", SourceType::Meeting);
        let request =
            AnalysisRequest::new(&source, AnalysisType::Summary, PromptId::MeetingSummary);

        let err = AnalysisService::new(&completer)
            .execute_computed(&request, &source, &meeting_content(), "write prose")
            .await
            .expect_err("a timeout is not a summary");
        match err {
            AnalysisFailure::NoCompletion(m) => assert!(m.contains("timed out"), "{m}"),
            other => panic!("expected NoCompletion, got {other:?}"),
        }
    }

    fn meeting_content() -> CanonicalContent {
        CanonicalContent::from_markdown("Volunteer interviews", "Three interviews happened.")
    }

    #[tokio::test]
    async fn a_computed_prompt_without_instructions_is_refused() {
        // The failure mode this guard exists for: sending a model an empty
        // system prompt and treating whatever comes back as an analysis.
        let source = SourceDescriptor::synthetic("m1", SourceType::Meeting);
        let request =
            AnalysisRequest::new(&source, AnalysisType::Extraction, PromptId::MeetingFacts);
        let err = AnalysisService::offline()
            .execute(&request, &source, &meeting_content())
            .await
            .expect_err("a computed prompt has no body of its own");

        match err {
            AnalysisFailure::NoCompletion(m) => {
                assert!(m.contains("meeting.facts"), "{m}");
                assert!(m.contains("computed"), "{m}");
            }
            other => panic!("expected NoCompletion, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn instructions_supplied_for_a_static_prompt_are_refused_not_ignored() {
        // Silently preferring one body over the other would hide a caller
        // running the wrong prompt. Refusing says so.
        let source = SourceDescriptor::synthetic("doc1", SourceType::Document);
        let request = AnalysisRequest::new(&source, AnalysisType::Summary, PromptId::Summary);
        let err = AnalysisService::offline()
            .execute_computed(&request, &source, &meeting_content(), "my own instructions")
            .await
            .expect_err("Summary carries its own instructions");

        match err {
            AnalysisFailure::NoCompletion(m) => assert!(m.contains("its own instructions"), "{m}"),
            other => panic!("expected NoCompletion, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_meeting_prompt_is_refused_on_a_source_that_is_not_a_meeting() {
        // Applicability is still checked for the new entries, and before the
        // instructions are resolved — a wrong-source call must not depend on
        // whether the caller remembered a body.
        let source = SourceDescriptor::synthetic("doc1", SourceType::Document);
        let request =
            AnalysisRequest::new(&source, AnalysisType::Summary, PromptId::MeetingSummary);
        let err = AnalysisService::offline()
            .execute_computed(&request, &source, &meeting_content(), "instructions")
            .await
            .expect_err("a document is not a meeting");
        assert!(matches!(err, AnalysisFailure::PromptNotApplicable(_)), "{err:?}");
    }

    #[test]
    fn the_two_meeting_prompts_are_sampled_for_what_they_produce() {
        // Extraction invents an owner the moment it has room to be creative;
        // prose needs a little. This is the per-stage sampling the meeting
        // pipeline had and the registry previously could not express.
        let facts = PromptId::MeetingFacts.definition();
        let prose = PromptId::MeetingSummary.definition();
        assert!(facts.expects_json());
        assert!(!prose.expects_json());
        assert!(
            facts.temperature < prose.temperature,
            "extraction must be the colder of the two"
        );
        assert!(facts.is_computed() && prose.is_computed());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::web::{CaptureCoverage, CaptureProvenance};
    use crate::pipeline::analysis::contract::AnalysisStatus;
    use crate::vault::VaultFile;

    fn provenance(capture_type: &str, coverage: CaptureCoverage) -> CaptureProvenance {
        CaptureProvenance {
            source_type: "web".to_string(),
            capture_type: capture_type.to_string(),
            application: "GitHub".to_string(),
            domain: "github.com".to_string(),
            url: "https://github.com/owner/repo".to_string(),
            page_title: "owner/repo".to_string(),
            captured_at: "2026-09-03T10:00:00Z".to_string(),
            browser_captured_at: None,
            browser: None,
            extractor_id: "github".to_string(),
            extractor_version: 1,
            trust: "external_untrusted".to_string(),
            fidelity: "structured".to_string(),
            coverage,
            notes: vec!["Only the README was reached.".to_string()],
            message_count: None,
            block_count: 3,
            skipped_block_count: 0,
            truncated: false,
            canonical_url: None,
            author: None,
            published_at: None,
            language: None,
            version: 1,
            previous_capture_id: None,
            recapture_count: 0,
            traversal: None,
        }
    }

    fn repo_file(coverage: CaptureCoverage) -> VaultFile {
        VaultFile::new_capture(
            "cap_1".to_string(),
            "capture.json".to_string(),
            "captures/cap_1".to_string(),
            "# owner/repo".to_string(),
            "hash".to_string(),
            provenance("repository", coverage),
        )
    }

    #[test]
    fn json_parsing_accepts_objects_and_fenced_objects() {
        assert!(parse_json_response(r#"{"a":1}"#).is_some());
        assert!(parse_json_response("```json\n{\"a\":1}\n```").is_some());
        assert!(parse_json_response("```\n{\"a\":1}\n```").is_some());
    }

    /// The client's own filler for a JSON prompt is a JSON *array*. Accepting
    /// non-objects would let that through as a structured analysis whose every
    /// field defaulted to empty.
    #[test]
    fn json_parsing_rejects_arrays_scalars_and_prose() {
        for bad in [
            r#"[{"title":"Follow up","assignee":"Unassigned"}]"#,
            "\"just a string\"",
            "42",
            "Here is your analysis, in prose.",
            "",
        ] {
            assert!(
                parse_json_response(bad).is_none(),
                "should not accept as structured output: {bad}"
            );
        }
    }

    #[tokio::test]
    async fn an_offline_service_reports_no_completion_rather_than_inventing_one() {
        let file = repo_file(CaptureCoverage::FullDocument);
        let source = SourceDescriptor::from_vault_file(&file);
        let request = context_request(&source).expect("a repository has a context prompt");
        let content = CanonicalContent::from_markdown("owner/repo", "# owner/repo\n\nA tool.");

        let failure = AnalysisService::offline()
            .execute(&request, &source, &content)
            .await
            .expect_err("no provider means no completion");

        assert!(matches!(failure, AnalysisFailure::NoCompletion(_)));
    }

    #[tokio::test]
    async fn an_empty_source_is_reported_as_empty_not_analysed() {
        let file = repo_file(CaptureCoverage::FullDocument);
        let source = SourceDescriptor::from_vault_file(&file);
        let request = context_request(&source).unwrap();
        let content = CanonicalContent::from_markdown("owner/repo", "   ");

        let failure = AnalysisService::offline()
            .execute(&request, &source, &content)
            .await
            .expect_err("empty content cannot be analysed");
        assert!(matches!(failure, AnalysisFailure::EmptySource));
    }

    /// §47 — a prompt written for one source type must be refused for another,
    /// before a request is spent on it.
    #[tokio::test]
    async fn a_mismatched_prompt_is_refused_before_the_provider_is_called() {
        let file = repo_file(CaptureCoverage::FullDocument);
        let source = SourceDescriptor::from_vault_file(&file);
        let content = CanonicalContent::from_markdown("owner/repo", "# owner/repo");

        // A repository source, deliberately handed the conversation prompt.
        let mismatched = AnalysisRequest::new(
            &source,
            AnalysisType::Context,
            PromptId::ConversationContext,
        );

        let failure = AnalysisService::offline()
            .execute(&mismatched, &source, &content)
            .await
            .expect_err("the conversation prompt does not apply to a repository");
        assert!(
            matches!(failure, AnalysisFailure::PromptNotApplicable(_)),
            "applicability must be checked before anything else, got {failure:?}"
        );
    }

    #[tokio::test]
    async fn a_failed_prose_analysis_produces_no_payload() {
        let file = repo_file(CaptureCoverage::FullDocument);
        let source = SourceDescriptor::from_vault_file(&file);
        let request = AnalysisRequest::new(&source, AnalysisType::Summary, PromptId::Summary);
        let content = CanonicalContent::from_markdown("owner/repo", "# owner/repo");

        let result = AnalysisService::offline()
            .run_prose(&request, &source, &content)
            .await;

        assert_eq!(result.metadata.status, AnalysisStatus::Failed);
        assert!(
            result.payload.is_none(),
            "there is no honest substitute for a summary nothing produced"
        );
        assert!(!result.is_usable());
    }

    #[tokio::test]
    async fn a_structured_fallback_is_recorded_as_insufficient_evidence() {
        let file = repo_file(CaptureCoverage::FullDocument);
        let source = SourceDescriptor::from_vault_file(&file);
        let request = context_request(&source).unwrap();
        let content = CanonicalContent::from_markdown("owner/repo", "# owner/repo\n\nA tool.");

        let result = AnalysisService::offline()
            .run_structured(
                &request,
                &source,
                &content,
                |_| Some("from model".to_string()),
                || "from fallback".to_string(),
            )
            .await;

        assert_eq!(result.metadata.status, AnalysisStatus::InsufficientEvidence);
        assert!(result.metadata.deterministic);
        assert_eq!(result.payload.as_deref(), Some("from fallback"));
        assert!(result.metadata.model.is_none());
        assert_eq!(result.metadata.prompt_id, "repository.context");
        assert_eq!(result.metadata.prompt_version, 1);
    }

    /// §29 — a partial capture's coverage reaches the prompt, so the model is
    /// told what it is not looking at.
    #[test]
    fn a_partial_capture_produces_a_coverage_caveat_naming_capture_notes() {
        let file = repo_file(CaptureCoverage::Partial);
        let source = SourceDescriptor::from_vault_file(&file);
        let caveat = CanonicalContent::coverage_caveat(&source).expect("partial needs a caveat");

        assert!(caveat.contains("incomplete"));
        assert!(caveat.contains("Only the README was reached."));

        let complete = repo_file(CaptureCoverage::FullDocument);
        let complete_source = SourceDescriptor::from_vault_file(&complete);
        assert!(CanonicalContent::coverage_caveat(&complete_source).is_none());
    }

    #[test]
    fn a_source_with_no_context_schema_says_so_instead_of_guessing() {
        let mut file = repo_file(CaptureCoverage::FullDocument);
        if let Some(p) = file.capture.as_mut() {
            p.capture_type = "page".to_string();
        }
        let source = SourceDescriptor::from_vault_file(&file);
        assert!(
            context_request(&source).is_none(),
            "a plain web page has no context prompt, and defaulting to one would invent a schema"
        );
    }
}
