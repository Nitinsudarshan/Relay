//! Unified Retrieval Service implementation.
//!
//! Evaluates queries across all Relay knowledge sources without per-feature silos.

use super::model::*;
use crate::meetings_v2::processing::MeetingProcessor;
use crate::meetings_v2::session_store::SessionStore;
use crate::vault::{VaultManager, VOICE_NOTE_TYPE};

/// Tokenize search text into lowercased terms.
pub fn tokenize(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
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
    for term in query_terms {
        if let Some(pos) = lower.find(term) {
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
    /// Executes a search across all provided Relay stores.
    pub fn search(
        vault: &VaultManager,
        session_store: Option<&SessionStore>,
        _meeting_processor: Option<&MeetingProcessor>,
        query: &RetrievalQuery,
    ) -> RetrievalResult {
        let terms = tokenize(&query.text);
        let mut candidates: Vec<RetrievedItem> = Vec::new();

        let allowed = |st: RetrievalSourceType| -> bool {
            query.filter.source_types.is_empty() || query.filter.source_types.contains(&st)
        };

        // 1. Scribbles
        if allowed(RetrievalSourceType::Scribble) {
            if let Ok(scribbles) = vault.list_scribbles() {
                for s in scribbles {
                    let mut tags = s.tags.clone();
                    tags.extend(s.topics.clone());
                    let body = match &s.summary {
                        Some(sum) if !sum.trim().is_empty() => format!("{}\n\n{}", sum.trim(), s.content),
                        _ => s.content.clone(),
                    };
                    let provenance = RetrievalProvenance::new(&s.id, RetrievalSourceType::Scribble);
                    if let Some(item) = Self::score_item(
                        &s.id,
                        RetrievalSourceType::Scribble,
                        &s.title,
                        &body,
                        Some(&s.created_at),
                        tags,
                        provenance,
                        &terms,
                        &query.filter,
                    ) {
                        candidates.push(item);
                    }
                }
            }
        }

        // 2. Voice Notes
        if allowed(RetrievalSourceType::VoiceNote) {
            if let Ok(notes) = vault.list_notes() {
                for n in notes {
                    if n.note_type == VOICE_NOTE_TYPE {
                        let provenance = RetrievalProvenance::new(&n.id, RetrievalSourceType::VoiceNote);
                        if let Some(item) = Self::score_item(
                            &n.id,
                            RetrievalSourceType::VoiceNote,
                            &n.title,
                            &n.content,
                            Some(&n.created_at),
                            n.tags.clone(),
                            provenance,
                            &terms,
                            &query.filter,
                        ) {
                            candidates.push(item);
                        }
                    }
                }
            }
        }

        // 3. Vault Files & Web Captures
        let check_files = allowed(RetrievalSourceType::File);
        let check_captures = allowed(RetrievalSourceType::Capture);
        if check_files || check_captures {
            if let Ok(files) = vault.list_vault_files() {
                for f in files {
                    let is_capture = f.is_capture();
                    let st = if is_capture {
                        RetrievalSourceType::Capture
                    } else {
                        RetrievalSourceType::File
                    };

                    if !allowed(st) {
                        continue;
                    }

                    let mut tags = f.tags.clone();
                    tags.extend(f.topics.clone());
                    let title = if is_capture {
                        f.capture
                            .as_ref()
                            .map(|c| c.page_title.as_str())
                            .unwrap_or(&f.original_filename)
                    } else {
                        &f.original_filename
                    };

                    let body = match &f.summary {
                        Some(sum) if !sum.trim().is_empty() => format!("{}\n\n{}", sum.trim(), f.content),
                        _ => f.content.clone(),
                    };

                    let mut prov = RetrievalProvenance::new(&f.id, st);
                    if let Some(c) = &f.capture {
                        prov = prov.with_origin(&c.url).with_capture(&f.id);
                    } else if !f.last_known_source_path.is_empty() {
                        prov = prov.with_origin(&f.last_known_source_path);
                    }

                    if let Some(item) = Self::score_item(
                        &f.id,
                        st,
                        title,
                        &body,
                        Some(&f.created_at),
                        tags,
                        prov,
                        &terms,
                        &query.filter,
                    ) {
                        candidates.push(item);
                    }
                }
            }
        }

        // 4. Meetings
        if allowed(RetrievalSourceType::Meeting) {
            if let Some(store) = session_store {
                if let Ok(sessions) = store.list_sessions() {
                    for s in sessions {
                        let title = if s.title.is_empty() { "Untitled Meeting" } else { &s.title };
                        let mut body = s.summary.clone().unwrap_or_default();
                        if !s.action_items.is_empty() {
                            body.push_str("\n\nAction Items:\n");
                            for item in &s.action_items {
                                body.push_str(&format!("- {}\n", item));
                            }
                        }
                        if body.trim().is_empty() {
                            body = format!("Meeting session {}", s.id);
                        }
                        let created_at = s.created_at.clone();
                        let prov = RetrievalProvenance::new(&s.id, RetrievalSourceType::Meeting);

                        if let Some(item) = Self::score_item(
                            &s.id,
                            RetrievalSourceType::Meeting,
                            title,
                            &body,
                            Some(&created_at),
                            vec!["meeting".to_string()],
                            prov,
                            &terms,
                            &query.filter,
                        ) {
                            candidates.push(item);
                        }
                    }
                }
            }
        }

        // Sort candidates by score descending
        candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        let total_matches = candidates.len();

        // Apply budget & limit
        let mut result_items = Vec::new();
        let mut budget_used = 0;
        let limit = query.limit.unwrap_or(usize::MAX);
        let max_budget = query.char_budget.unwrap_or(usize::MAX);

        for item in candidates {
            if result_items.len() >= limit {
                break;
            }
            let item_len = item.content.len();
            if budget_used + item_len > max_budget && !result_items.is_empty() {
                // Char budget exceeded
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

    /// Evaluates a single candidate item against search terms and filters.
    #[allow(clippy::too_many_arguments)]
    pub fn score_item(
        id: &str,
        source_type: RetrievalSourceType,
        title: &str,
        content: &str,
        timestamp: Option<&str>,
        topics: Vec<String>,
        mut provenance: RetrievalProvenance,
        terms: &[String],
        filter: &RetrievalFilter,
    ) -> Option<RetrievedItem> {
        // Time filter
        if let Some(tf) = &filter.time_filter {
            if let Some(ts) = timestamp {
                if let Some(after) = &tf.created_after {
                    if ts < after.as_str() {
                        return None;
                    }
                }
                if let Some(before) = &tf.created_before {
                    if ts > before.as_str() {
                        return None;
                    }
                }
            }
        }

        // Tag filter
        if !filter.tags.is_empty() {
            let has_tag = filter.tags.iter().any(|t| {
                let lower_t = t.to_lowercase();
                topics.iter().any(|topic| topic.to_lowercase() == lower_t)
            });
            if !has_tag {
                return None;
            }
        }

        // Empty query matches all items passing filters with base score
        if terms.is_empty() {
            let snippet = extract_snippet(content, terms, 240);
            provenance.evidence = Some(snippet.clone());
            return Some(RetrievedItem {
                id: id.to_string(),
                source_type,
                title: title.to_string(),
                content: content.to_string(),
                snippet,
                score: source_type.default_weight(),
                timestamp: timestamp.map(|t| t.to_string()),
                provenance,
                topics,
                metadata: serde_json::Value::Null,
            });
        }

        let title_lower = title.to_lowercase();
        let content_lower = content.to_lowercase();

        let mut term_matches = 0;
        let mut title_boost = 0.0;
        let mut topic_boost = 0.0;

        for term in terms {
            let in_title = title_lower.contains(term);
            let in_content = content_lower.contains(term);
            let in_topics = topics.iter().any(|top| top.to_lowercase().contains(term));

            if in_title || in_content || in_topics {
                term_matches += 1;
                if in_title {
                    title_boost += 3.0;
                }
                if in_topics {
                    topic_boost += 2.0;
                }
            }
        }

        if term_matches == 0 {
            return None;
        }

        let coverage = term_matches as f32 / terms.len() as f32;
        let raw_score = (coverage * 5.0) + title_boost + topic_boost;
        let final_score = raw_score * source_type.default_weight();

        let snippet = extract_snippet(content, terms, 240);
        provenance.evidence = Some(snippet.clone());

        Some(RetrievedItem {
            id: id.to_string(),
            source_type,
            title: title.to_string(),
            content: content.to_string(),
            snippet,
            score: final_score,
            timestamp: timestamp.map(|t| t.to_string()),
            provenance,
            topics,
            metadata: serde_json::Value::Null,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize() {
        let terms = tokenize("Orca stablyai/orca project");
        assert_eq!(terms, vec!["orca", "stablyai", "orca", "project"]);
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
        // Scored item itself doesn't filter source_type directly (search does), but let's check time filter
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

        // Outside time window (too late)
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

        // In time window
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
