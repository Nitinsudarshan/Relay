//! Unified Retrieval Service implementation.
//!
//! Evaluates queries across all Relay knowledge sources without per-feature silos.
//! Employs multi-signal deterministic scoring, provenance preservation, and explainability.

use super::model::*;
use super::providers::{DerivedDataProvider, MeetingProvider, MemoryProvider, VaultProvider};
use crate::meetings_v2::processing::MeetingProcessor;
use crate::meetings_v2::session_store::SessionStore;
use crate::memory::MemoryStore;
use crate::vault::VaultManager;

/// Common English stop words filtered out during boost calculations.
pub const STOP_WORDS: &[&str] = &[
    "a", "about", "above", "after", "again", "against", "all", "am", "an", "and", "any", "are",
    "as", "at", "be", "because", "been", "before", "being", "below", "between", "both", "but",
    "by", "did", "do", "does", "doing", "down", "during", "each", "few", "for", "from", "further",
    "had", "has", "have", "having", "he", "her", "here", "hers", "herself", "him", "himself", "his",
    "how", "i", "if", "in", "into", "is", "it", "its", "itself", "just", "me", "more", "most",
    "my", "myself", "no", "nor", "not", "now", "of", "off", "on", "once", "only", "or", "other",
    "our", "ours", "ourselves", "out", "over", "own", "s", "same", "she", "should", "so", "some",
    "such", "t", "than", "that", "the", "their", "theirs", "them", "themselves", "then", "there",
    "these", "they", "this", "those", "through", "to", "too", "under", "until", "up", "very", "was",
    "we", "were", "what", "when", "where", "which", "while", "who", "whom", "why", "will", "with",
];

/// Tokenize search text into lowercased terms, preserving repository identifiers.
pub fn tokenize(query: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let lower = query.trim().to_lowercase();
    if lower.is_empty() {
        return terms;
    }

    // Check for repo/URL patterns like owner/repo
    for token in lower.split_whitespace() {
        let clean = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '_' && c != '-');
        if clean.contains('/') && !clean.contains("://") {
            terms.push(clean.to_string());
            for sub in clean.split('/') {
                let sub_clean = sub.trim_matches(|c: char| !c.is_alphanumeric());
                if !sub_clean.is_empty() && !terms.contains(&sub_clean.to_string()) {
                    terms.push(sub_clean.to_string());
                }
            }
        }
    }

    // Standard word tokenizer
    let words: Vec<String> = lower
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    for w in words {
        if !terms.contains(&w) {
            terms.push(w);
        }
    }

    terms
}

/// Helper to generate a contextual snippet around matching query terms.
pub fn extract_snippet(content: &str, query_terms: &[String], max_chars: usize) -> String {
    if content.trim().is_empty() {
        return String::new();
    }
    if query_terms.is_empty() {
        return content.chars().take(max_chars).collect();
    }

    let lower = content.to_lowercase();
    let mut best_pos = None;

    // Filter out stop words for snippet centering if non-stop-words exist
    let content_terms: Vec<&String> = query_terms
        .iter()
        .filter(|t| !STOP_WORDS.contains(&t.as_str()))
        .collect();
    let search_terms = if !content_terms.is_empty() {
        content_terms
    } else {
        query_terms.iter().collect()
    };

    for term in search_terms {
        if let Some(pos) = lower.find(term.as_str()) {
            best_pos = Some(pos);
            break;
        }
    }

    let start_byte = match best_pos {
        Some(pos) => pos.saturating_sub(max_chars / 3),
        None => 0,
    };

    // Find nearest char boundary
    let mut safe_start = start_byte;
    while !content.is_char_boundary(safe_start) && safe_start > 0 {
        safe_start -= 1;
    }

    let slice = &content[safe_start..];
    let snippet: String = slice.chars().take(max_chars).collect();
    if safe_start > 0 {
        format!("...{}", snippet.trim_start())
    } else {
        snippet
    }
}

/// Unified retrieval engine.
pub struct UnifiedRetrievalService;

impl UnifiedRetrievalService {
    /// Executes a search across all provided Relay stores with candidate normalization,
    /// multi-signal scoring, and explainability.
    pub fn search(
        vault: &VaultManager,
        session_store: Option<&SessionStore>,
        meeting_processor: Option<&MeetingProcessor>,
        query: &RetrievalQuery,
    ) -> RetrievalResult {
        Self::search_with_memory(vault, None, session_store, meeting_processor, query)
    }

    /// Extended search allowing optional memory store injection.
    pub fn search_with_memory(
        vault: &VaultManager,
        memory_store: Option<&MemoryStore>,
        session_store: Option<&SessionStore>,
        meeting_processor: Option<&MeetingProcessor>,
        query: &RetrievalQuery,
    ) -> RetrievalResult {
        let terms = tokenize(&query.text);
        let mut candidates = Vec::new();

        // 1. Gather Vault candidates (Scribbles, Voice Notes, Files, Captures)
        let vault_provider = VaultProvider::new(vault);
        candidates.extend(vault_provider.gather_all(query));

        // 2. Gather Derived Data candidates (RepositoryContext, Summaries)
        let derived_provider = DerivedDataProvider::new(vault);
        candidates.extend(derived_provider.gather(query));

        // 3. Gather Memories if available
        if let Some(mem_store) = memory_store {
            let memory_provider = MemoryProvider::new(mem_store);
            candidates.extend(memory_provider.gather(query));
        }

        // 4. Gather Meetings if available
        let meeting_provider = MeetingProvider::new(session_store, meeting_processor);
        candidates.extend(meeting_provider.gather(query));

        // Score candidates
        let mut scored_items: Vec<RetrievedItem> = Vec::new();
        for candidate in candidates {
            if let Some(item) = Self::score_candidate(&candidate, &terms, &query.text, &query.filter) {
                scored_items.push(item);
            }
        }

        // Sort candidates deterministically:
        // 1. Score descending
        // 2. Source type weight descending
        // 3. Timestamp descending
        // 4. Stable ID ascending
        scored_items.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    b.source_type
                        .default_weight()
                        .partial_cmp(&a.source_type.default_weight())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| b.timestamp.cmp(&a.timestamp))
                .then_with(|| a.id.cmp(&b.id))
        });

        // Deduplication: prevent duplicate items with same source origin or content snippet
        let mut deduplicated: Vec<RetrievedItem> = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        for item in scored_items {
            if seen_ids.contains(&item.id) {
                continue;
            }
            seen_ids.insert(item.id.clone());
            deduplicated.push(item);
        }

        let total_matches = deduplicated.len();

        // Apply budget & limit
        let mut result_items = Vec::new();
        let mut budget_used = 0;
        let limit = query.limit.unwrap_or(usize::MAX);
        let max_budget = query.char_budget.unwrap_or(usize::MAX);

        for item in deduplicated {
            if result_items.len() >= limit {
                break;
            }
            let item_len = item.content.len();
            if budget_used + item_len > max_budget && !result_items.is_empty() {
                continue;
            }
            budget_used += item_len;
            result_items.push(item);
        }

        RetrievalResult {
            query: query.text.clone(),
            items: result_items,
            total_matches,
            budget_used,
        }
    }

    /// Evaluates a normalized candidate against query signals, filters, and generates explainability.
    pub fn score_candidate(
        candidate: &CandidateItem,
        terms: &[String],
        raw_query: &str,
        filter: &RetrievalFilter,
    ) -> Option<RetrievedItem> {
        // Time filter
        if let Some(tf) = &filter.time_filter {
            if let Some(ref ts) = candidate.timestamp {
                if let Some(after) = &tf.created_after {
                    if ts < after {
                        return None;
                    }
                }
                if let Some(before) = &tf.created_before {
                    if ts > before {
                        return None;
                    }
                }
            }
        }

        // Tag / topic filter
        if !filter.tags.is_empty() {
            let has_tag = filter.tags.iter().any(|t| {
                let lower_t = t.to_lowercase();
                candidate.topics.iter().any(|topic| topic.to_lowercase() == lower_t)
            });
            if !has_tag {
                return None;
            }
        }

        // Entity key filter
        if !filter.entity_keys.is_empty() {
            let has_entity = filter.entity_keys.iter().any(|e| {
                let lower_e = e.to_lowercase();
                candidate.entity_refs.iter().any(|ent| ent.to_lowercase() == lower_e)
                    || candidate.title.to_lowercase().contains(&lower_e)
            });
            if !has_entity {
                return None;
            }
        }

        let mut provenance = candidate.provenance.clone();

        // Empty query: match all with base weight + recency
        if terms.is_empty() {
            let snippet = extract_snippet(&candidate.content, terms, 240);
            provenance.evidence = Some(snippet.clone());
            let base = candidate.source_type.default_weight();
            return Some(RetrievedItem {
                id: candidate.id.clone(),
                source_type: candidate.source_type,
                title: candidate.title.clone(),
                content: candidate.content.clone(),
                snippet,
                score: base,
                timestamp: candidate.timestamp.clone(),
                provenance,
                topics: candidate.topics.clone(),
                entity_refs: candidate.entity_refs.clone(),
                explainability: Explainability {
                    matched_terms: Vec::new(),
                    match_types: vec![MatchType::RecencyOnly],
                    why: vec!["empty query matched all items passing filter".to_string()],
                    base_score: base,
                    boosts_applied: Vec::new(),
                    final_score: base,
                },
                metadata: candidate.metadata.clone(),
            });
        }

        let title_lower = candidate.title.to_lowercase();
        let content_lower = candidate.content.to_lowercase();
        let raw_query_lower = raw_query.trim().to_lowercase();

        let mut matched_terms = Vec::new();
        let mut match_types = Vec::new();
        let mut why = Vec::new();
        let mut boosts_applied = Vec::new();

        let mut title_boost = 0.0;
        let mut topic_boost = 0.0;
        let mut exact_phrase_boost = 0.0;
        let mut derived_priority_boost = 0.0;

        // 1. Exact phrase match
        if !raw_query_lower.is_empty()
            && (title_lower.contains(&raw_query_lower) || content_lower.contains(&raw_query_lower))
        {
            exact_phrase_boost += 6.0;
            match_types.push(MatchType::ExactPhrase);
            why.push("exact phrase match".to_string());
            boosts_applied.push("exact_phrase (+6.0)".to_string());
        }

        // 2. Term coverage
        let mut term_matches = 0;
        for term in terms {
            let in_title = title_lower.contains(term);
            let in_content = content_lower.contains(term);
            let in_topics = candidate.topics.iter().any(|top| top.to_lowercase().contains(term));
            let in_entities = candidate.entity_refs.iter().any(|ent| ent.to_lowercase().contains(term));

            if in_title || in_content || in_topics || in_entities {
                term_matches += 1;
                matched_terms.push(term.clone());

                if in_title && !boosts_applied.iter().any(|b| b.starts_with("title")) {
                    title_boost += 3.5;
                    match_types.push(MatchType::TitleMatch);
                    why.push("title match".to_string());
                    boosts_applied.push("title_match (+3.5)".to_string());
                }

                if in_topics && !boosts_applied.iter().any(|b| b.starts_with("topic")) {
                    topic_boost += 2.0;
                    match_types.push(MatchType::TopicMatch);
                    why.push("topic match".to_string());
                    boosts_applied.push("topic_match (+2.0)".to_string());
                }

                if in_entities && !boosts_applied.iter().any(|b| b.starts_with("entity")) {
                    match_types.push(MatchType::EntityMatch);
                    why.push("entity reference match".to_string());
                    boosts_applied.push("entity_match (+2.5)".to_string());
                }
            }
        }

        if term_matches == 0 && exact_phrase_boost == 0.0 {
            return None;
        }

        // 3. Derived Data Priority:
        // When querying for overview or what a project/repo is (e.g. "What is Orca?"),
        // a structured RepositoryContext should be prioritized above raw captures.
        if candidate.source_type == RetrievalSourceType::DerivedArtifact {
            derived_priority_boost += 3.0;
            match_types.push(MatchType::DerivedAbstraction);
            why.push("derived context abstraction priority".to_string());
            boosts_applied.push("derived_abstraction (+3.0)".to_string());
        }

        let coverage = term_matches as f32 / terms.len().max(1) as f32;
        if coverage > 0.0 {
            match_types.push(MatchType::TermCoverage);
        }

        let coverage_score = coverage * 5.0;
        let base_score = coverage_score + title_boost + topic_boost + exact_phrase_boost + derived_priority_boost;
        let source_weight = candidate.source_type.default_weight();
        let final_score = base_score * source_weight;

        let snippet = extract_snippet(&candidate.content, terms, 240);
        provenance.evidence = Some(snippet.clone());

        Some(RetrievedItem {
            id: candidate.id.clone(),
            source_type: candidate.source_type,
            title: candidate.title.clone(),
            content: candidate.content.clone(),
            snippet,
            score: final_score,
            timestamp: candidate.timestamp.clone(),
            provenance,
            topics: candidate.topics.clone(),
            entity_refs: candidate.entity_refs.clone(),
            explainability: Explainability {
                matched_terms,
                match_types,
                why,
                base_score,
                boosts_applied,
                final_score,
            },
            metadata: candidate.metadata.clone(),
        })
    }

    /// Backwards-compatible scoring helper for tests and external callers.
    #[allow(clippy::too_many_arguments)]
    pub fn score_item(
        id: &str,
        source_type: RetrievalSourceType,
        title: &str,
        content: &str,
        timestamp: Option<&str>,
        topics: Vec<String>,
        provenance: RetrievalProvenance,
        terms: &[String],
        filter: &RetrievalFilter,
    ) -> Option<RetrievedItem> {
        let candidate = CandidateItem {
            id: id.to_string(),
            source_type,
            title: title.to_string(),
            content: content.to_string(),
            timestamp: timestamp.map(|t| t.to_string()),
            topics,
            entity_refs: Vec::new(),
            provenance,
            metadata: serde_json::Value::Null,
        };
        let raw_query = terms.join(" ");
        Self::score_candidate(&candidate, terms, &raw_query, filter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize() {
        let terms = tokenize("Orca stablyai/orca project");
        assert!(terms.contains(&"orca".to_string()));
        assert!(terms.contains(&"stablyai".to_string()));
        assert!(terms.contains(&"stablyai/orca".to_string()));
        assert!(terms.contains(&"project".to_string()));
    }

    #[test]
    fn test_score_item_relevance_and_provenance() {
        let prov = RetrievalProvenance::new("note_1", RetrievalSourceType::Scribble);
        let terms = tokenize("Relay architecture");
        let filter = RetrievalFilter::default();

        let item = UnifiedRetrievalService::score_item(
            "note_1",
            RetrievalSourceType::Scribble,
            "Relay Knowledge Architecture",
            "This document outlines the unified retrieval architecture of Relay.",
            Some("2026-09-04T12:00:00Z"),
            vec!["Architecture".to_string()],
            prov,
            &terms,
            &filter,
        );

        assert!(item.is_some());
        let item = item.unwrap();
        assert!(item.score > 5.0);
        assert_eq!(item.source_type, RetrievalSourceType::Scribble);
        assert!(item.snippet.contains("Relay"));
        assert_eq!(item.provenance.source_id, "note_1");
        assert!(!item.explainability.why.is_empty());
    }

    #[test]
    fn test_exact_phrase_matching_boost() {
        let prov = RetrievalProvenance::new("note_2", RetrievalSourceType::Scribble);
        let terms = tokenize("parallel coding-agent workflows");
        let filter = RetrievalFilter::default();

        let item = UnifiedRetrievalService::score_item(
            "note_2",
            RetrievalSourceType::Scribble,
            "Orca Workflows",
            "Relay enables parallel coding-agent workflows across multiple models.",
            Some("2026-09-04T12:00:00Z"),
            vec![],
            prov,
            &terms,
            &filter,
        );

        assert!(item.is_some());
        let item = item.unwrap();
        assert!(item.explainability.match_types.contains(&MatchType::ExactPhrase));
        assert!(item.explainability.why.iter().any(|w| w.contains("exact phrase")));
    }

    #[test]
    fn test_filter_by_source_type() {
        let prov = RetrievalProvenance::new("note_1", RetrievalSourceType::VoiceNote);
        let terms = tokenize("anything");
        let filter = RetrievalFilter {
            source_types: vec![RetrievalSourceType::Scribble],
            ..Default::default()
        };

        let item = UnifiedRetrievalService::score_item(
            "note_1",
            RetrievalSourceType::VoiceNote,
            "Title",
            "Content with anything",
            None,
            vec![],
            prov,
            &terms,
            &filter,
        );
        assert!(item.is_some());
    }

    #[test]
    fn test_time_filter() {
        let prov = RetrievalProvenance::new("note_1", RetrievalSourceType::Scribble);
        let terms = tokenize("test");
        let filter = RetrievalFilter {
            time_filter: Some(TimeFilter {
                created_after: Some("2026-09-01T00:00:00Z".to_string()),
                created_before: Some("2026-09-03T00:00:00Z".to_string()),
            }),
            ..Default::default()
        };

        let too_late = UnifiedRetrievalService::score_item(
            "note_1",
            RetrievalSourceType::Scribble,
            "test title",
            "test content",
            Some("2026-09-04T00:00:00Z"),
            vec![],
            prov.clone(),
            &terms,
            &filter,
        );
        assert!(too_late.is_none());

        let in_window = UnifiedRetrievalService::score_item(
            "note_1",
            RetrievalSourceType::Scribble,
            "test title",
            "test content",
            Some("2026-09-02T00:00:00Z"),
            vec![],
            prov,
            &terms,
            &filter,
        );
        assert!(in_window.is_some());
    }
}
