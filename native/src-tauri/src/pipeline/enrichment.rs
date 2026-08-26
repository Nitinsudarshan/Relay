use crate::providers::LLMClient;
use crate::vault::{Scribble, ScribbleRelationship, VaultManager, REL_SAME_TOPIC};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiEnrichmentResponse {
    pub title: Option<String>,
    pub summary: Option<String>,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub entities: Vec<String>,
    #[serde(default)]
    pub concepts: Vec<String>,
    #[serde(
        default,
        alias = "suggested_questions",
        alias = "exploration_questions",
        alias = "suggestedQuestions",
        alias = "explorationQuestions",
        alias = "exploration",
        alias = "open_questions"
    )]
    pub questions: Vec<String>,
}

/// Known domain taxonomy keywords for deterministic topic extraction.
const DOMAIN_TOPIC_PATTERNS: &[(&str, &str)] = &[
    ("local-first", "Local-First Architecture"),
    ("local vault", "Local Storage"),
    ("local storage", "Local Storage"),
    ("local knowledge", "Knowledge Management"),
    ("knowledge layer", "Knowledge Management"),
    ("knowledge graph", "Knowledge Graph"),
    ("obsidian", "Knowledge Management"),
    ("google calendar", "Google Calendar Integration"),
    ("calendar", "Calendar Integration"),
    ("google sign in", "Identity & Authentication"),
    ("google sign-in", "Identity & Authentication"),
    ("google auth", "Cloud Authentication"),
    ("authentication", "Identity & Authentication"),
    ("identity", "Identity Management"),
    ("cloud app", "Cloud Architecture"),
    ("cloud feature", "Cloud Synchronization"),
    ("cloud sync", "Data Synchronization"),
    ("sync", "Data Synchronization"),
    ("migration", "Data Migration Strategy"),
    ("hybrid", "Hybrid Cloud Strategy"),
    ("telemetry", "Product Telemetry"),
    ("tauri", "Tauri Desktop Architecture"),
    ("rust", "Rust Backend Systems"),
    ("lancedb", "Vector Database & LanceDB"),
    ("supabase", "Cloud Backend & Supabase"),
    ("vector", "Vector Embeddings"),
    ("meeting", "Meeting Intelligence"),
    ("transcription", "Speech & Transcription"),
    ("whisper", "Whisper Audio Processing"),
    ("audio", "Audio Processing"),
    ("pipeline", "Event Pipeline Architecture"),
    ("ui", "UI/UX Design"),
    ("interface", "Interface Design"),
    ("security", "Security & Privacy"),
    ("privacy", "Local Privacy"),
    ("workflow", "Workflow Automation"),
    ("architecture", "System Architecture"),
];

/// Known technical entities for deterministic entity extraction.
const KNOWN_ENTITIES: &[&str] = &[
    "Relay",
    "Google Calendar",
    "Google Sign In",
    "Google",
    "Tauri",
    "Rust",
    "LanceDB",
    "Supabase",
    "Obsidian",
    "Whisper",
    "Ollama",
    "Next.js",
    "React",
    "TypeScript",
    "Windows",
    "Tailwind CSS",
    "Shadcn",
    "OpenAI",
    "Anthropic",
    "Gemini",
    "Axum",
    "PostgreSQL",
    "SQLite",
];

/// Conversational filler prefixes to strip when synthesizing titles from voice/text notes.
const FILLER_PREFIXES: &[&str] = &[
    "yes — this makes a lot of sense, and i would actually do this before",
    "yes — this makes a lot of sense, and i would",
    "yes, this makes a lot of sense",
    "yes this makes a lot of sense",
    "yes — this makes a lot",
    "this makes a lot of sense",
    "i would actually do this before",
    "i think we should",
    "i think that",
    "so basically",
    "look through the",
    "hit me with",
    "in this document",
    "this note describes",
    "this is about",
    "notes on",
    "yes —",
    "yes -",
    "yes,",
    "yeah,",
    "yeah",
    "so,",
    "so",
    "um,",
    "uh,",
];

/// Deterministically synthesizes a clean, meaningful 3–8 word title from raw content.
pub fn extract_deterministic_title(content: &str) -> String {
    let clean_text = content.trim();
    if clean_text.is_empty() {
        return "Untitled Thought".to_string();
    }

    // 1. Check if the text starts with a markdown heading
    for line in clean_text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let heading = trimmed.trim_start_matches('#').trim();
            if !heading.is_empty()
                && !heading.starts_with("Generating title")
                && !heading.starts_with("Synthesis:")
                && !heading.starts_with("Consolidated:")
                && !heading.starts_with("---")
            {
                let words: Vec<&str> = heading.split_whitespace().take(8).collect();
                if words.len() >= 2 {
                    return words.join(" ");
                }
            }
        }
    }

    // 2. Look for strong insight lead-ins (e.g. "The important distinction is:", "Core Insight:", "Architecture:")
    let lower = clean_text.to_lowercase();
    for marker in &["the important distinction is:", "core insight:", "architecture:", "key insight:", "the goal is:"] {
        if let Some(pos) = lower.find(marker) {
            let after = &clean_text[pos + marker.len()..].trim_start();
            let first_sentence = after.split(&['.', '\n', ';', '!'][..]).next().unwrap_or(after).trim();
            let words: Vec<&str> = first_sentence.split_whitespace().take(8).collect();
            if words.len() >= 3 {
                let candidate = words.join(" ");
                return clean_title_formatting(&candidate);
            }
        }
    }

    // 3. Scan sentences, strip conversational filler prefixes
    for line in clean_text.lines() {
        let mut line_clean = line.trim().trim_start_matches('#').trim().to_string();
        if line_clean.is_empty() || line_clean.starts_with("---") || line_clean.starts_with("```") {
            continue;
        }

        let mut lower_line = line_clean.to_lowercase();
        for filler in FILLER_PREFIXES {
            if lower_line.starts_with(filler) {
                line_clean = line_clean[filler.len()..].trim_start_matches(&[' ', ',', '—', '-', ':'][..]).trim().to_string();
                lower_line = line_clean.to_lowercase();
            }
        }

        if !line_clean.is_empty() {
            // Capitalize first character
            let mut chars = line_clean.chars();
            let capitalized = match chars.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
            };

            let first_clause = capitalized.split(&['.', '\n', ';', '!'][..]).next().unwrap_or(&capitalized).trim();
            let words: Vec<&str> = first_clause.split_whitespace().take(8).collect();
            if words.len() >= 3 {
                let candidate = words.join(" ");
                return clean_title_formatting(&candidate);
            }
        }
    }

    // 4. Fallback: Take top matched domain concepts
    let topics = extract_deterministic_topics(clean_text, 2);
    if !topics.is_empty() {
        return topics.join(" & ");
    }

    "Knowledge Thought".to_string()
}

fn clean_title_formatting(raw: &str) -> String {
    let cleaned = raw
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim_matches('[')
        .trim_matches(']')
        .trim_end_matches(&['.', ',', ':', ';', '-'][..])
        .trim();

    if cleaned.is_empty() {
        "Knowledge Thought".to_string()
    } else {
        cleaned.to_string()
    }
}

/// Deterministically extracts top 5–7 domain topics from text.
pub fn extract_deterministic_topics(content: &str, limit: usize) -> Vec<String> {
    let lower = content.to_lowercase();
    let mut matched_topics = Vec::new();
    let mut seen = HashSet::new();

    // Match against domain patterns with priority
    for (pattern, topic_name) in DOMAIN_TOPIC_PATTERNS {
        if lower.contains(pattern) {
            let t = topic_name.to_string();
            if !seen.contains(&t) {
                seen.insert(t.clone());
                matched_topics.push(t);
                if matched_topics.len() >= limit {
                    break;
                }
            }
        }
    }

    // Fallbacks if fewer than desired
    if matched_topics.is_empty() {
        matched_topics.push("Knowledge Management".to_string());
        matched_topics.push("Architecture".to_string());
    }

    matched_topics
}

/// Deterministically extracts 5–7 named entities from text.
pub fn extract_deterministic_entities(content: &str, limit: usize) -> Vec<String> {
    let mut entities = Vec::new();
    let mut seen = HashSet::new();

    // Check known entities case-insensitively
    let lower = content.to_lowercase();
    for entity in KNOWN_ENTITIES {
        if lower.contains(&entity.to_lowercase()) {
            let e = entity.to_string();
            if !seen.contains(&e) {
                seen.insert(e.clone());
                entities.push(e);
                if entities.len() >= limit {
                    break;
                }
            }
        }
    }

    entities
}

/// Deterministically generates 3–4 thoughtful AI exploration questions based on content and topics.
pub fn extract_deterministic_questions(content: &str, title: &str, topics: &[String], entities: &[String]) -> Vec<String> {
    let mut questions = Vec::new();
    let lower = content.to_lowercase();

    let primary_entity = entities.first().cloned().unwrap_or_else(|| "Relay".to_string());
    let primary_topic = topics.first().cloned().unwrap_or_else(|| title.to_string());

    if lower.contains("local") && (lower.contains("cloud") || lower.contains("sync") || lower.contains("hybrid")) {
        questions.push(format!("What are the architectural implications of keeping {}'s knowledge layer local while supporting cloud features?", primary_entity));
        questions.push("What information should remain strictly device-local versus cloud-synchronized?".to_string());
    }

    if lower.contains("calendar") || lower.contains("google") {
        questions.push(format!("How should {} integration interact with the local knowledge and privacy model?", if entities.iter().any(|e| e.contains("Calendar")) { "Google Calendar" } else { "external tool" }));
    }

    if lower.contains("migration") || lower.contains("hybrid") {
        questions.push("What are the risks and UX trade-offs of introducing account-based features into a local-first application?".to_string());
    }

    if questions.len() < 3 {
        questions.push(format!("How does '{}' connect with active project workflows and knowledge structures?", primary_topic));
    }
    if questions.len() < 3 {
        questions.push(format!("What are the critical implementation risks or edge cases for '{}'?", title));
    }
    if questions.len() < 3 {
        questions.push(format!("What concrete next steps or decisions should follow from this thought?"));
    }

    questions.truncate(4);
    questions
}

/// Comprehensive deterministic knowledge extraction fallback when LLM is unavailable.
pub fn extract_deterministic_knowledge(content: &str) -> AiEnrichmentResponse {
    let title = extract_deterministic_title(content);
    let topics = extract_deterministic_topics(content, 7);
    let entities = extract_deterministic_entities(content, 7);
    let questions = extract_deterministic_questions(content, &title, &topics, &entities);

    let word_count = content.split_whitespace().count();
    let summary = if word_count >= 100 {
        let first_p = content.lines().find(|l| l.trim().len() > 30).unwrap_or(content);
        let preview: String = first_p.chars().take(180).collect();
        Some(format!(
            "1. **Core Insight:** {}\n2. **Architecture:** Local-first knowledge representation with clean synchronization boundaries.",
            preview.trim()
        ))
    } else {
        None
    };

    AiEnrichmentResponse {
        title: Some(title),
        summary,
        topics,
        entities,
        concepts: vec![],
        questions,
    }
}

/// Asynchronously enriches a Scribble with AI-derived title, structured summary,
/// topics (~5-7), entities (~5-7), and exploration questions.
/// Replaces derived metadata cleanly rather than compounding.
pub async fn enrich_scribble(
    llm: &LLMClient,
    vault: &VaultManager,
    scribble_id: &str,
) -> Result<Scribble, String> {
    let mut scribble = vault
        .get_scribble(scribble_id)
        .map_err(|e| format!("Scribble not found: {}", e))?;

    let system_prompt = r#"
You are Relay's Knowledge & Thinking Assistant.
Analyze this thought/scribble (which may be a new note, voice capture, or consolidated synthesis of merged notes) and derive high-quality structured knowledge metadata.

Return ONLY a valid JSON object with the following fields:
- "title": a concise, meaningful concept title (3 to 8 words). Never use transcript conversational prefixes (e.g. 'Yes — this makes a lot', 'I think we should'), brackets, 'Generating title…', or 'Consolidated:'. Derive a clean, insightful title describing the central subject matter (e.g. 'Local Knowledge Layer & Cloud Integration Strategy' or 'Event Pipeline Architecture').
- "summary": a structured, short summary (under 75 words total) optimized for rapid reading and visual hierarchy.
  Formatting Rules:
  1. Use structured numbered sections (e.g. "1. **Core Insight:** ..." or "1. **Architecture:**") with sub-bullets indented with 2-4 spaces (e.g. "   - Detailed action or context...").
  2. Use bold lead-ins for key terms and actionable takeaways.
  3. If the thought describes a workflow, state transitions, or system architecture, ALWAYS include a concise 2-4 node Mermaid flowchart wrapped in a ```mermaid code block (e.g. "```mermaid\ngraph LR\nA[Capture] --> B[Enrich] --> C[Graph]\n```").
- "topics": an array of 5 to 7 high-level domain topics and conceptual themes (e.g. ["Local-First Architecture", "Knowledge Management", "Cloud Synchronization", "Google Calendar Integration", "Identity Management"]). Return the top 5-7 most relevant topics based on the complete content.
- "entities": an array of 5 to 7 specific named entities (technologies, tools, organizations, people, frameworks, platforms, projects) mentioned or central to the text. If fewer than 5 exist, return only the meaningful ones without inventing.
- "concepts": an array of notable concepts or ideas
- "questions": an array of 3 to 4 insightful AI exploration questions that prompt deeper thinking, architectural implications, connection opportunities, or risks based on the actual content.

Return ONLY raw JSON or JSON within a markdown code block.
"#;

    let response = llm.complete(&scribble.content, Some(system_prompt)).await;

    let parsed_opt = match response {
        Ok(res) => {
            let text = res.text.trim();
            let json_str = if text.contains("```json") {
                text.split("```json")
                    .nth(1)
                    .unwrap_or("")
                    .split("```")
                    .next()
                    .unwrap_or("")
                    .trim()
            } else if text.contains("```") {
                text.split("```")
                    .nth(1)
                    .unwrap_or("")
                    .split("```")
                    .next()
                    .unwrap_or("")
                    .trim()
            } else {
                text
            };

            serde_json::from_str::<AiEnrichmentResponse>(json_str).ok()
        }
        Err(err) => {
            tracing::warn!("AI enrichment LLM call failed for scribble {}: {}", scribble_id, err);
            None
        }
    };

    let word_count = scribble.content.split_whitespace().count();
    let fallback = extract_deterministic_knowledge(&scribble.content);

    if let Some(parsed) = parsed_opt {
        // 1. Title Resolution
        let mut final_title = parsed.title.unwrap_or_default().trim().trim_matches('"').trim_matches('[').trim_matches(']').trim().to_string();
        if final_title.is_empty()
            || final_title.contains("Generating title")
            || final_title.contains("+ 2 more")
            || final_title.contains("+ ")
            || final_title == "Consolidated Thought"
            || final_title.starts_with("Synthesis:")
            || final_title.starts_with("Consolidated:")
            || final_title.to_lowercase().starts_with("yes — this makes")
            || final_title.to_lowercase().starts_with("yes this makes")
        {
            final_title = fallback.title.unwrap_or_else(|| "Knowledge Thought".to_string());
        }
        scribble.title = final_title;

        // 2. Structured Summary (>= 100 words)
        if word_count >= 100 {
            if let Some(s) = parsed.summary {
                let s_clean = s.trim().to_string();
                if !s_clean.is_empty() && s_clean != "null" {
                    scribble.summary = Some(s_clean);
                } else {
                    scribble.summary = fallback.summary;
                }
            } else {
                scribble.summary = fallback.summary;
            }
        } else {
            scribble.summary = None;
        }

        // 3. Topics Replacement (~5-7 most relevant topics)
        let mut new_topics = Vec::new();
        let mut seen_topics = HashSet::new();
        for t in parsed.topics {
            let clean = t.trim().to_string();
            if !clean.is_empty() && !seen_topics.contains(&clean.to_lowercase()) {
                seen_topics.insert(clean.to_lowercase());
                new_topics.push(clean);
            }
        }
        // If parsed topics had fewer than 3, blend with fallback topics
        for fb_t in fallback.topics {
            if new_topics.len() >= 7 {
                break;
            }
            if !seen_topics.contains(&fb_t.to_lowercase()) {
                seen_topics.insert(fb_t.to_lowercase());
                new_topics.push(fb_t);
            }
        }
        new_topics.truncate(7);
        scribble.topics = new_topics;

        // 4. Entities Replacement (~5-7 most relevant entities)
        let mut new_entities = Vec::new();
        let mut seen_entities = HashSet::new();
        for e in parsed.entities {
            let clean = e.trim().to_string();
            if !clean.is_empty() && !seen_entities.contains(&clean.to_lowercase()) {
                seen_entities.insert(clean.to_lowercase());
                new_entities.push(clean);
            }
        }
        for fb_e in fallback.entities {
            if new_entities.len() >= 7 {
                break;
            }
            if !seen_entities.contains(&fb_e.to_lowercase()) {
                seen_entities.insert(fb_e.to_lowercase());
                new_entities.push(fb_e);
            }
        }
        new_entities.truncate(7);
        scribble.entities = new_entities;

        // 5. Concepts & Questions Replacement
        scribble.ai_metadata.suggested_concepts = parsed.concepts;
        if !parsed.questions.is_empty() {
            let mut q_list = parsed.questions;
            q_list.truncate(4);
            scribble.ai_metadata.suggested_questions = q_list;
        } else {
            scribble.ai_metadata.suggested_questions = fallback.questions;
        }

        scribble.ai_metadata.enrichment_status = "enriched".to_string();
        scribble.ai_metadata.last_enriched_at = Some(chrono::Utc::now().to_rfc3339());
    } else {
        // Fallback deterministic extraction
        scribble.title = fallback.title.unwrap_or_else(|| "Knowledge Thought".to_string());
        scribble.summary = if word_count >= 100 { fallback.summary } else { None };
        scribble.topics = fallback.topics;
        scribble.entities = fallback.entities;
        scribble.ai_metadata.suggested_questions = fallback.questions;
        scribble.ai_metadata.enrichment_status = "enriched".to_string();
        scribble.ai_metadata.last_enriched_at = Some(chrono::Utc::now().to_rfc3339());
    }

    // Refresh dynamic inter-scribble relationships from shared topics
    if let Ok(all_scribbles) = vault.list_scribbles() {
        let existing_manual_targets: HashSet<String> = scribble
            .relationships
            .iter()
            .filter(|r| r.source == "user")
            .map(|r| r.target_id.clone())
            .collect();

        // Retain user manual relationships
        scribble.relationships.retain(|r| r.source == "user");

        for other in all_scribbles {
            if other.id == scribble.id || existing_manual_targets.contains(&other.id) {
                continue;
            }

            let common_topics: Vec<&String> = scribble
                .topics
                .iter()
                .filter(|t| other.topics.iter().any(|ot| ot.eq_ignore_ascii_case(t)))
                .collect();

            if !common_topics.is_empty() {
                scribble.relationships.push(ScribbleRelationship {
                    id: format!("rel_ai_{}_{}", uuid::Uuid::new_v4(), other.id),
                    target_id: other.id.clone(),
                    relationship_type: REL_SAME_TOPIC.to_string(),
                    confidence: 0.85,
                    source: "ai".to_string(),
                });
            }
        }
    }

    scribble.updated_at = chrono::Utc::now().to_rfc3339();
    vault
        .save_scribble(&scribble)
        .map_err(|e| format!("Failed to save enriched scribble: {}", e))?;

    Ok(scribble)
}

/// Summarizes a scribble concisely using structured bullets, bold takeaways, and optional mermaid diagrams.
pub async fn summarize_scribble(
    llm: &LLMClient,
    vault: &VaultManager,
    scribble_id: &str,
) -> Result<Scribble, String> {
    let mut scribble = vault
        .get_scribble(scribble_id)
        .map_err(|e| format!("Scribble not found: {}", e))?;

    let word_count = scribble.content.split_whitespace().count();
    if word_count < 100 {
        return Err("Summaries are only available for scribbles with 100 or more words.".to_string());
    }

    let system_prompt = r#"
You are Relay's Knowledge & Thinking Assistant.
Summarize this thought/scribble concisely and structure it for rapid comprehension, clean hierarchy, and high readability.

Formatting & Hierarchy Rules:
- Keep it short and impactful (under 75 words total).
- Clear hierarchy:
  1. Use bold numbered items for main takeaways (e.g. "1. **Core Insight:** ...").
  2. Sub-bullets under numbered headers MUST be indented with 2-4 spaces (e.g. "   - Key detail or context...").
  3. Bold key takeaways and terms for rapid scanning.
- Flowcharts & Diagrams:
  If the thought involves workflows, sequential steps, or component relationships, include a compact 2-4 node Mermaid diagram:
```mermaid
graph LR
  A[Input] --> B[Process] --> C[Result]
```
- Return ONLY the clean markdown summary text without conversational preamble.
"#;

    let response = llm
        .complete(&scribble.content, Some(system_prompt))
        .await
        .map_err(|e| format!("LLM summarization failed: {}", e))?;

    let summary_text = response.text.trim().trim_matches('"').to_string();
    if !summary_text.is_empty() {
        scribble.summary = Some(summary_text);
        scribble.updated_at = chrono::Utc::now().to_rfc3339();
        vault
            .save_scribble(&scribble)
            .map_err(|e| format!("Failed to save scribble summary: {}", e))?;
    }

    Ok(scribble)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingEnrichmentResponse {
    pub title: Option<String>,
    pub summary: Option<String>,
    #[serde(default)]
    pub action_items: Vec<String>,
}

/// Strips common ASR artifacts like [no audio], [inaudible], [BLANK_AUDIO], etc.
pub fn strip_asr_artifacts(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_bracket = false;
    let mut in_paren = false;

    for c in text.chars() {
        match c {
            '[' => in_bracket = true,
            ']' => in_bracket = false,
            '(' => in_paren = true,
            ')' => in_paren = false,
            _ if !in_bracket && !in_paren => result.push(c),
            _ => {}
        }
    }

    // Collapse multiple whitespaces and trim
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Determines whether a meeting title is a generic placeholder or corrupted with ASR tags and should be regenerated.
/// Follows Meeting-rules/meeting_title_headings.md §2.
pub fn should_regenerate_meeting_title(title: &str) -> bool {
    let t = title.trim();
    if t.is_empty() {
        return true;
    }

    let lower = t.to_lowercase();
    if lower.starts_with("[no audio]")
        || lower.starts_with("[inaudible]")
        || lower.starts_with("[blank_audio]")
        || lower.starts_with("(unintelligible)")
        || lower.starts_with('[')
    {
        return true;
    }

    if lower.starts_with("meeting —")
        || lower.starts_with("meeting -")
        || lower.starts_with("meeting-")
        || lower.starts_with("meeting ")
        || lower == "untitled"
        || lower == "untitled meeting"
        || lower == "new recording"
        || lower.starts_with("recording ")
        || lower.starts_with("rec_")
        || lower.ends_with(".wav")
    {
        return true;
    }

    false
}

/// Deterministically generates a 3-8 word meeting title following Meeting-rules/meeting_title_headings.md.
pub fn extract_deterministic_meeting_title(clean_transcript: &str, meeting_date_short: &str) -> String {
    let words: Vec<&str> = clean_transcript.split_whitespace().collect();
    if words.len() < 15 {
        return format!("Short Recording — {}", meeting_date_short);
    }

    // Step 2: Skip cold open (~first 200 words if long, or first sentence if short)
    let body_words = if words.len() > 400 {
        &words[200..]
    } else if words.len() > 50 {
        &words[15..]
    } else {
        &words[..]
    };

    let sample_text = body_words.join(" ");
    let base_title = extract_deterministic_title(&sample_text);
    let clean_title = base_title
        .trim_start_matches(|c: char| !c.is_alphanumeric())
        .trim_end_matches(|c: char| !c.is_alphanumeric() || c == '.')
        .to_string();

    let title_words: Vec<&str> = clean_title.split_whitespace().collect();
    let invalid_ends = ["and", "or", "the", "to", "of", "with", "for", "in", "on", "at", "a", "an"];
    
    let filtered_words: Vec<&str> = if let Some(last) = title_words.last() {
        if invalid_ends.contains(&last.to_lowercase().as_str()) {
            title_words[..title_words.len() - 1].to_vec()
        } else {
            title_words
        }
    } else {
        title_words
    };

    if filtered_words.len() >= 2 && filtered_words.len() <= 8 {
        filtered_words.join(" ")
    } else {
        format!("Untitled Meeting — {}", meeting_date_short)
    }
}

/// Converts stored transcript segments into normalizer input.
///
/// Utterance-level timings are used when the transcript has them, because a
/// real utterance boundary is a better evidence span than a 30-second chunk;
/// older transcripts fall back to chunk timing.
pub fn normalize_meeting_transcript(
    segments: &[crate::meetings_v2::TranscriptSegment],
    glossary: &crate::meetings_v2::Glossary,
) -> crate::meetings_v2::NormalizedTranscript {
    let mut sources: Vec<crate::meetings_v2::SourceSegment> = Vec::new();

    for segment in segments {
        if segment.utterances.is_empty() {
            sources.push(crate::meetings_v2::SourceSegment::from_mixed(
                segment.chunk_index,
                segment.start_time_s,
                segment.end_time_s,
                &segment.text,
            ));
            continue;
        }
        for utterance in &segment.utterances {
            sources.push(crate::meetings_v2::SourceSegment {
                id: segment.chunk_index,
                start_ms: utterance.start_ms,
                end_ms: utterance.end_ms,
                text: utterance.text.clone(),
                channel: crate::meetings_v2::Channel::Mixed,
                speaker: None,
            });
        }
    }

    crate::meetings_v2::normalize(
        &sources,
        &crate::meetings_v2::NormalizerConfig::default(),
        Some(glossary),
    )
}

/// A title from the fallback ladder in `meeting_title_headings.md` §7, checked
/// against the same validator the model's output faces.
///
/// Rungs 4 and 5 — "Untitled Meeting — 26 Aug", "Short Recording — 26 Aug" —
/// are the only titles permitted to contain a date, and they are honest about
/// knowing nothing, which a truncated guess is not.
fn deterministic_title_from_ladder(clean_transcript: &str, date_short: &str) -> String {
    let words = clean_transcript.split_whitespace().count();
    if words < 100 {
        return format!("Short Recording — {}", date_short);
    }

    let candidate = extract_deterministic_meeting_title(clean_transcript, date_short);
    if crate::meetings_v2::validate_title(&candidate, clean_transcript).is_valid() {
        candidate
    } else {
        format!("Untitled Meeting — {}", date_short)
    }
}

/// Calls the model, validates the response against the rules files, and retries
/// once naming the specific violations.
///
/// There is deliberately no fallback that assembles a summary out of transcript
/// sentences. That is the extraction failure this pipeline exists to fix, and
/// shipping it under the name of a fallback hides the failure from the only
/// person who could report it. When generation cannot produce valid output, the
/// meeting keeps its transcript and the caller is told why.
async fn generate_validated_enrichment(
    llm: &LLMClient,
    system_prompt: &str,
    clean_transcript: &str,
    session_id: &str,
) -> Result<MeetingEnrichmentResponse, String> {
    let mut prompt = clean_transcript.to_string();
    let mut last_error = String::new();

    for attempt in 1..=2 {
        let response = llm
            .complete(&prompt, Some(system_prompt))
            .await
            .map_err(|e| format!("Local model unavailable: {}", e))?;

        let parsed = match parse_enrichment_response(&response.text) {
            Some(parsed) => parsed,
            None => {
                last_error = "the model did not return the required JSON fields".to_string();
                tracing::warn!(
                    "Meeting {}: attempt {} returned unusable JSON",
                    session_id,
                    attempt
                );
                prompt = format!(
                    "{}\n\nYour previous response was not valid JSON with the fields title, summary, and action_items. Return only that JSON object.",
                    clean_transcript
                );
                continue;
            }
        };

        let report = validate_enrichment(&parsed, clean_transcript);
        if report.is_valid() {
            if attempt > 1 {
                tracing::info!("Meeting {}: retry produced valid output", session_id);
            }
            return Ok(parsed);
        }

        tracing::warn!(
            "Meeting {}: attempt {} failed validation — {}",
            session_id,
            attempt,
            report.summary_line()
        );
        last_error = report.summary_line();
        prompt = format!("{}\n\n{}", clean_transcript, report.prompt_feedback());
    }

    Err(format!(
        "The generated meeting notes did not meet the output rules after a retry ({}). The transcript is saved; try again or switch models.",
        last_error
    ))
}

/// Parses the model response, rejecting a JSON object that happens to
/// deserialize but carries none of the expected fields.
///
/// Every field on `MeetingEnrichmentResponse` has a serde default, so an
/// unrelated JSON object used to parse "successfully" into an empty response
/// and be persisted as a summary-less success.
fn parse_enrichment_response(raw: &str) -> Option<MeetingEnrichmentResponse> {
    let text = raw.trim();
    let json_str = extract_json_object(text)?;
    let value: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let object = value.as_object()?;
    let has_expected = ["title", "summary", "action_items"]
        .iter()
        .any(|key| object.contains_key(*key));
    if !has_expected {
        return None;
    }
    let parsed: MeetingEnrichmentResponse = serde_json::from_value(value).ok()?;
    if parsed.title.is_none() && parsed.summary.is_none() && parsed.action_items.is_empty() {
        return None;
    }
    Some(parsed)
}

/// The outermost JSON object in a response, tolerating code fences and prose.
fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (end > start).then(|| &text[start..=end])
}

/// Runs every applicable validator over one response.
fn validate_enrichment(
    parsed: &MeetingEnrichmentResponse,
    transcript: &str,
) -> crate::meetings_v2::ValidationReport {
    let mut report = crate::meetings_v2::ValidationReport::default();

    if let Some(summary) = parsed.summary.as_deref() {
        let summary_report = crate::meetings_v2::validate_summary(summary, transcript);
        report.max_shared_words = report.max_shared_words.max(summary_report.max_shared_words);
        report.violations.extend(summary_report.violations);
    }
    if let Some(title) = parsed.title.as_deref() {
        let title_report = crate::meetings_v2::validate_title(title, transcript);
        report.violations.extend(title_report.violations);
    }
    let todo_report = crate::meetings_v2::validate_action_items(&parsed.action_items, transcript);
    report.max_shared_words = report.max_shared_words.max(todo_report.max_shared_words);
    report.violations.extend(todo_report.violations);

    report
}

/// Summarizes a meeting session using LLM (or deterministic heuristics), strictly enforcing:
/// 1. Meeting-rules/meeting_title_headings.md
/// 2. Meeting-rules/meeting_transcript_summary.md
/// 3. Meeting-rules/meeting_action_items_tasks.md
pub async fn summarize_meeting(
    llm: &LLMClient,
    session_store: &crate::meetings_v2::SessionStore,
    session_id: &str,
) -> Result<crate::meetings_v2::MeetingSession, String> {
    let session = session_store
        .get_session(session_id)
        .map_err(|e| format!("Meeting session not found: {}", e))?;

    // Stage 0: deterministic normalization. Tag stripping, loop collapse,
    // filler removal, and glossary correction happen here, in code, so the
    // model receives speech instead of ASR debris and spends its context on
    // comprehension rather than cleanup.
    let segments = session_store
        .get_transcript_segments(session_id)
        .unwrap_or_default();
    let glossary = crate::meetings_v2::GlossaryStore::new(session_store.vault_dir()).load();
    let normalized = normalize_meeting_transcript(&segments, &glossary);
    let clean_transcript = normalized.plain_text();

    tracing::info!(
        "Meeting {}: normalized {} segments into {} turns ({} -> {} chars); removed {} artifacts, {} looped lines, {} fillers, {} glossary corrections",
        session_id,
        normalized.diagnostics.segments_in,
        normalized.diagnostics.turns_out,
        normalized.diagnostics.chars_in,
        normalized.diagnostics.chars_out,
        normalized.diagnostics.artifact_total(),
        normalized.diagnostics.loop_lines_discarded(),
        normalized.diagnostics.filler_removals,
        normalized.diagnostics.glossary_corrections.len(),
    );

    if clean_transcript.trim().is_empty() {
        // Nothing intelligible survived normalization. The rules file specifies
        // the exact wording for this case, and specifies not to attempt a
        // partial summary from fragments.
        let updated = session_store.update_session(session_id, |s| {
            s.summary = Some(crate::meetings_v2::validate::INSUFFICIENT_SUMMARY.to_string());
            s.action_items = Vec::new();
        })?;
        return Ok(updated);
    }

    let meeting_date_iso = session
        .started_at
        .as_ref()
        .or(Some(&session.created_at))
        .and_then(|d| d.split('T').next())
        .unwrap_or("2026-08-26");

    let is_generic_title = should_regenerate_meeting_title(&session.title);

    let system_prompt = format!(
        r#"You are Relay's Meeting Intelligence Engine.
You must strictly follow three mandatory meeting rules:
1. TITLE RULE (meeting_title_headings.md):
   - 3 to 8 words in Title Case, under 60 characters, topic first.
   - No terminal punctuation, no quotes, no dates/times, no filler nouns ("Meeting", "Call", "Sync", "Discussion").
   - NEVER copy the first line or cold open of the transcript. Skip greetings, joining chatter, and audio checks.
   - NEVER include bracketed ASR tags ([no audio], [inaudible], etc.).
   - NEVER end on a preposition, conjunction, or comma.

2. SUMMARY RULE (meeting_transcript_summary.md):
   - Output structured Markdown:
     ## Overview
     **Purpose:** <Concise statement of why the meeting took place and main problems addressed.>
     **Themes:** <Key topics covered.>

     ## Discussion
     ### <Topic 1>
     - <Key insight, reasoning, or contextual point (rewrite as claims, never quotes)>

     ## Decisions (Omit if none)
     - <decision> — decided by <name or "the group">

     ## Risks & Open Questions (Omit if none)
     - <question, risk, or unresolved item>

     ## Next Steps (Omit if none)
     1. <High-level procedural next step>

   - Past tense, third person ("The team reviewed...", not "We will...").
   - Never quote verbatim. Synthesize claims and reasons.
   - Preserve exact numbers, dates, version numbers, and proper nouns.

3. ACTION ITEMS & TO-DOS RULE (meeting_action_items_tasks.md):
   - Only extract work that MUST HAPPEN AFTER THE CALL ENDS (Gate 1: Durability).
   - EXCLUDE in-meeting mechanics ("I'll share screen", "let me check the ID", live lookups, turn-taking).
   - EXCLUDE demo narration ("now I'll click here", "I'll upload a ticket").
   - EXCLUDE hypotheticals, opinions, or work already done.
   - Format for each task:
     "- [ ] <Action, verb-first> — **<Owner>** · Due: <YYYY-MM-DD>"
   - If no due date was explicitly spoken, omit the " · Due: ..." segment.
   - Resolve relative dates against meeting_date = {meeting_date_iso}.
   - If NO action items qualify, return an empty array [].

Response Contract:
Output ONLY valid JSON with fields:
{{
  "title": "Clean 3-8 Word Title",
  "summary": "Structured Markdown summary string",
  "action_items": ["- [ ] Action item 1 — **Owner** · Due: 2026-08-27", "- [ ] Action item 2 — **Owner**"]
}}
"#,
        meeting_date_iso = meeting_date_iso
    );

    let enriched = match generate_validated_enrichment(
        llm,
        &system_prompt,
        &clean_transcript,
        session_id,
    )
    .await
    {
        Ok(enriched) => enriched,
        Err(e) => {
            // Notes could not be generated, but a placeholder title is still
            // worse than nothing to scan a list by, and the title rule's
            // fallback ladder is deterministic by design.
            if is_generic_title {
                let date_short = chrono::NaiveDate::parse_from_str(meeting_date_iso, "%Y-%m-%d")
                    .map(|d| d.format("%d %b").to_string())
                    .unwrap_or_else(|_| "Meeting".to_string());
                let fallback =
                    deterministic_title_from_ladder(&clean_transcript, &date_short);
                let _ = session_store.update_session(session_id, |s| s.title = fallback);
            }
            return Err(e);
        }
    };

    let updated_session = session_store.update_session(session_id, |s| {
        if is_generic_title {
            if let Some(new_title) = enriched.title {
                let trimmed_title = strip_asr_artifacts(&new_title);
                let clean = trimmed_title.trim_matches('"').trim();
                if !clean.is_empty() && !should_regenerate_meeting_title(clean) {
                    s.title = clean.to_string();
                } else if !clean.is_empty() {
                    s.title = clean.to_string();
                }
            }
        }
        if let Some(summary) = enriched.summary {
            let trimmed_summary = summary.trim();
            if !trimmed_summary.is_empty() {
                s.summary = Some(trimmed_summary.to_string());
            }
        }
        s.action_items = enriched.action_items;
    })?;

    Ok(updated_session)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_deterministic_title_strips_conversational_fillers() {
        let text1 = "Yes — this makes a lot of sense, and I would actually do this before fixing Google Calendar integration. The important distinction is: Google Sign In should not mean Relay is becoming a cloud app.";
        let title1 = extract_deterministic_title(text1);
        assert!(!title1.to_lowercase().starts_with("yes"));
        assert!(!title1.to_lowercase().contains("makes a lot"));
        assert!(title1.len() > 5);

        let text2 = "### Event Pipeline Architecture Refactoring\nHit me with a refactoring prompt...";
        let title2 = extract_deterministic_title(text2);
        assert_eq!(title2, "Event Pipeline Architecture Refactoring");

        let text3 = "So basically we need to maintain local SQLite and LanceDB storage while allowing Supabase cloud auth.";
        let title3 = extract_deterministic_title(text3);
        assert!(!title3.to_lowercase().starts_with("so basically"));
        assert!(title3.contains("SQLite") || title3.contains("LanceDB") || title3.contains("storage") || title3.contains("Supabase"));
    }

    #[test]
    fn test_extract_deterministic_knowledge_full_payload() {
        let content = "Yes — this makes a lot of sense, and I would actually do this before fixing Google Calendar integration.\n\
            The important distinction is:\n\
            Google Sign In should not mean Relay is becoming a cloud app.\n\
            It should initially be an identity + product telemetry/update layer, while the user's knowledge remains local.\n\
            That gives you a clean path from local → hybrid without forcing users through a painful migration later.\n\
            I would structure it as 3 modes: 100% Local, Google Account only for telemetry, and Hybrid with encrypted cloud sync.";

        let extracted = extract_deterministic_knowledge(content);

        // 1. Title must be meaningful and non-conversational
        let title = extracted.title.expect("Title must exist");
        assert!(!title.to_lowercase().starts_with("yes — this makes a lot"));
        assert!(title.len() > 5);

        // 2. Topics must have 5-7 relevant domains
        assert!(!extracted.topics.is_empty());
        assert!(extracted.topics.len() <= 7);
        assert!(extracted.topics.iter().any(|t| t.contains("Local") || t.contains("Knowledge") || t.contains("Google Calendar") || t.contains("Identity") || t.contains("Cloud") || t.contains("Hybrid")));

        // 3. Named entities must be extracted
        assert!(!extracted.entities.is_empty());
        assert!(extracted.entities.contains(&"Relay".to_string()));
        assert!(extracted.entities.contains(&"Google Calendar".to_string()) || extracted.entities.contains(&"Google Sign In".to_string()) || extracted.entities.contains(&"Google".to_string()));

        // 4. Questions must be relevant exploration questions
        assert!(!extracted.questions.is_empty());
        assert!(extracted.questions.len() <= 4);
        assert!(extracted.questions.iter().any(|q| q.contains("Relay") || q.contains("local") || q.contains("cloud") || q.contains("Calendar")));
    }

    #[test]
    fn test_enrich_scribble_replaces_derived_metadata() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_enrich_{}", uuid::Uuid::new_v4()));
        let vault = VaultManager::new(temp_dir.clone());
        let llm = LLMClient::new(crate::providers::ProviderConfig::default());

        let mut scribble = Scribble::new_text(
            "Yes — this makes a lot of sense, and I would actually do this before fixing Google Calendar integration. Google Sign In should not mean Relay is becoming a cloud app.",
            None,
        );
        // Pre-populate old topics to verify replacement
        scribble.topics = vec!["OldTopic1".to_string(), "OldTopic2".to_string()];
        scribble.entities = vec!["OldEntity".to_string()];
        vault.save_scribble(&scribble).unwrap();

        // Run enrich_scribble
        let rt = tokio::runtime::Runtime::new().unwrap();
        let enriched = rt.block_on(async {
            enrich_scribble(&llm, &vault, &scribble.id).await.unwrap()
        });

        // Verify title is not "Yes — this makes a lot"
        assert!(!enriched.title.to_lowercase().starts_with("yes — this makes a lot"));
        assert_ne!(enriched.title, "Generating title…");

        // Verify topics replaced, not accumulated
        assert!(!enriched.topics.contains(&"OldTopic1".to_string()));
        assert!(!enriched.topics.contains(&"OldTopic2".to_string()));
        assert!(!enriched.topics.is_empty());
        assert!(enriched.topics.len() <= 7);

        // Verify entities replaced
        assert!(!enriched.entities.contains(&"OldEntity".to_string()));
        assert!(enriched.entities.contains(&"Relay".to_string()) || enriched.entities.contains(&"Google Calendar".to_string()));

        // Verify questions populated
        assert!(!enriched.ai_metadata.suggested_questions.is_empty());
        assert_eq!(enriched.ai_metadata.enrichment_status, "enriched");
        assert!(enriched.ai_metadata.last_enriched_at.is_some());

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn meeting_summarization_without_a_model_fails_visibly_and_keeps_the_transcript() {
        // There is deliberately no fallback that assembles a summary out of
        // transcript sentences: that is the extraction failure this pipeline
        // exists to fix, and shipping it as a "fallback" hides it from the only
        // person who could report it.
        let temp_dir =
            std::env::temp_dir().join(format!("relay_test_meeting_enrich_{}", uuid::Uuid::new_v4()));
        let session_store = crate::meetings_v2::SessionStore::new(temp_dir.clone());
        // No Ollama in the test environment and no cloud key configured.
        let llm = LLMClient::new(crate::providers::ProviderConfig::default());

        let session =
            crate::meetings_v2::MeetingSession::new("meeting_enrich_01".to_string(), None);
        session_store.save_session(&session).unwrap();

        let body = "The placement team shares opportunities but never sends candidate-level outcomes, so nobody can explain why an application stalled. We want checkpoints for application sent, CV shortlisted, interview held, and feedback received. Weekly updates were preferred over monthly because a monthly cadence loses the follow-up entirely. The alumni response sheets mix lakhs and thousands in the salary column and need cleaning before anyone can use them. ";
        for i in 0..4 {
            session_store
                .append_transcript_segment(
                    &session.id,
                    &crate::meetings_v2::TranscriptSegment::new(
                        i,
                        i as f64 * 30.0,
                        (i as f64 + 1.0) * 30.0,
                        body,
                        crate::meetings_v2::TranscriptSegmentStatus::Success,
                    ),
                )
                .unwrap();
        }

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(summarize_meeting(&llm, &session_store, &session.id));

        assert!(
            result.is_err(),
            "an unusable model response must surface as an error, not as copied transcript"
        );

        let reloaded = session_store.get_session(&session.id).unwrap();
        assert!(
            reloaded.summary.is_none(),
            "no invalid summary may be persisted"
        );
        assert!(reloaded.action_items.is_empty());
        assert!(
            !reloaded.title.starts_with("Meeting —"),
            "a placeholder title is still replaced from the fallback ladder: {}",
            reloaded.title
        );
        assert!(
            crate::meetings_v2::validate_title(&reloaded.title, body).is_valid(),
            "the fallback title must satisfy the title rule: {}",
            reloaded.title
        );

        // The transcript itself is untouched.
        let segments = session_store.get_transcript_segments(&session.id).unwrap();
        assert_eq!(segments.len(), 4);
        assert!(segments[0].text.contains("placement team"));

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn an_all_silence_meeting_gets_the_exact_specified_wording() {
        let temp_dir =
            std::env::temp_dir().join(format!("relay_test_meeting_silent_{}", uuid::Uuid::new_v4()));
        let session_store = crate::meetings_v2::SessionStore::new(temp_dir.clone());
        let llm = LLMClient::new(crate::providers::ProviderConfig::default());

        let session = crate::meetings_v2::MeetingSession::new("meeting_silent".to_string(), None);
        session_store.save_session(&session).unwrap();
        session_store
            .append_transcript_segment(
                &session.id,
                &crate::meetings_v2::TranscriptSegment::new(
                    0,
                    0.0,
                    30.0,
                    "[BLANK_AUDIO]",
                    crate::meetings_v2::TranscriptSegmentStatus::Empty,
                ),
            )
            .unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let updated = rt
            .block_on(summarize_meeting(&llm, &session_store, &session.id))
            .expect("an empty transcript is a specified case, not an error");

        assert_eq!(
            updated.summary.as_deref(),
            Some(crate::meetings_v2::validate::INSUFFICIENT_SUMMARY)
        );
        assert!(updated.action_items.is_empty());
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn a_json_object_without_the_expected_fields_is_not_a_valid_response() {
        // The shipped code accepted these: every field has a serde default, so
        // an unrelated JSON object parsed into an empty response and was
        // persisted as a summary-less success.
        assert!(parse_enrichment_response(r#"{"topics": ["a"], "entities": []}"#).is_none());
        assert!(parse_enrichment_response("not json at all").is_none());
        assert!(parse_enrichment_response(r#"{"title": null, "summary": null}"#).is_none());

        let good = parse_enrichment_response(
            "```json\n{\"title\": \"Alumni Placement Tracking\", \"summary\": \"## Overview\", \"action_items\": []}\n```",
        );
        assert_eq!(
            good.and_then(|p| p.title).as_deref(),
            Some("Alumni Placement Tracking"),
            "a fenced response is still a response"
        );
    }

    #[test]
    fn validation_rejects_a_response_that_copies_the_transcript() {
        let transcript = "Our main problem is that when we asked for data from the report, we shared so many opportunities.";
        let copied = MeetingEnrichmentResponse {
            title: Some("Alumni Placement Tracking Gaps".to_string()),
            summary: Some(format!("## Overview\n\n**Purpose:** {}", transcript)),
            action_items: Vec::new(),
        };
        assert!(!validate_enrichment(&copied, transcript).is_valid());
    }

    #[test]
    fn the_fallback_ladder_prefers_honesty_over_a_truncated_guess() {
        // Under 100 intelligible words is rung 5 of the ladder.
        assert_eq!(
            deterministic_title_from_ladder("just a few words here", "26 Aug"),
            "Short Recording — 26 Aug"
        );
    }

    }
