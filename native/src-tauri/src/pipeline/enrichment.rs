use crate::providers::LLMClient;
use crate::vault::{Scribble, ScribbleRelationship, VaultManager, REL_SAME_TOPIC};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AiEnrichmentResponse {
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

/// Asynchronously enriches a Scribble with AI-derived title, structured summary,
/// topics, entities, concept links, and exploration questions.
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
- "title": a short, sharp, meaningful concept title (3 to 8 words). Never return placeholder prefixes like "Synthesis: Generating title…", brackets, or "Untitled". Derive a clean, insightful title describing the core subject matter (e.g. 'Event Pipeline Architecture' or 'Quarterly Revenue Strategy').
- "summary": a structured, short summary (under 75 words total) optimized for rapid reading and visual hierarchy.
  Formatting Rules:
  1. Use structured numbered sections (e.g. "1. **Core Insight:** ..." or "1. **Architecture:**") with sub-bullets indented with 2-4 spaces (e.g. "   - Detailed action or context...").
  2. Use bold lead-ins for key terms and actionable takeaways.
  3. If the thought describes a workflow, state transitions, or system architecture, ALWAYS include a concise 2-4 node Mermaid flowchart wrapped in a ```mermaid code block (e.g. "```mermaid\ngraph LR\nA[Capture] --> B[Enrich] --> C[Graph]\n```").
- "topics": an array of 2-5 high-level domain topics (e.g. ["Knowledge Management", "Architecture", "Audio Processing"])
- "entities": an array of specific named entities (people, technologies, products, organizations, projects)
- "concepts": an array of notable concepts or ideas
- "questions": an array of 2-4 insightful exploration questions that prompt deeper thinking, next steps, or connection opportunities (e.g. ["How can this interface be simplified for mobile?", "What validation tests are required?"])

Return ONLY raw JSON or JSON within a markdown code block.
"#;

    let response = match llm.complete(&scribble.content, Some(system_prompt)).await {
        Ok(res) => res,
        Err(err) => {
            tracing::warn!("AI enrichment failed for scribble {}: {}", scribble_id, err);
            scribble.ai_metadata.enrichment_status = "failed".to_string();
            // If title is stuck on a placeholder, provide a clean deterministic content-derived title
            if scribble.title == "Generating title…"
                || scribble.title.starts_with("Synthesis:")
                || scribble.title.starts_with("[Synthesis:")
                || scribble.title.starts_with("Consolidated:")
            {
                let first_line = scribble.content.lines().next().unwrap_or("Consolidated Thought");
                let clean = first_line.trim_start_matches('#').trim();
                let words: Vec<&str> = clean.split_whitespace().take(6).collect();
                scribble.title = if !words.is_empty() {
                    words.join(" ")
                } else {
                    "Consolidated Thought".to_string()
                };
            }
            let _ = vault.save_scribble(&scribble);
            return Ok(scribble);
        }
    };

    let text = response.text.trim();
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

    let word_count = scribble.content.split_whitespace().count();

    if let Ok(parsed) = serde_json::from_str::<AiEnrichmentResponse>(json_str) {
        if let Some(t) = parsed.title {
            let mut t_clean = t.trim().trim_matches('"').trim_matches('[').trim_matches(']').trim().to_string();
            // Reject any echoed placeholder text from the LLM
            if t_clean.contains("Generating title")
                || t_clean.contains("+ 2 more")
                || t_clean.contains("+ ")
                || t_clean == "Consolidated Thought"
                || t_clean.starts_with("Synthesis: Generating")
                || t_clean.starts_with("Consolidated: Generating")
            {
                let first_clean = scribble
                    .content
                    .lines()
                    .map(|l| l.trim().trim_start_matches('#').trim())
                    .find(|l| {
                        !l.is_empty()
                            && !l.contains("Generating title")
                            && !l.contains("+ ")
                            && !l.starts_with("Synthesis:")
                            && !l.starts_with("Consolidated:")
                            && !l.starts_with("---")
                    });
                if let Some(clean_line) = first_clean {
                    let words: Vec<&str> = clean_line.split_whitespace().take(6).collect();
                    if !words.is_empty() {
                        t_clean = words.join(" ");
                    }
                }
            }
            if !t_clean.is_empty() {
                scribble.title = t_clean;
            }
        }

        // Only save summary if the scribble is >= 100 words in length
        if word_count >= 100 {
            if let Some(s) = parsed.summary {
                let s_clean = s.trim().to_string();
                if !s_clean.is_empty() && s_clean != "null" {
                    scribble.summary = Some(s_clean);
                }
            }
        } else {
            scribble.summary = None;
        }

        // Merge topics uniquely
        for topic in parsed.topics {
            let t = topic.trim().to_string();
            if !t.is_empty() && !scribble.topics.iter().any(|existing| existing.eq_ignore_ascii_case(&t)) {
                scribble.topics.push(t);
            }
        }

        // Merge entities uniquely
        for entity in parsed.entities {
            let e = entity.trim().to_string();
            if !e.is_empty() && !scribble.entities.iter().any(|existing| existing.eq_ignore_ascii_case(&e)) {
                scribble.entities.push(e);
            }
        }

        scribble.ai_metadata.suggested_concepts = parsed.concepts;

        // Ensure questions are populated
        if !parsed.questions.is_empty() {
            scribble.ai_metadata.suggested_questions = parsed.questions;
        } else {
            let topic_name = scribble.topics.first().cloned().unwrap_or_else(|| scribble.title.clone());
            scribble.ai_metadata.suggested_questions = vec![
                format!("How does this concept connect with other active projects and workflows?"),
                format!("What are the key technical or execution risks to monitor for '{}'?", topic_name),
                format!("What concrete next steps or decisions should follow from this thought?"),
            ];
        }

        scribble.ai_metadata.enrichment_status = "enriched".to_string();
        scribble.ai_metadata.last_enriched_at = Some(chrono::Utc::now().to_rfc3339());

        // Find potential relationship candidates from other existing scribbles in the vault
        if let Ok(all_scribbles) = vault.list_scribbles() {
            let existing_targets: std::collections::HashSet<String> =
                scribble.relationships.iter().map(|r| r.target_id.clone()).collect();

            for other in all_scribbles {
                if other.id == scribble.id || existing_targets.contains(&other.id) {
                    continue;
                }

                // Check for shared topics
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
    } else {
        tracing::warn!("Could not parse AI enrichment JSON for scribble {}", scribble_id);
        scribble.ai_metadata.enrichment_status = "failed".to_string();
        let first_clean = scribble
            .content
            .lines()
            .map(|l| l.trim().trim_start_matches('#').trim())
            .find(|l| {
                !l.is_empty()
                    && !l.contains("Generating title")
                    && !l.contains("+ ")
                    && !l.starts_with("Synthesis:")
                    && !l.starts_with("Consolidated:")
                    && !l.starts_with("---")
            });
        if let Some(clean_line) = first_clean {
            let words: Vec<&str> = clean_line.split_whitespace().take(6).collect();
            if !words.is_empty() {
                scribble.title = words.join(" ");
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
struct MeetingAiEnrichmentResponse {
    pub summary: Option<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub action_items: Vec<ExtractedMeetingActionItem>,
    #[serde(default)]
    pub questions: Vec<String>,
    #[serde(default)]
    pub candidate_scribbles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExtractedMeetingActionItem {
    pub title: String,
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
}

/// Asynchronously enriches a Meeting with AI-derived structured summary,
/// key decisions, action items, open questions, and candidate scribbles.
pub async fn enrich_meeting(
    llm: &LLMClient,
    vault: &VaultManager,
    meeting_id: &str,
) -> Result<crate::vault::Meeting, String> {
    let mut meeting = vault
        .get_meeting(meeting_id)
        .map_err(|e| format!("Meeting not found: {}", e))?;

    let combined_content = format!(
        "Meeting Title: {}\n\nNotes:\n{}\n\nTranscript:\n{}",
        meeting.title, meeting.notes, meeting.transcript
    );

    let system_prompt = r#"
You are Relay's Executive Meeting Intelligence Assistant.
Analyze this meeting record (notes and transcript) and derive structured meeting intelligence.

Return ONLY a valid JSON object with the following fields:
- "summary": a crisp executive summary (2-3 structured points with bold lead-ins and indented sub-bullets). If the discussion involves a workflow or process, include a compact 2-4 node Mermaid diagram wrapped in ```mermaid ... ```.
- "decisions": an array of explicit decisions agreed upon during the meeting (e.g. ["Deploy v0.8.3 to staging on Tuesday", "Keep local-only storage as default"]).
- "action_items": an array of tasks with structure:
  [
    {
      "title": "Task description",
      "assignee": "Person's name or null",
      "due_date": "YYYY-MM-DD or null",
      "priority": "high" | "medium" | "low"
    }
  ]
- "questions": an array of 2-4 open, unresolved questions or discussion points.
- "candidate_scribbles": an array of 2-4 atomic, high-value insights or concepts from this meeting that are worth preserving as independent permanent thoughts in a knowledge base.

Return ONLY raw JSON or JSON within a markdown code block.
"#;

    let response = match llm.complete(&combined_content, Some(system_prompt)).await {
        Ok(res) => res,
        Err(err) => {
            tracing::warn!("AI enrichment failed for meeting {}: {}", meeting_id, err);
            return Ok(meeting);
        }
    };

    let text = response.text.trim();
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

    let word_count = combined_content.split_whitespace().count();

    if let Ok(parsed) = serde_json::from_str::<MeetingAiEnrichmentResponse>(json_str) {
        // Summary rule: only save if meeting content is >= 100 words
        if word_count >= 100 {
            if let Some(s) = parsed.summary {
                let s_clean = s.trim().to_string();
                if !s_clean.is_empty() && s_clean != "null" {
                    meeting.summary = Some(s_clean);
                }
            }
        }

        if !parsed.decisions.is_empty() {
            meeting.decisions = parsed.decisions;
        }

        if !parsed.action_items.is_empty() {
            meeting.action_items = parsed
                .action_items
                .into_iter()
                .map(|item| crate::vault::MeetingActionItem {
                    id: format!("act_{}", uuid::Uuid::new_v4()),
                    title: item.title,
                    assignee: item.assignee,
                    due_date: item.due_date,
                    priority: item.priority.unwrap_or_else(|| "medium".to_string()),
                    status: "todo".to_string(),
                })
                .collect();
        }

        if !parsed.questions.is_empty() {
            meeting.questions = parsed.questions;
        }

        if !parsed.candidate_scribbles.is_empty() {
            meeting.candidate_scribbles = parsed.candidate_scribbles;
        }
    } else {
        tracing::warn!("Could not parse meeting AI enrichment JSON for meeting {}", meeting_id);
    }

    meeting.updated_at = chrono::Utc::now().to_rfc3339();
    meeting.status = crate::vault::MEETING_STATUS_COMPLETED.to_string();
    vault
        .save_meeting(&meeting)
        .map_err(|e| format!("Failed to save enriched meeting: {}", e))?;

    Ok(meeting)
}
