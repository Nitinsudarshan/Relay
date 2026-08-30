//! Context assembly — turning retrieved items and conversation history
//! into the one prompt the model sees.
//!
//! Kept separate from the engine, and pure, because this is where the
//! product's central promise is either enforced or quietly lost: a
//! personal-memory question must be answerable *only* from the user's own
//! data. That rule lives in a string, and a string with no test around it
//! is a rule that erodes.

use super::intent::Intent;
use super::retrieval::{ContextItem, RetrievalResult};
use super::session::{Role, TalkbackSession};

/// What Talkback says when a memory question has no evidence behind it.
///
/// Returned *without* calling the model. The cheapest way never to
/// hallucinate a memory is not to ask a model to avoid hallucinating one.
pub const NO_EVIDENCE_RESPONSE: &str = "I couldn't find that in your Relay data.";

/// Roughly how many characters one token is worth for budgeting. English
/// prose sits near 4; 3.6 leaves headroom for Devanagari and for
/// tokenizers less generous than the estimate.
const CHARS_PER_TOKEN: f32 = 3.6;

/// Share of the model's window that retrieved context may occupy. The
/// rest is the system prompt, the conversation, and the answer itself.
const CONTEXT_WINDOW_SHARE: f32 = 0.35;

/// Converts a provider's configured context window into a character
/// budget for retrieval.
///
/// Derived rather than constant so raising `context_tokens` in Provider
/// Settings actually buys more grounding, which is the whole reason that
/// setting exists (`providers::ProviderConfig::context_tokens`).
pub fn char_budget_for(context_tokens: u32) -> usize {
    let budget = (context_tokens as f32 * CONTEXT_WINDOW_SHARE * CHARS_PER_TOKEN) as usize;
    budget.clamp(1_500, 40_000)
}

/// The shared voice rules. Every prompt below starts from these, so
/// Talkback sounds like one thing whether it is recalling, answering, or
/// confirming an action.
const VOICE_RULES: &str = "\
You are Relay's Talkback — the user's own thinking partner, speaking aloud.

How you speak:
- Two or three sentences by default. This is spoken aloud, not read.
- Plain language. No markdown, no bullet points, no headings, no emoji.
- Answer first, then add the one detail that matters. Never preamble.
- Say \"you\" and \"your\", not \"the user\".
- If asked to expand, then go longer.";

/// The prompt for a turn that must be grounded in the user's own data.
fn grounded_rules() -> String {
    format!(
        "{VOICE_RULES}

This is a question about the user's own history, so it has exactly one
honest source: the CONTEXT below, taken from their Relay data.

- Answer only from the CONTEXT. Nothing else you know counts here.
- If the CONTEXT does not contain the answer, say so plainly and stop. Do
  not reason towards a likely answer, and do not offer a general one.
- Never invent a name, a date, a number, or a decision.
- Refer to what they said naturally — \"in your pricing scribble\", \"in
  Tuesday's meeting\" — rather than citing markers."
    )
}

/// The prompt for a general question. Context still helps; it just is not
/// the only permitted source.
fn general_rules() -> String {
    format!(
        "{VOICE_RULES}

The CONTEXT below is from the user's own Relay data. Where it is relevant,
prefer it and say where it came from. Where it is not, answer normally and
do not pretend it was grounded in their notes."
    )
}

/// Renders retrieved items as a context block.
///
/// Each item is labelled with its source type, title and date. That
/// labelling is what lets the model say "in Tuesday's pricing review"
/// instead of "in the context", and it is the reason provenance survives
/// as far as the spoken answer.
pub fn render_context(result: &RetrievalResult) -> String {
    if result.items.is_empty() {
        return String::new();
    }
    let mut out = String::from("CONTEXT — from the user's own Relay data:\n");
    for (index, item) in result.items.iter().enumerate() {
        out.push_str(&format!(
            "\n[{}] {} — {}{}{}\n{}\n",
            index + 1,
            item.source_type.label(),
            item.title,
            item.detail
                .as_ref()
                .map(|d| format!(" ({d})"))
                .unwrap_or_default(),
            short_date(&item.timestamp)
                .map(|d| format!(", {d}"))
                .unwrap_or_default(),
            item.excerpt.trim()
        ));
    }
    out
}

/// `2026-08-30T12:00:00Z` → `2026-08-30`. Returns `None` rather than
/// inventing a date for an item that has none.
fn short_date(timestamp: &str) -> Option<&str> {
    if timestamp.len() >= 10 && timestamp.as_bytes()[4] == b'-' {
        Some(&timestamp[..10])
    } else {
        None
    }
}

/// Renders recent conversation so pronouns resolve.
pub fn render_history(session: &TalkbackSession, turns: usize) -> String {
    let recent = session.recent(turns);
    if recent.is_empty() {
        return String::new();
    }
    let mut out = String::from("CONVERSATION SO FAR:\n");
    for turn in recent {
        let speaker = match turn.role {
            Role::User => "User",
            Role::Agent => "Relay",
        };
        out.push_str(&format!("{}: {}\n", speaker, turn.text.trim()));
    }
    out
}

/// The complete system prompt for one turn.
pub fn build_system_prompt(
    intent: Intent,
    retrieval: &RetrievalResult,
    session: &TalkbackSession,
    history_turns: usize,
) -> String {
    let rules = if intent.requires_grounding() {
        grounded_rules()
    } else {
        general_rules()
    };

    let context = render_context(retrieval);
    let history = render_history(session, history_turns);

    let mut prompt = rules;
    if !history.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(&history);
    }
    if context.is_empty() {
        if intent.requires_grounding() {
            prompt.push_str(
                "\n\nCONTEXT: nothing in the user's Relay data matched this question. \
                 Say so plainly and do not answer from general knowledge.",
            );
        }
    } else {
        prompt.push_str("\n\n");
        prompt.push_str(&context);
    }
    prompt
}

/// Turns cited sources into the spoken answer to "where did you get
/// that?".
///
/// Produced deterministically rather than by the model: the one question
/// whose answer must be exactly true is the one about provenance.
pub fn describe_sources(sources: &[ContextItem]) -> String {
    if sources.is_empty() {
        return "That one wasn't from your Relay data — I answered it generally.".to_string();
    }
    let described: Vec<String> = sources
        .iter()
        .take(3)
        .map(|s| match short_date(&s.timestamp) {
            Some(date) => format!("your {} \"{}\" from {}", s.source_type.label(), s.title, date),
            None => format!("your {} \"{}\"", s.source_type.label(), s.title),
        })
        .collect();

    let list = match described.len() {
        1 => described[0].clone(),
        2 => format!("{} and {}", described[0], described[1]),
        _ => format!(
            "{}, and {}",
            described[..described.len() - 1].join(", "),
            described[described.len() - 1]
        ),
    };
    format!("That came from {list}.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::talkback::retrieval::SourceType;

    fn item(source_type: SourceType, id: &str, title: &str, excerpt: &str) -> ContextItem {
        ContextItem {
            source_type,
            source_id: id.to_string(),
            title: title.to_string(),
            timestamp: "2026-08-25T09:30:00Z".to_string(),
            relevance: 4.2,
            excerpt: excerpt.to_string(),
            detail: None,
            expanded: false,
        }
    }

    fn result(items: Vec<ContextItem>) -> RetrievalResult {
        RetrievalResult {
            items,
            searched_sources: SourceType::ALL.to_vec(),
            total_candidates: 12,
        }
    }

    #[test]
    fn budget_scales_with_the_configured_window() {
        let small = char_budget_for(8_192);
        let large = char_budget_for(32_768);
        assert!(large > small, "{large} should exceed {small}");
        assert!(small >= 1_500);
    }

    #[test]
    fn budget_is_clamped_at_both_ends() {
        assert_eq!(char_budget_for(0), 1_500);
        assert_eq!(char_budget_for(1), 1_500);
        assert_eq!(char_budget_for(u32::MAX), 40_000);
    }

    #[test]
    fn a_memory_prompt_forbids_general_knowledge() {
        let prompt = build_system_prompt(
            Intent::PersonalMemory,
            &result(vec![item(
                SourceType::MeetingFacts,
                "m1",
                "Pricing review",
                "flat seat licence",
            )]),
            &TalkbackSession::new(),
            6,
        );
        assert!(prompt.contains("Answer only from the CONTEXT"));
        assert!(prompt.contains("Never invent a name, a date, a number"));
        assert!(prompt.contains("flat seat licence"));
    }

    #[test]
    fn a_memory_prompt_with_no_context_says_so_explicitly() {
        let prompt = build_system_prompt(
            Intent::PersonalMemory,
            &result(vec![]),
            &TalkbackSession::new(),
            6,
        );
        assert!(prompt.contains("nothing in the user's Relay data matched"));
        assert!(prompt.contains("do not answer from general knowledge"));
    }

    #[test]
    fn a_general_prompt_does_not_forbid_general_knowledge() {
        let prompt = build_system_prompt(
            Intent::General,
            &result(vec![]),
            &TalkbackSession::new(),
            6,
        );
        assert!(!prompt.contains("Answer only from the CONTEXT"));
        assert!(prompt.contains("answer normally"));
    }

    #[test]
    fn every_prompt_asks_for_a_short_spoken_answer() {
        for intent in [Intent::PersonalMemory, Intent::General] {
            let prompt =
                build_system_prompt(intent, &result(vec![]), &TalkbackSession::new(), 6);
            assert!(prompt.contains("Two or three sentences"), "{:?}", intent);
            assert!(prompt.contains("No markdown"), "{:?}", intent);
        }
    }

    #[test]
    fn context_carries_source_type_title_and_date() {
        let rendered = render_context(&result(vec![item(
            SourceType::Scribble,
            "s1",
            "Pricing model rethink",
            "usage based versus flat",
        )]));
        assert!(rendered.contains("[1] Scribble — Pricing model rethink, 2026-08-25"));
        assert!(rendered.contains("usage based versus flat"));
    }

    #[test]
    fn context_includes_the_detail_qualifier_when_present() {
        let mut decision = item(SourceType::MeetingFacts, "m1", "Pricing review", "ship flat");
        decision.detail = Some("decision".to_string());
        let rendered = render_context(&result(vec![decision]));
        assert!(rendered.contains("Pricing review (decision)"), "{rendered}");
    }

    #[test]
    fn context_is_empty_when_nothing_was_retrieved() {
        assert!(render_context(&result(vec![])).is_empty());
    }

    #[test]
    fn history_renders_both_speakers_in_order() {
        let mut session = TalkbackSession::new();
        session.push_user("what did we decide", Intent::PersonalMemory, true);
        session.push_agent("A flat seat licence.", vec![]);
        let rendered = render_history(&session, 6);
        let user_at = rendered.find("User: what did we decide").unwrap();
        let agent_at = rendered.find("Relay: A flat seat licence.").unwrap();
        assert!(user_at < agent_at);
    }

    #[test]
    fn history_is_omitted_for_a_fresh_session() {
        assert!(render_history(&TalkbackSession::new(), 6).is_empty());
    }

    #[test]
    fn source_description_is_singular_dual_and_listed() {
        let one = describe_sources(&[item(SourceType::Scribble, "s1", "Pricing", "x")]);
        assert_eq!(one, "That came from your Scribble \"Pricing\" from 2026-08-25.");

        let two = describe_sources(&[
            item(SourceType::Scribble, "s1", "Pricing", "x"),
            item(SourceType::VoiceNote, "n1", "Hiring", "y"),
        ]);
        assert!(two.contains(" and your Voice Note \"Hiring\""), "{two}");

        let three = describe_sources(&[
            item(SourceType::Scribble, "s1", "A", "x"),
            item(SourceType::VoiceNote, "n1", "B", "y"),
            item(SourceType::Meeting, "m1", "C", "z"),
        ]);
        assert!(three.contains(", and your Meeting \"C\""), "{three}");
    }

    #[test]
    fn source_description_is_honest_when_there_were_none() {
        let described = describe_sources(&[]);
        assert!(described.contains("wasn't from your Relay data"));
    }

    #[test]
    fn source_description_caps_at_three() {
        let many: Vec<ContextItem> = (0..8)
            .map(|i| item(SourceType::Scribble, &format!("s{i}"), &format!("T{i}"), "x"))
            .collect();
        let described = describe_sources(&many);
        assert!(!described.contains("T3"), "listed more than three: {described}");
    }

    #[test]
    fn a_missing_timestamp_is_omitted_rather_than_invented() {
        let mut undated = item(SourceType::Scribble, "s1", "Pricing", "x");
        undated.timestamp = String::new();
        assert_eq!(
            describe_sources(&[undated.clone()]),
            "That came from your Scribble \"Pricing\"."
        );
        assert!(!render_context(&result(vec![undated])).contains(", ,"));
    }
}
