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
Lead with the outcome, not the process. Keep the Overview to at most three sentences \
and collapse the Discussion to the few points that change what someone does next. \
Decisions, action items, risks, and open questions still appear in full — brevity \
comes out of the explanation, never out of the outcomes."
                .to_string(),
            builtin: true,
        },
        MeetingExtension {
            id: "project_update".to_string(),
            name: "Project Update".to_string(),
            instructions: "Write as a status update to a project channel. Organize the \
Discussion around what moved, what is blocked, and what is next. Name the projects \
and components involved wherever the facts identify them. Keep risks and blockers \
prominent rather than buried."
                .to_string(),
            builtin: true,
        },
        MeetingExtension {
            id: "decision_log".to_string(),
            name: "Decision Log".to_string(),
            instructions: "Write as a decision record. Put Decisions immediately after the \
Overview; for each decision state what was settled, the reason where the facts give \
one, and who settled it where the facts say. Keep the Overview to a single short \
paragraph of context. Do not restate discussion that did not produce a decision."
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

/// How much depth a mode asks for.
///
/// Depth, not size. The size budget is computed per meeting by
/// `processing::length` and stated to the model separately, because a mode that
/// carries its own absolute length is wrong on almost every meeting: it padded
/// short ones and truncated long ones. What is left here is the only thing a
/// mode should decide — how much of the *reasoning* survives.
///
/// Every mode is bound by the same floor: a decision, a commitment, an owner, a
/// deadline, or an unresolved question is never dropped to save room. Concise
/// means less explanation, never fewer outcomes.
pub fn mode_instructions(mode: SummaryMode) -> &'static str {
    match mode {
        SummaryMode::Concise => {
            "Written to be scanned. Keep every decision, commitment, owner, deadline, \
risk, and open question, and cut everything else hard: one line per point, no \
secondary discussion, no examples, and reasoning only where a decision would be \
unreadable without it. Merge closely related topics rather than dropping one."
        }
        SummaryMode::Standard => {
            "The default record. Cover the substantive discussion, the reasoning behind \
each decision, and the context needed to act on it. Leave out repetition, \
tangents, and minor examples. One or two sentences per point."
        }
        SummaryMode::Detailed => {
            "The full record, organized by topic. Keep the reasoning, the alternatives \
that were considered and rejected, the trade-offs, and the disagreements. Retain \
useful secondary discussion. This is still a summary and not a transcript: never \
narrate the meeting turn by turn, and never approach the length of the source."
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
    fn no_mode_may_buy_brevity_by_dropping_an_outcome() {
        // The failure this pins: "make it shorter" turning into "drop the
        // action items", which is the one trade a summary may never make.
        let concise = mode_instructions(SummaryMode::Concise).to_lowercase();
        assert!(concise.contains("keep every decision"));
        for word in ["commitment", "owner", "deadline", "open question"] {
            assert!(
                concise.contains(word),
                "concise mode must protect {}",
                word
            );
        }
    }

    #[test]
    fn a_mode_decides_depth_and_never_an_absolute_length() {
        // Absolute sizes belong to `processing::length`, which derives them from
        // the meeting. A mode that names its own word count is wrong on every
        // meeting that is not the size it assumed.
        for mode in [
            SummaryMode::Concise,
            SummaryMode::Standard,
            SummaryMode::Detailed,
        ] {
            let text = mode_instructions(mode).to_lowercase();
            assert!(
                !text.contains("length:") && !text.contains(" words"),
                "{:?} states a size instead of a depth",
                mode
            );
        }
    }

    #[test]
    fn mode_parsing_defaults_to_standard() {
        assert_eq!(SummaryMode::parse("concise"), SummaryMode::Concise);
        assert_eq!(SummaryMode::parse("DETAILED"), SummaryMode::Detailed);
        assert_eq!(SummaryMode::parse("nonsense"), SummaryMode::Standard);
        assert_eq!(SummaryMode::default(), SummaryMode::Standard);
    }
}
