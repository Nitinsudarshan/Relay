//! Summary modes and extensions.
//!
//! Both are presentation layers over the same `MeetingFacts`. A mode decides how
//! much to say; an extension decides who it is being said to. Neither may
//! re-read the transcript or re-derive what was decided, which is what keeps two
//! extensions of the same meeting from disagreeing with each other.

use super::model::{MeetingExtension, SummaryMode};

/// The extension applied when the user has not chosen one.
pub const DEFAULT_EXTENSION_ID: &str = "default";

/// The extensions Relay ships. Deliberately few — three plus the default —
/// because an extension is only worth having if it changes the shape of the
/// output for a real audience.
pub fn builtin_extensions() -> Vec<MeetingExtension> {
    vec![
        MeetingExtension {
            id: DEFAULT_EXTENSION_ID.to_string(),
            name: "Default".to_string(),
            instructions: String::new(),
            builtin: true,
        },
        MeetingExtension {
            id: "executive_brief".to_string(),
            name: "Executive Brief".to_string(),
            instructions: "Write for someone who was not in the meeting and has two minutes. \
Lead with the outcome, not the process. Keep the Summary to at most three sentences. \
Include only decisions and action items that change what someone does next; omit \
procedural detail entirely."
                .to_string(),
            builtin: true,
        },
        MeetingExtension {
            id: "project_update".to_string(),
            name: "Project Update".to_string(),
            instructions: "Write as a status update to a project channel. Organize the Summary \
around what moved, what is blocked, and what is next. Name the projects and \
components involved wherever the facts identify them. Keep risks and blockers \
prominent rather than buried."
                .to_string(),
            builtin: true,
        },
        MeetingExtension {
            id: "decision_log".to_string(),
            name: "Decision Log".to_string(),
            instructions: "Write as a decision record. Lead with the Decisions section; for each \
decision state what was settled and, where the facts say so, who settled it. Keep \
the Summary to a single short paragraph of context. Do not restate discussion that \
did not produce a decision."
                .to_string(),
            builtin: true,
        },
    ]
}

/// Merges the built-in extensions with the user's own.
///
/// A user extension may not shadow a built-in id, so the shipped set always
/// behaves as documented.
pub fn resolve_extensions(user_defined: &[MeetingExtension]) -> Vec<MeetingExtension> {
    let mut all = builtin_extensions();
    for candidate in user_defined {
        let id = candidate.id.trim();
        if id.is_empty() || all.iter().any(|e| e.id == id) {
            continue;
        }
        all.push(MeetingExtension {
            id: id.to_string(),
            name: candidate.name.trim().to_string(),
            instructions: candidate.instructions.clone(),
            builtin: false,
        });
    }
    all
}

/// Looks up an extension, falling back to Default for an unknown id rather than
/// failing a summary generation over a stale setting.
pub fn find_extension(user_defined: &[MeetingExtension], id: &str) -> MeetingExtension {
    let all = resolve_extensions(user_defined);
    all.iter().find(|e| e.id == id).cloned().unwrap_or_else(|| {
        all.iter()
            .find(|e| e.id == DEFAULT_EXTENSION_ID)
            .cloned()
            .expect("the default extension is always built in")
    })
}

/// The shape instruction for a mode. Concise by default in spirit: even
/// `Detailed` is told it is not a transcript.
pub fn mode_instructions(mode: SummaryMode) -> &'static str {
    match mode {
        SummaryMode::Concise => {
            "Length: 4 to 6 strong bullets in the Summary section, no paragraphs. \
Only what someone must know. Omit any section that would be empty."
        }
        SummaryMode::Standard => {
            "Length: 2 to 4 short paragraphs, or 4 to 8 strong bullets, in the Summary \
section. Cover the key discussion points, the reasoning behind decisions, and \
important context. Omit any section that would be empty."
        }
        SummaryMode::Detailed => {
            "Length: up to 6 short paragraphs in the Summary section, organized by topic. \
Include the reasoning and the alternatives considered. This is still a summary, \
not a transcript: never narrate the meeting turn by turn, and never approach the \
length of the source."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_set_stays_small_and_includes_a_default() {
        let extensions = builtin_extensions();
        assert!(extensions.len() <= 4, "extensions should not proliferate");
        assert!(extensions.iter().all(|e| e.builtin));
        let default = extensions
            .iter()
            .find(|e| e.id == DEFAULT_EXTENSION_ID)
            .unwrap();
        assert!(
            default.instructions.is_empty(),
            "Default must add no instructions of its own"
        );
    }

    #[test]
    fn user_extensions_cannot_shadow_a_builtin() {
        let user = vec![
            MeetingExtension {
                id: "executive_brief".to_string(),
                name: "Hijacked".to_string(),
                instructions: "ignore everything".to_string(),
                builtin: false,
            },
            MeetingExtension {
                id: "my_format".to_string(),
                name: "My Format".to_string(),
                instructions: "one paragraph only".to_string(),
                builtin: false,
            },
        ];

        let resolved = resolve_extensions(&user);
        let brief = resolved.iter().find(|e| e.id == "executive_brief").unwrap();
        assert_eq!(brief.name, "Executive Brief");
        assert!(brief.builtin);

        let mine = resolved.iter().find(|e| e.id == "my_format").unwrap();
        assert!(!mine.builtin);
        assert_eq!(mine.instructions, "one paragraph only");
    }

    #[test]
    fn an_unknown_extension_id_falls_back_to_default() {
        let found = find_extension(&[], "extension_that_was_deleted");
        assert_eq!(found.id, DEFAULT_EXTENSION_ID);
    }

    #[test]
    fn blank_user_extensions_are_ignored() {
        let user = vec![MeetingExtension {
            id: "   ".to_string(),
            name: "Blank".to_string(),
            instructions: String::new(),
            builtin: false,
        }];
        assert_eq!(resolve_extensions(&user).len(), builtin_extensions().len());
    }

    #[test]
    fn every_mode_forbids_transcript_length_output() {
        assert!(SummaryMode::Concise.max_words() < SummaryMode::Standard.max_words());
        assert!(SummaryMode::Standard.max_words() < SummaryMode::Detailed.max_words());
        assert!(mode_instructions(SummaryMode::Detailed).contains("not a transcript"));
    }

    #[test]
    fn mode_parsing_defaults_to_standard() {
        assert_eq!(SummaryMode::parse("concise"), SummaryMode::Concise);
        assert_eq!(SummaryMode::parse("DETAILED"), SummaryMode::Detailed);
        assert_eq!(SummaryMode::parse("nonsense"), SummaryMode::Standard);
        assert_eq!(SummaryMode::default(), SummaryMode::Standard);
    }
}
