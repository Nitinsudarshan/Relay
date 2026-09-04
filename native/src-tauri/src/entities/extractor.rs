//! Evidence-grounded entity and fact extraction.
//!
//! Extracts named entities across people, orgs, projects, tech, URLs, dates, and identifiers
//! strictly grounded in the source text.

use super::model::{EntityCategory, ExtractedEntity};
use std::collections::HashMap;

/// Known technology keywords with their canonical display names.
const TECH_CATALOG: &[(&str, &str)] = &[
    ("git", "Git"),
    ("rust", "Rust"),
    ("tauri", "Tauri"),
    ("react", "React"),
    ("next.js", "Next.js"),
    ("nextjs", "Next.js"),
    ("typescript", "TypeScript"),
    ("javascript", "JavaScript"),
    ("python", "Python"),
    ("webgl", "WebGL"),
    ("ssh", "SSH"),
    ("sqlite", "SQLite"),
    ("whisper", "Whisper"),
    ("piper", "Piper"),
    ("ollama", "Ollama"),
    ("docker", "Docker"),
    ("graphql", "GraphQL"),
    ("grpc", "gRPC"),
];

/// Known organizations and products with their canonical names.
const KNOWN_ORGS_PRODUCTS: &[(&str, &str, EntityCategory)] = &[
    ("stablyai", "stablyai", EntityCategory::Organization),
    ("github", "GitHub", EntityCategory::Organization),
    ("anthropic", "Anthropic", EntityCategory::Organization),
    ("openai", "OpenAI", EntityCategory::Organization),
    ("google", "Google", EntityCategory::Organization),
    ("relay", "Relay", EntityCategory::Product),
    ("orca", "Orca", EntityCategory::Product),
    ("claude code", "Claude Code", EntityCategory::Product),
    ("claudecode", "Claude Code", EntityCategory::Product),
    ("chatgpt", "ChatGPT", EntityCategory::Product),
];

/// Finds the full sentence enclosing a given byte offset.
fn find_enclosing_sentence(text: &str, match_pos: usize) -> String {
    let before = &text[..match_pos];
    let start = before.rfind(['.', '\n', '!', '?'])
        .map(|idx| idx + 1)
        .unwrap_or(0);

    let after = &text[match_pos..];
    let end_relative = after.find(['.', '\n', '!', '?'])
        .unwrap_or(after.len());

    let sentence = &text[start..match_pos + end_relative];
    sentence.trim().to_string()
}

pub struct EntityExtractor;

impl EntityExtractor {
    /// Deterministically extracts grounded entities from text for a given source.
    pub fn extract_deterministic(source_id: &str, content: &str) -> Vec<ExtractedEntity> {
        let mut results = Vec::new();
        let mut seen: HashMap<(String, EntityCategory), usize> = HashMap::new();

        if content.trim().is_empty() {
            return results;
        }

        let lower_content = content.to_lowercase();

        // 1. Extract URLs
        for word in content.split_whitespace() {
            let clean_word = word
                .trim_matches(&['(', ')', '[', ']', '<', '>', '"', '\'', ',', ';'][..])
                .trim_end_matches('.');
            if clean_word.starts_with("http://") || clean_word.starts_with("https://") {
                if let Some(pos) = content.find(clean_word) {
                    let evidence = find_enclosing_sentence(content, pos);
                    let key = (clean_word.to_string(), EntityCategory::Url);
                    if let Some(idx) = seen.get(&key) {
                        results[*idx].occurrences += 1;
                    } else {
                        seen.insert(key, results.len());
                        let mut ent = ExtractedEntity::new(clean_word, EntityCategory::Url, source_id, evidence);
                        ent.confidence = 1.0;
                        results.push(ent);
                    }
                }
            }
        }

        // 2. Extract GitHub repository identifiers (e.g., github.com/owner/repo or owner/repo)
        for word in content.split_whitespace() {
            let clean = word.trim_matches(&['(', ')', '[', ']', '<', '>', '"', '\'', ',', ';', '.'][..]);
            if clean.contains('/') && !clean.contains("://") && !clean.starts_with('/') && !clean.ends_with('/') {
                let parts: Vec<&str> = clean.split('/').collect();
                if parts.len() == 2 && parts[0].chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                    && parts[1].chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                    && !parts[0].is_empty() && !parts[1].is_empty()
                {
                    if let Some(pos) = content.find(clean) {
                        let evidence = find_enclosing_sentence(content, pos);
                        let key = (clean.to_string(), EntityCategory::Project);
                        if let Some(idx) = seen.get(&key) {
                            results[*idx].occurrences += 1;
                        } else {
                            seen.insert(key, results.len());
                            let mut ent = ExtractedEntity::new(clean, EntityCategory::Project, source_id, evidence);
                            ent.confidence = 0.95;
                            results.push(ent);
                        }
                    }
                }
            }
        }

        // 3. Extract Technologies from catalog
        for (keyword, canonical) in TECH_CATALOG {
            let mut search_from = 0;
            while let Some(pos) = lower_content[search_from..].find(keyword) {
                let actual_pos = search_from + pos;
                search_from = actual_pos + keyword.len();

                // Check word boundaries
                let before_char = if actual_pos > 0 {
                    lower_content[..actual_pos].chars().last()
                } else {
                    None
                };
                let after_char = lower_content[actual_pos + keyword.len()..].chars().next();

                let left_boundary = before_char.map(|c| !c.is_alphanumeric()).unwrap_or(true);
                let right_boundary = after_char.map(|c| !c.is_alphanumeric()).unwrap_or(true);

                if left_boundary && right_boundary {
                    let evidence = find_enclosing_sentence(content, actual_pos);
                    let key = (canonical.to_string(), EntityCategory::Technology);
                    if let Some(idx) = seen.get(&key) {
                        results[*idx].occurrences += 1;
                    } else {
                        seen.insert(key, results.len());
                        let mut ent = ExtractedEntity::new(*canonical, EntityCategory::Technology, source_id, evidence);
                        ent.confidence = 0.95;
                        results.push(ent);
                    }
                    break; // Count once per keyword in deterministic pass
                }
            }
        }

        // 4. Extract Orgs and Products
        for (keyword, canonical, category) in KNOWN_ORGS_PRODUCTS {
            let mut search_from = 0;
            while let Some(pos) = lower_content[search_from..].find(keyword) {
                let actual_pos = search_from + pos;
                search_from = actual_pos + keyword.len();

                let before_char = if actual_pos > 0 {
                    lower_content[..actual_pos].chars().last()
                } else {
                    None
                };
                let after_char = lower_content[actual_pos + keyword.len()..].chars().next();

                let left_boundary = before_char.map(|c| !c.is_alphanumeric()).unwrap_or(true);
                let right_boundary = after_char.map(|c| !c.is_alphanumeric()).unwrap_or(true);

                if left_boundary && right_boundary {
                    let evidence = find_enclosing_sentence(content, actual_pos);
                    let key = (canonical.to_string(), *category);
                    if let Some(idx) = seen.get(&key) {
                        results[*idx].occurrences += 1;
                    } else {
                        seen.insert(key, results.len());
                        let mut ent = ExtractedEntity::new(*canonical, *category, source_id, evidence);
                        ent.confidence = 0.95;
                        results.push(ent);
                    }
                    break;
                }
            }
        }

        // 5. Extract ISO dates (YYYY-MM-DD)
        for word in content.split_whitespace() {
            let clean = word.trim_matches(&['(', ')', '[', ']', '<', '>', '"', '\'', ',', ';', '.'][..]);
            if clean.len() == 10 {
                let chars: Vec<char> = clean.chars().collect();
                if chars[4] == '-' && chars[7] == '-'
                    && chars[0..4].iter().all(|c| c.is_ascii_digit())
                    && chars[5..7].iter().all(|c| c.is_ascii_digit())
                    && chars[8..10].iter().all(|c| c.is_ascii_digit())
                {
                    if let Some(pos) = content.find(clean) {
                        let evidence = find_enclosing_sentence(content, pos);
                        let key = (clean.to_string(), EntityCategory::Date);
                        if let Some(idx) = seen.get(&key) {
                            results[*idx].occurrences += 1;
                        } else {
                            seen.insert(key, results.len());
                            let mut ent = ExtractedEntity::new(clean, EntityCategory::Date, source_id, evidence);
                            ent.confidence = 1.0;
                            results.push(ent);
                        }
                    }
                }
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_evidence_grounded_entities() {
        let text = "Project Orca is developed by stablyai using Git, WebGL, and SSH. See stablyai/orca on https://github.com/stablyai/orca.";
        let entities = EntityExtractor::extract_deterministic("cap_100", text);

        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"Orca"));
        assert!(names.contains(&"stablyai"));
        assert!(names.contains(&"Git"));
        assert!(names.contains(&"WebGL"));
        assert!(names.contains(&"SSH"));
        assert!(names.contains(&"stablyai/orca"));
        assert!(names.contains(&"https://github.com/stablyai/orca"));

        // Check evidence retention
        let git_ent = entities.iter().find(|e| e.name == "Git").unwrap();
        assert_eq!(git_ent.category, EntityCategory::Technology);
        assert!(git_ent.evidence.contains("Git"));
        assert_eq!(git_ent.source_id, "cap_100");
    }

    #[test]
    fn test_do_not_extract_unmentioned_entities() {
        let text = "A simple note discussing weather and tea.";
        let entities = EntityExtractor::extract_deterministic("note_1", text);
        assert!(entities.is_empty());
    }
}
