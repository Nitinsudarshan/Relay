//! Memory Formation and Conflict Resolution Policy Engine.
//!
//! Enforces deliberate memory formation: distinguishes captured evidence from durable memory,
//! checks eligibility, detects semantic conflicts on the same subject, and maintains explicit
//! supersedes chains with epistemic states.

use serde::{Deserialize, Serialize};

#[allow(unused_imports)]
use super::model::{EpistemicState, MemoryItem, MemoryProvenance, MemoryStatus, MemoryType};
use super::store::MemoryStore;

/// Candidate memory proposed for durable retention.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateMemory {
    pub memory_type: MemoryType,
    pub subject: String,
    pub content: String,
    pub evidence: String,
    pub source_id: String,
    pub confidence: f32,
    pub reason_for_retention: String,
}

/// Action taken by the formation policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormationAction {
    Created,
    Superseded,
    Deduplicated,
    Rejected,
}

/// Outcome of evaluating a memory candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryFormationOutcome {
    pub action: FormationAction,
    pub memory: Option<MemoryItem>,
    pub superseded_memory_id: Option<String>,
    pub reason: String,
}

pub struct MemoryFormationService;

impl MemoryFormationService {
    /// Evaluates a candidate memory against formation eligibility and conflict policies.
    pub fn process_candidate(
        store: &MemoryStore,
        candidate: CandidateMemory,
    ) -> Result<MemoryFormationOutcome, String> {
        // 1. Eligibility Check
        if candidate.subject.trim().is_empty() {
            return Ok(MemoryFormationOutcome {
                action: FormationAction::Rejected,
                memory: None,
                superseded_memory_id: None,
                reason: "Rejected: Candidate subject cannot be empty".to_string(),
            });
        }
        if candidate.content.trim().is_empty() {
            return Ok(MemoryFormationOutcome {
                action: FormationAction::Rejected,
                memory: None,
                superseded_memory_id: None,
                reason: "Rejected: Candidate content cannot be empty".to_string(),
            });
        }
        if candidate.evidence.trim().is_empty() {
            return Ok(MemoryFormationOutcome {
                action: FormationAction::Rejected,
                memory: None,
                superseded_memory_id: None,
                reason: "Rejected: Durable memory requires grounding evidence".to_string(),
            });
        }
        if candidate.confidence < 0.60 {
            return Ok(MemoryFormationOutcome {
                action: FormationAction::Rejected,
                memory: None,
                superseded_memory_id: None,
                reason: format!(
                    "Rejected: Confidence {} is below retention threshold 0.60",
                    candidate.confidence
                ),
            });
        }

        // 2. Conflict & Existing State Detection
        let active_memories = store.list_active(Some(candidate.memory_type));
        let norm_sub = candidate.subject.trim().to_lowercase();
        let existing_on_subject = active_memories
            .into_iter()
            .find(|m| m.subject.trim().to_lowercase() == norm_sub);

        let prov = MemoryProvenance {
            source_id: candidate.source_id.clone(),
            source_type: "candidate_formation".to_string(),
            evidence: candidate.evidence.clone(),
            confidence: candidate.confidence,
            extracted_by: "MemoryFormationService".to_string(),
        };

        if let Some(existing) = existing_on_subject {
            // Case A: Exact content match -> Deduplicate idempotently
            if existing.content.trim() == candidate.content.trim() {
                return Ok(MemoryFormationOutcome {
                    action: FormationAction::Deduplicated,
                    memory: Some(existing),
                    superseded_memory_id: None,
                    reason: "Deduplicated: Identical active memory already recorded".to_string(),
                });
            }

            // Case B: Contradictory / Updated content -> Supersede
            let (old_item, new_item) = store.supersede_memory(
                &existing.id,
                &candidate.content,
                prov,
            )?;

            Ok(MemoryFormationOutcome {
                action: FormationAction::Superseded,
                memory: Some(new_item),
                superseded_memory_id: Some(old_item.id),
                reason: format!(
                    "Superseded: Memory '{}' updated with new durable evidence",
                    candidate.subject
                ),
            })
        } else {
            // Case C: New subject -> Create active memory
            let mut new_item = MemoryItem::new(
                candidate.memory_type,
                candidate.subject.clone(),
                candidate.content.clone(),
                prov,
            );
            new_item.metadata = Some(serde_json::json!({
                "reason_for_retention": candidate.reason_for_retention,
            }));

            let stored = store.create_memory(new_item)?;
            Ok(MemoryFormationOutcome {
                action: FormationAction::Created,
                memory: Some(stored),
                superseded_memory_id: None,
                reason: format!(
                    "Created: Established new durable memory for '{}'",
                    candidate.subject
                ),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::model::{EpistemicState, MemoryStatus};

    #[test]
    fn test_memory_formation_and_conflict_superseding() {
        let temp_dir = std::env::temp_dir().join(format!("relay_mem_form_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let store = MemoryStore::new(&temp_dir);

        // Candidate 1: User preference A
        let c1 = CandidateMemory {
            memory_type: MemoryType::Preference,
            subject: "Report Formatting".to_string(),
            content: "User prefers detailed responses with bullet points.".to_string(),
            evidence: "Please give me detailed bullet points in reports.".to_string(),
            source_id: "conv_1".to_string(),
            confidence: 1.0,
            reason_for_retention: "Explicit user preference".to_string(),
        };

        let out1 = MemoryFormationService::process_candidate(&store, c1).unwrap();
        assert_eq!(out1.action, FormationAction::Created);
        let mem1 = out1.memory.unwrap();
        assert_eq!(mem1.status, MemoryStatus::Active);

        // Candidate 2: Contradictory user preference B
        let c2 = CandidateMemory {
            memory_type: MemoryType::Preference,
            subject: "Report Formatting".to_string(),
            content: "User prefers concise executive summaries without bullets.".to_string(),
            evidence: "Keep all future reports concise and executive style.".to_string(),
            source_id: "conv_2".to_string(),
            confidence: 1.0,
            reason_for_retention: "Updated user instruction".to_string(),
        };

        let out2 = MemoryFormationService::process_candidate(&store, c2).unwrap();
        assert_eq!(out2.action, FormationAction::Superseded);
        let mem2 = out2.memory.unwrap();
        assert_eq!(mem2.status, MemoryStatus::Active);
        assert_eq!(mem2.supersedes_id, Some(mem1.id.clone()));

        // Check that old memory is marked superseded and no longer in list_active
        let active = store.list_active(None);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].content, "User prefers concise executive summaries without bullets.");

        let old = store.get_memory(&mem1.id).unwrap();
        assert_eq!(old.status, MemoryStatus::Superseded);
        assert_eq!(old.epistemic_state, EpistemicState::NoLongerCurrent);
        assert_eq!(old.superseded_by, Some(mem2.id));

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
