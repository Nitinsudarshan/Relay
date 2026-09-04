//! Conservative Entity Resolution.
//!
//! Reconciles aliases, identifiers, and URLs into canonical entities without
//! aggressive over-merging.

use super::model::{slugify, EntityCategory, EntityMention, ExtractedEntity, ResolvedEntity};

/// Normalizes a string for conservative equivalence checking:
/// lowercases, trims, and strips spaces, hyphens, and underscores.
pub fn normalize_for_matching(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Normalizes a URL or repository identifier into a clean identifier slug.
pub fn extract_repo_identifier(s: &str) -> Option<String> {
    let trimmed = s.trim().trim_end_matches('/');
    if let Some(pos) = trimmed.find("github.com/") {
        let repo_part = &trimmed[pos + "github.com/".len()..];
        let parts: Vec<&str> = repo_part.split('/').collect();
        if parts.len() >= 2 {
            return Some(format!("{}/{}", parts[0].to_ascii_lowercase(), parts[1].to_ascii_lowercase()));
        }
    }

    if trimmed.contains('/') && !trimmed.contains("://") {
        let parts: Vec<&str> = trimmed.split('/').collect();
        if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Some(format!("{}/{}", parts[0].to_ascii_lowercase(), parts[1].to_ascii_lowercase()));
        }
    }

    None
}

pub struct EntityResolver;

impl EntityResolver {
    /// Conservatively resolves a batch of extracted entities into canonical resolved entities.
    pub fn resolve(extracted: &[ExtractedEntity]) -> Vec<ResolvedEntity> {
        let mut resolved: Vec<ResolvedEntity> = Vec::new();

        for ent in extracted {
            let mention = EntityMention {
                source_id: ent.source_id.clone(),
                evidence: ent.evidence.clone(),
                confidence: ent.confidence,
                timestamp: None,
            };

            let repo_id = if ent.category == EntityCategory::Url || ent.category == EntityCategory::Project {
                extract_repo_identifier(&ent.name)
            } else {
                None
            };

            let norm_name = normalize_for_matching(&ent.name);

            // Look for matching resolved candidate
            let mut match_idx = None;
            for (idx, candidate) in resolved.iter().enumerate() {
                // Rule 1: Never merge across different non-interchangeable categories.
                // Exception: Url/Identifier matching a Project/Product repo_id
                let categories_compatible = match (ent.category, candidate.category) {
                    (a, b) if a == b => true,
                    (EntityCategory::Url, EntityCategory::Project)
                    | (EntityCategory::Project, EntityCategory::Url)
                    | (EntityCategory::Url, EntityCategory::Product)
                    | (EntityCategory::Product, EntityCategory::Url) => true,
                    _ => false,
                };

                if !categories_compatible {
                    continue;
                }

                // Match condition A: Matching repo identifier (e.g. stablyai/orca)
                if let Some(ref rid) = repo_id {
                    if candidate.source_identifiers.iter().any(|id| id == rid) {
                        match_idx = Some(idx);
                        break;
                    }
                    // Match short name with repo name (e.g. "Orca" matches "stablyai/orca")
                    if let Some(repo_name) = rid.split('/').nth(1) {
                        if normalize_for_matching(repo_name) == normalize_for_matching(&candidate.canonical_name) {
                            match_idx = Some(idx);
                            break;
                        }
                    }
                }

                // Match condition B: Exact normalized name match within same category (e.g. "Claude Code" == "ClaudeCode")
                if ent.category == candidate.category && norm_name == normalize_for_matching(&candidate.canonical_name) {
                    match_idx = Some(idx);
                    break;
                }

                // Match condition C: Existing alias match
                if candidate.aliases.iter().any(|a| normalize_for_matching(a) == norm_name) {
                    match_idx = Some(idx);
                    break;
                }
            }

            if let Some(idx) = match_idx {
                // Merge into existing resolved entity
                let candidate = &mut resolved[idx];
                if !candidate.aliases.contains(&ent.name) && candidate.canonical_name != ent.name {
                    candidate.aliases.push(ent.name.clone());
                }
                if let Some(rid) = repo_id {
                    if !candidate.source_identifiers.contains(&rid) {
                        candidate.source_identifiers.push(rid);
                    }
                }
                if ent.category == EntityCategory::Url && !candidate.urls.contains(&ent.name) {
                    candidate.urls.push(ent.name.clone());
                }
                candidate.mentions.push(mention);
            } else {
                // Create new resolved entity
                let aliases = Vec::new();
                let mut source_identifiers = Vec::new();
                let mut urls = Vec::new();

                if let Some(ref rid) = repo_id {
                    source_identifiers.push(rid.clone());
                }
                if ent.category == EntityCategory::Url {
                    urls.push(ent.name.clone());
                }

                let id = format!("resolved_{}_{}", ent.category.as_str(), slugify(&ent.name));
                resolved.push(ResolvedEntity {
                    id,
                    canonical_name: ent.name.clone(),
                    category: ent.category,
                    aliases,
                    source_identifiers,
                    urls,
                    confidence: ent.confidence,
                    mentions: vec![mention],
                });
            }
        }

        resolved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_project_and_url_aliases() {
        let mentions = vec![
            ExtractedEntity::new("Orca", EntityCategory::Project, "src_1", "Project Orca"),
            ExtractedEntity::new("stablyai/orca", EntityCategory::Project, "src_2", "repo stablyai/orca"),
            ExtractedEntity::new("https://github.com/stablyai/orca", EntityCategory::Url, "src_3", "view on https://github.com/stablyai/orca"),
        ];

        let resolved = EntityResolver::resolve(&mentions);
        // All 3 reference the same repo/project and should merge into one
        assert_eq!(resolved.len(), 1);
        let orca = &resolved[0];
        assert_eq!(orca.canonical_name, "Orca");
        assert_eq!(orca.mentions.len(), 3);
        assert!(orca.source_identifiers.contains(&"stablyai/orca".to_string()));
        assert!(orca.urls.contains(&"https://github.com/stablyai/orca".to_string()));
    }

    #[test]
    fn test_resolve_claude_code_variants() {
        let mentions = vec![
            ExtractedEntity::new("Claude Code", EntityCategory::Product, "src_1", "Built with Claude Code"),
            ExtractedEntity::new("ClaudeCode", EntityCategory::Product, "src_2", "Using ClaudeCode here"),
        ];

        let resolved = EntityResolver::resolve(&mentions);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].canonical_name, "Claude Code");
        assert_eq!(resolved[0].mentions.len(), 2);
    }

    #[test]
    fn test_do_not_merge_ambiguous_distinct_entities() {
        let mentions = vec![
            ExtractedEntity::new("Git", EntityCategory::Technology, "src_1", "using git"),
            ExtractedEntity::new("GitHub", EntityCategory::Organization, "src_2", "on github"),
        ];

        let resolved = EntityResolver::resolve(&mentions);
        // Git and GitHub are different categories and distinct concepts; must stay separate!
        assert_eq!(resolved.len(), 2);
    }
}
