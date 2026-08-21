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
    #[serde(default)]
    pub questions: Vec<String>,
}

/// Asynchronously enriches a Scribble with AI-derived title, summary,
/// topics, entities, and concept links without blocking initial creation.
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
Analyze this thought/scribble and derive high-quality structured knowledge metadata.

Return ONLY a valid JSON object with the following fields:
- "title": a short, meaningful, human-readable title (3 to 8 words). Do NOT simply truncate the first words of the sentence. Derive a concise concept title (e.g. 'RAG Model for Educational Content' or 'Healthcare Report Monitoring').
- "summary": a 1-2 sentence core distillation
- "topics": an array of 2-5 high-level domain topics (e.g. ["Knowledge Management", "Android", "Audio Processing"])
- "entities": an array of specific named entities (people, technologies, products, organizations, projects)
- "concepts": an array of notable concepts or ideas
- "questions": an array of 1-3 open questions or exploration directions raised by this thought

Return ONLY raw JSON or JSON within a markdown code block.
"#;

    let response = match llm.complete(&scribble.content, Some(system_prompt)).await {
        Ok(res) => res,
        Err(err) => {
            tracing::warn!("AI enrichment failed for scribble {}: {}", scribble_id, err);
            scribble.ai_metadata.enrichment_status = "failed".to_string();
            // If title is stuck on a placeholder, provide a clean deterministic content-derived title
            if scribble.title == "Generating title…" || scribble.title.starts_with("Synthesis:") {
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

    if let Ok(parsed) = serde_json::from_str::<AiEnrichmentResponse>(json_str) {
        if let Some(t) = parsed.title {
            let t_clean = t.trim().to_string();
            let is_placeholder = scribble.title == "Generating title…"
                || scribble.title.starts_with("Untitled")
                || scribble.title.starts_with("Merged:")
                || scribble.title.starts_with("Synthesis:")
                || scribble.source_metadata.get("creation_method").and_then(|v| v.as_str()) == Some("merge")
                || scribble.title.len() < 8;

            if !t_clean.is_empty() && is_placeholder {
                scribble.title = t_clean;
            }
        }

        if let Some(s) = parsed.summary {
            if !s.trim().is_empty() {
                scribble.summary = Some(s.trim().to_string());
            }
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
        scribble.ai_metadata.suggested_questions = parsed.questions;
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
        if scribble.title == "Generating title…" || scribble.title.starts_with("Synthesis:") {
            let first_line = scribble.content.lines().next().unwrap_or("Consolidated Thought");
            let clean = first_line.trim_start_matches('#').trim();
            let words: Vec<&str> = clean.split_whitespace().take(6).collect();
            scribble.title = if !words.is_empty() {
                words.join(" ")
            } else {
                "Consolidated Thought".to_string()
            };
        }
    }

    scribble.updated_at = chrono::Utc::now().to_rfc3339();
    vault
        .save_scribble(&scribble)
        .map_err(|e| format!("Failed to save enriched scribble: {}", e))?;

    Ok(scribble)
}
