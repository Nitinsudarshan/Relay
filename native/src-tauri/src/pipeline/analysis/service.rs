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

use crate::providers::{LLMClient, ProviderError, ProviderType};

use super::content::CanonicalContent;
use super::contract::{
    AnalysisFailure, AnalysisRequest, AnalysisResult, AnalysisType, MetadataBuilder,
};
use super::prompts::PromptId;
use super::source::SourceDescriptor;
use crate::pipeline::source_boundary;

/// Executes analyses against a provider.
///
/// Holds a borrowed client rather than building its own: §39 says there is one
/// authoritative provider abstraction, and a service that constructs its own
/// `LLMClient` is the first step towards a second one.
pub struct AnalysisService<'a> {
    llm: Option<&'a LLMClient>,
}

impl<'a> AnalysisService<'a> {
    /// A service backed by a provider.
    pub fn new(llm: &'a LLMClient) -> Self {
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

        let client = self.llm.ok_or_else(|| {
            AnalysisFailure::NoCompletion("no provider is configured".to_string())
        })?;

        // The boundary is applied from the source's trust level, not from the
        // caller's memory. A captured page is external whatever its domain.
        let body = content.as_prompt_body();
        let (user_prompt, mut system_prompt) = if source.trust.is_external() {
            (
                source_boundary::wrap_external_source(&source.describe_origin(), &body).framed,
                format!(
                    "{}\n{}",
                    prompt.system_instructions,
                    source_boundary::EXTERNAL_SOURCE_RULE
                ),
            )
        } else {
            (body, prompt.system_instructions.to_string())
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

fn provider_name(provider: &ProviderType) -> String {
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

    let value: serde_json::Value = serde_json::from_str(candidate).ok()?;
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
