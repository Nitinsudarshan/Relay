//! Prompts as identified, versioned, testable objects.
//!
//! # What was wrong with constants
//!
//! Relay's prompts lived in four places — two canonical constants in
//! `pipeline::enrichment`, two in `capture::web::context`, one inline literal
//! in `pipeline`, and two builders in `meetings_v2`. Each was fine on its own.
//! Together they had two problems that only show up later:
//!
//! Nothing recorded *which* prompt produced a stored result, so a prompt could
//! be rewritten and every artifact it ever produced would silently claim to be
//! the output of the new one. And nothing stopped a prompt from being used on
//! a source it was never written for — the repository prompt asks for a tech
//! stack, and a conversation given that prompt will invent one.
//!
//! A [`PromptDefinition`] fixes both: an id that outlives the text, a version
//! that changes when the text does, and an explicit list of source types the
//! prompt applies to.
//!
//! # Versioning
//!
//! `version` is bumped by hand when the instructions change in a way that
//! changes the output. It is recorded on every result. Old derived data keeps
//! the version it was produced under, so "regenerate everything produced by
//! `repository.context` v1" is a question that can be answered.
//!
//! This is a registry, not a prompt editor. There is no UI, no storage and no
//! user-authored prompt — those are separate product concerns and §19 says so.

use super::source::SourceType;
use crate::providers::CompletionOptions;

/// The stable name of a prompt.
///
/// An enum rather than a string so that a typo is a compile error and the set
/// is enumerable — the applicability tests iterate over [`PromptId::ALL`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptId {
    /// Canonical short summary. Applies to any source.
    Summary,
    /// Canonical knowledge enrichment: title, topics, entities, questions.
    Enrichment,
    /// Structured work context for a captured AI conversation.
    ConversationContext,
    /// Structured repository context: objective, stack, features, users, issues.
    RepositoryContext,
    /// Stage A of a meeting: the transcript reduced to structured facts.
    MeetingFacts,
    /// Stage B of a meeting: prose written from those facts, transcript closed.
    MeetingSummary,
}

impl PromptId {
    /// Every registered prompt. Tests iterate this, so a new prompt is covered
    /// by the registry's invariants the moment it is added.
    pub const ALL: &'static [PromptId] = &[
        PromptId::Summary,
        PromptId::Enrichment,
        PromptId::ConversationContext,
        PromptId::RepositoryContext,
        PromptId::MeetingFacts,
        PromptId::MeetingSummary,
    ];

    /// The wire name written into derived data. Stable — changing one is a
    /// data migration, which is the point of having it separate from the enum.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Enrichment => "enrichment",
            Self::ConversationContext => "conversation.context",
            Self::RepositoryContext => "repository.context",
            Self::MeetingFacts => "meeting.facts",
            Self::MeetingSummary => "meeting.summary",
        }
    }

    pub fn definition(&self) -> PromptDefinition {
        definition(*self)
    }
}

/// Whether a prompt expects prose back or a JSON document.
///
/// Decides whether the service validates the response as JSON before anything
/// downstream sees it, which is the difference between §24's parse → validate
/// chain and treating arbitrary prose as a successful structured analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputContract {
    Prose,
    Json,
}

/// Where a prompt's instruction text comes from.
///
/// The registry's doc below already says the bodies live in the modules that
/// own the semantics, because those modules also own the parsers that must
/// agree with them. This is that principle carried one step further: some
/// bodies cannot be constants at all.
///
/// A meeting summary's instructions interpolate the length budget, the depth
/// mode, the chosen extension, the user's standing instructions and their
/// language — none of which are known until the call. `meetings_v2::processing`
/// deliberately did not migrate onto this registry for exactly that reason
/// (`analysis/mod.rs`, "Not migrated yet"). `Computed` is what removes the
/// blocker: the registry keeps identity, version, output contract and sampling,
/// and only the text is the caller's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptBody {
    /// Fixed instructions, owned by the module the registry names.
    Static(&'static str),
    /// Instructions built per call by the module that owns them. The caller
    /// supplies the text; everything else about the prompt still comes from
    /// here.
    Computed,
}

/// A prompt, everything about it that callers need, and nothing about how it
/// is sent.
pub struct PromptDefinition {
    pub id: PromptId,
    /// Bumped when the instructions change in a way that changes the output.
    pub version: u32,
    /// What this prompt is for, in one line. Not sent to the model.
    pub purpose: &'static str,
    pub body: PromptBody,
    pub output_contract: OutputContract,
    /// The source types this prompt is written for. Empty means "any source",
    /// used by the two canonical prompts that genuinely are source-agnostic.
    pub applies_to: &'static [SourceType],
    /// Sampling this prompt needs. Strict-JSON extraction wants near-zero
    /// temperature; prose wants a little room. Before this, both ran at
    /// whatever the provider defaulted to.
    pub temperature: f32,
    pub max_output_tokens: u32,
}

impl PromptDefinition {
    /// Whether this prompt may be used on a source of this type.
    pub fn applies_to_source(&self, source_type: SourceType) -> bool {
        self.applies_to.is_empty() || self.applies_to.contains(&source_type)
    }

    /// Completion options for this prompt, over the client's configured window.
    pub fn options(&self, base: CompletionOptions) -> CompletionOptions {
        CompletionOptions {
            temperature: self.temperature,
            max_output_tokens: self.max_output_tokens,
            ..base
        }
    }

    pub fn expects_json(&self) -> bool {
        self.output_contract == OutputContract::Json
    }

    /// The fixed instructions, when this prompt has any.
    ///
    /// `None` means the caller must supply them — see [`PromptBody::Computed`].
    /// Returning an `Option` rather than an empty string is deliberate: a
    /// computed prompt sent with no instructions is a bug, and an empty string
    /// would let it reach a model as one.
    pub fn static_instructions(&self) -> Option<&'static str> {
        match self.body {
            PromptBody::Static(text) => Some(text),
            PromptBody::Computed => None,
        }
    }

    pub fn is_computed(&self) -> bool {
        matches!(self.body, PromptBody::Computed)
    }
}

/// The registry.
///
/// The instruction bodies stay in the modules that own the semantics —
/// `pipeline::enrichment` for the canonical two, `capture::web::context` for
/// the two context prompts — because those modules also own the parsers that
/// must agree with them. What lives here is the identity, the version, and the
/// applicability rule.
pub fn definition(id: PromptId) -> PromptDefinition {
    match id {
        PromptId::Summary => PromptDefinition {
            id,
            version: 1,
            purpose: "Short, structured prose summary of any source.",
            body: PromptBody::Static(crate::pipeline::CANONICAL_SUMMARY_SYSTEM_PROMPT),
            output_contract: OutputContract::Prose,
            applies_to: &[],
            temperature: 0.3,
            max_output_tokens: 900,
        },
        PromptId::Enrichment => PromptDefinition {
            id,
            version: 1,
            purpose: "Title, summary, topics, entities and exploration questions.",
            body: PromptBody::Static(crate::pipeline::CANONICAL_ANALYSIS_SYSTEM_PROMPT),
            output_contract: OutputContract::Json,
            applies_to: &[],
            temperature: 0.2,
            max_output_tokens: 1_500,
        },
        PromptId::ConversationContext => PromptDefinition {
            id,
            version: 1,
            purpose: "Structured work context for a captured AI conversation.",
            body: PromptBody::Static(crate::capture::web::context::CONTEXT_EXTRACTION_SYSTEM_PROMPT),
            output_contract: OutputContract::Json,
            applies_to: &[SourceType::Conversation],
            // Extraction into strict JSON. The failure mode at a higher
            // temperature is a fabricated decision, not a livelier one.
            temperature: 0.1,
            max_output_tokens: 2_400,
        },
        PromptId::RepositoryContext => PromptDefinition {
            id,
            version: 1,
            purpose: "Objective, stack, features, user base and issues for a repository.",
            body: PromptBody::Static(crate::capture::web::context::REPOSITORY_CONTEXT_SYSTEM_PROMPT),
            output_contract: OutputContract::Json,
            applies_to: &[SourceType::Repository],
            temperature: 0.1,
            max_output_tokens: 2_400,
        },
        PromptId::MeetingFacts => PromptDefinition {
            id,
            version: 1,
            purpose: "Stage A of a meeting: the transcript reduced to structured facts.",
            // Built per call: the instructions carry the transcript window, the
            // speaker roster and the extraction schema, which depend on the
            // meeting. `meetings_v2::processing::extract` owns both the builder
            // and the validator that has to agree with it.
            body: PromptBody::Computed,
            output_contract: OutputContract::Json,
            applies_to: &[SourceType::Meeting],
            // Extraction invents an owner or a deadline the moment it is given
            // room to be creative.
            temperature: 0.1,
            max_output_tokens: 2_400,
        },
        PromptId::MeetingSummary => PromptDefinition {
            id,
            version: 1,
            purpose: "Stage B of a meeting: prose written from the facts, transcript closed.",
            // Built per call: length budget, depth mode, chosen extension, the
            // user's standing instructions, and their notes language and
            // script. None of it is known until the call.
            body: PromptBody::Computed,
            output_contract: OutputContract::Prose,
            applies_to: &[SourceType::Meeting],
            // Slightly above extraction because rewriting requires generation,
            // low enough to stay grounded. This entry was written at 0.4 before
            // anything ran on it; the pipeline it now serves has always sent
            // 0.3, and the migration follows the shipped value rather than
            // quietly raising the temperature of every meeting summary.
            temperature: 0.3,
            // A default the caller almost always overrides: the real allowance
            // is computed from the length budget the user chose.
            max_output_tokens: 1_600,
        },

    }
}

/// The prompt that produces structured context for a source type, if one does.
///
/// This is the lookup that replaces `if github { … } else { … }` at the call
/// site. A source type with no context prompt returns `None` rather than
/// falling through to whichever branch happens to be last.
pub fn context_prompt_for(source_type: SourceType) -> Option<PromptId> {
    match source_type {
        SourceType::Conversation => Some(PromptId::ConversationContext),
        SourceType::Repository => Some(PromptId::RepositoryContext),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_prompt_id_resolves_and_has_a_stable_name() {
        for id in PromptId::ALL {
            let def = id.definition();
            assert_eq!(def.id, *id);
            assert!(!id.as_str().is_empty());
            // A static prompt must carry instructions; a computed one must
            // carry none, so that a caller supplying them cannot be silently
            // ignored and a caller forgetting them cannot silently send an
            // empty system prompt.
            match def.body {
                PromptBody::Static(text) => assert!(
                    !text.trim().is_empty(),
                    "{} is static and has no instructions",
                    id.as_str()
                ),
                PromptBody::Computed => assert!(
                    def.static_instructions().is_none(),
                    "{} is computed and must not also carry a body",
                    id.as_str()
                ),
            }
            assert!(def.version >= 1, "{} must be versioned", id.as_str());
        }
    }

    /// The ids are written into stored derived data. Renaming one silently
    /// orphans every artifact produced under the old name.
    #[test]
    fn prompt_ids_are_the_names_stored_on_disk() {
        assert_eq!(PromptId::Summary.as_str(), "summary");
        assert_eq!(PromptId::Enrichment.as_str(), "enrichment");
        assert_eq!(PromptId::ConversationContext.as_str(), "conversation.context");
        assert_eq!(PromptId::RepositoryContext.as_str(), "repository.context");
        assert_eq!(PromptId::MeetingFacts.as_str(), "meeting.facts");
        assert_eq!(PromptId::MeetingSummary.as_str(), "meeting.summary");
    }

    /// §47: a source-specific prompt must not be usable on another source type.
    /// The repository prompt asks for a tech stack; a conversation handed that
    /// question answers it.
    #[test]
    fn source_specific_prompts_reject_the_wrong_source_type() {
        let repo = PromptId::RepositoryContext.definition();
        assert!(repo.applies_to_source(SourceType::Repository));
        assert!(!repo.applies_to_source(SourceType::Conversation));
        assert!(!repo.applies_to_source(SourceType::Document));

        let convo = PromptId::ConversationContext.definition();
        assert!(convo.applies_to_source(SourceType::Conversation));
        assert!(!convo.applies_to_source(SourceType::Repository));
    }

    #[test]
    fn canonical_prompts_apply_to_any_source() {
        for id in [PromptId::Summary, PromptId::Enrichment] {
            let def = id.definition();
            for source_type in [
                SourceType::Conversation,
                SourceType::Repository,
                SourceType::Document,
                SourceType::WebPage,
                SourceType::Meeting,
            ] {
                assert!(
                    def.applies_to_source(source_type),
                    "{} should be source-agnostic",
                    id.as_str()
                );
            }
        }
    }

    #[test]
    fn context_prompt_selection_replaces_the_source_conditional() {
        assert_eq!(
            context_prompt_for(SourceType::Repository),
            Some(PromptId::RepositoryContext)
        );
        assert_eq!(
            context_prompt_for(SourceType::Conversation),
            Some(PromptId::ConversationContext)
        );
        // No context prompt yet, and saying so beats defaulting to one of the
        // two that exist.
        assert_eq!(context_prompt_for(SourceType::Document), None);
        assert_eq!(context_prompt_for(SourceType::WebPage), None);
    }

    /// Every prompt that declares JSON output must actually ask for it, or the
    /// client's own fallback branches the wrong way and the service validates
    /// prose as though it were structured.
    #[test]
    fn json_prompts_ask_the_model_for_json() {
        for id in PromptId::ALL {
            let def = id.definition();
            // A computed prompt has no body here to check. The invariant still
            // holds — it is enforced where the body is built, by the module
            // that owns both the builder and the parser. See
            // `meetings_v2::processing::extract`.
            let Some(instructions) = def.static_instructions() else {
                continue;
            };
            if def.expects_json() {
                assert!(
                    instructions.contains("JSON"),
                    "{} declares JSON output but never asks for it",
                    id.as_str()
                );
            }
        }
    }

    #[test]
    fn extraction_prompts_run_cooler_than_prose_prompts() {
        let extraction = PromptId::RepositoryContext.definition();
        let prose = PromptId::Summary.definition();
        assert!(
            extraction.temperature < prose.temperature,
            "strict extraction must not run at a prose temperature"
        );
    }

    #[test]
    fn prompt_options_override_sampling_but_keep_the_configured_window() {
        let base = CompletionOptions {
            context_tokens: 16_384,
            ..CompletionOptions::default()
        };
        let options = PromptId::ConversationContext.definition().options(base);
        assert_eq!(options.context_tokens, 16_384, "the user's window is preserved");
        assert!((options.temperature - 0.1).abs() < f32::EPSILON);
    }
}
