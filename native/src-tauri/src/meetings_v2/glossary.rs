//! Per-workspace glossary: the team's product names, projects, people, and
//! acronyms, plus the mishearings they arrive as.
//!
//! Degraded multilingual ASR turns proper nouns into near-homophones —
//! "Aluminium" for alumni, "Nagpur Kul" for NavGurukul. A model asked to guess
//! at those either invents a meaning or drops the point; given the team's own
//! vocabulary it simply reads them correctly. This is the single
//! highest-value input for the transcripts Relay actually sees, which is why
//! `Meeting-rules/meeting_transcript_summary.md` §4.4 requires it on every call.
//!
//! Matching is deliberately conservative. A term that cannot be mapped with
//! confidence is left exactly as it was heard: a wrong correction is invisible
//! to the user and corrupts everything downstream, while an uncorrected term is
//! merely unhelpful.

use rphonetic::DoubleMetaphone;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Where a term came from. Corrections outrank calendar names, which outrank
/// nothing — but all three are equally authoritative once present.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GlossarySource {
    Calendar,
    Correction,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlossaryTerm {
    pub id: String,
    /// The spelling that should appear in output.
    pub canonical: String,
    /// Mishearings confirmed for this term. Matched exactly (case-insensitive)
    /// before any phonetic reasoning is attempted.
    #[serde(default)]
    pub aliases: Vec<String>,
    pub source: GlossarySource,
    pub added_at: String,
}

impl GlossaryTerm {
    pub fn new(canonical: &str, source: GlossarySource) -> Self {
        Self {
            id: format!("term_{}", uuid::Uuid::new_v4()),
            canonical: canonical.trim().to_string(),
            aliases: Vec::new(),
            source,
            added_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn with_aliases(mut self, aliases: &[&str]) -> Self {
        self.aliases = aliases.iter().map(|a| a.trim().to_string()).collect();
        self
    }
}

/// One correction applied to a transcript, for the diagnostics report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlossaryHit {
    pub heard: String,
    pub canonical: String,
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct MatchConfig {
    /// Maximum edit distance between phonetic codes for a match.
    pub max_code_distance: usize,
    /// Minimum Jaro-Winkler similarity between the normalized spellings. Guards
    /// against phonetically close but visibly unrelated words.
    pub min_string_similarity: f64,
    /// Longest candidate phrase, in words. "Nagpur Kul" needs 2; "Pay Forward
    /// Art" needs 3.
    pub max_phrase_words: usize,
}

impl Default for MatchConfig {
    fn default() -> Self {
        Self {
            max_code_distance: 2,
            min_string_similarity: 0.62,
            max_phrase_words: 3,
        }
    }
}

/// Precomputed phonetic keys for one spelling of one term.
struct IndexEntry {
    canonical: String,
    spelling_lower: String,
    primary: String,
    alternate: String,
    word_count: usize,
}

pub struct Glossary {
    terms: Vec<GlossaryTerm>,
    index: Vec<IndexEntry>,
    exact: HashSet<String>,
    config: MatchConfig,
    encoder: DoubleMetaphone,
}

impl Default for Glossary {
    fn default() -> Self {
        Self::new(Vec::new(), MatchConfig::default())
    }
}

impl Glossary {
    pub fn new(terms: Vec<GlossaryTerm>, config: MatchConfig) -> Self {
        // No max code length: truncating to four characters throws away exactly
        // the tail that distinguishes "NavGurukul" from "NavGurus".
        let encoder = DoubleMetaphone::new(None);
        let mut index = Vec::new();
        let mut exact = HashSet::new();

        for term in &terms {
            for spelling in std::iter::once(&term.canonical).chain(term.aliases.iter()) {
                let lower = spelling.to_lowercase();
                exact.insert(lower.clone());
                let key = phonetic_input(spelling);
                if key.is_empty() {
                    continue;
                }
                let codes = encoder.double_metaphone(&key);
                index.push(IndexEntry {
                    canonical: term.canonical.clone(),
                    spelling_lower: lower,
                    primary: codes.primary(),
                    alternate: codes.alternate(),
                    word_count: spelling.split_whitespace().count().max(1),
                });
            }
        }

        Self {
            terms,
            index,
            exact,
            config,
            encoder,
        }
    }

    pub fn from_terms(terms: Vec<GlossaryTerm>) -> Self {
        Self::new(terms, MatchConfig::default())
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn terms(&self) -> &[GlossaryTerm] {
        &self.terms
    }

    /// The canonical spellings, for injection into a model prompt.
    pub fn prompt_terms(&self) -> Vec<String> {
        self.terms.iter().map(|t| t.canonical.clone()).collect()
    }

    /// Longest phrase length worth testing against this glossary.
    fn max_window(&self) -> usize {
        self.index
            .iter()
            .map(|e| e.word_count)
            .max()
            .unwrap_or(1)
            .min(self.config.max_phrase_words)
    }

    /// Rewrites mangled proper nouns in `text` to their canonical spelling.
    ///
    /// Returns the corrected text and one hit per distinct correction.
    pub fn normalize_text(&self, text: &str) -> (String, Vec<GlossaryHit>) {
        if self.index.is_empty() || text.trim().is_empty() {
            return (text.to_string(), Vec::new());
        }

        let tokens: Vec<&str> = text.split_whitespace().collect();
        let max_window = self.max_window();
        let mut out: Vec<String> = Vec::with_capacity(tokens.len());
        let mut hits: Vec<GlossaryHit> = Vec::new();
        let mut i = 0;

        while i < tokens.len() {
            let mut matched = false;

            // Longest phrase first: "Nagpur Kul" must win over "Nagpur".
            for window in (1..=max_window.min(tokens.len() - i)).rev() {
                let phrase_tokens = &tokens[i..i + window];
                let phrase = phrase_tokens.join(" ");
                let (core, trailing) = split_trailing_punctuation(&phrase);
                if core.is_empty() {
                    continue;
                }

                if let Some(canonical) = self.lookup(core) {
                    if !core.eq_ignore_ascii_case(&canonical) {
                        record_hit(&mut hits, core, &canonical);
                    }
                    out.push(format!("{}{}", canonical, trailing));
                    i += window;
                    matched = true;
                    break;
                }
            }

            if !matched {
                out.push(tokens[i].to_string());
                i += 1;
            }
        }

        (out.join(" "), hits)
    }

    /// Resolves one candidate phrase to a canonical term, or `None`.
    pub fn lookup(&self, phrase: &str) -> Option<String> {
        let lower = phrase.to_lowercase();

        // 1. Exact spelling or confirmed alias.
        if self.exact.contains(&lower) {
            return self
                .index
                .iter()
                .find(|e| e.spelling_lower == lower)
                .map(|e| e.canonical.clone());
        }

        if !self.is_candidate(phrase) {
            return None;
        }

        let key = phonetic_input(phrase);
        if key.len() < 3 {
            return None;
        }
        let codes = self.encoder.double_metaphone(&key);
        let candidate_codes = [codes.primary(), codes.alternate()];

        // 2. Phonetic nearest neighbour, with a string-similarity guard.
        let mut matches: Vec<(usize, f64, &str)> = Vec::new();

        for entry in &self.index {
            let mut entry_best: Option<usize> = None;
            for candidate in candidate_codes.iter().filter(|c| !c.is_empty()) {
                for target in [&entry.primary, &entry.alternate] {
                    if target.is_empty() {
                        continue;
                    }
                    // A different first sound is a different word.
                    if candidate.chars().next() != target.chars().next() {
                        continue;
                    }
                    let distance = strsim::levenshtein(candidate, target);
                    if distance <= self.config.max_code_distance
                        && entry_best.map_or(true, |b| distance < b)
                    {
                        entry_best = Some(distance);
                    }
                }
            }

            let Some(distance) = entry_best else { continue };
            let similarity = strsim::jaro_winkler(&lower, &entry.spelling_lower);
            if similarity < self.config.min_string_similarity {
                continue;
            }
            matches.push((distance, similarity, entry.canonical.as_str()));
        }

        let best_distance = matches.iter().map(|(d, _, _)| *d).min()?;
        let contenders: Vec<&(usize, f64, &str)> = matches
            .iter()
            .filter(|(d, _, _)| *d == best_distance)
            .collect();

        // Two different terms fitting the same sound equally well is
        // undecidable by sound, and sound is all we have. Correct to neither.
        let distinct: HashSet<&str> = contenders.iter().map(|(_, _, c)| *c).collect();
        if distinct.len() > 1 {
            return None;
        }

        contenders
            .into_iter()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(_, _, canonical)| canonical.to_string())
    }

    /// Whether a phrase is worth testing at all.
    ///
    /// ASR renders proper nouns capitalized, so requiring a capital keeps
    /// ordinary prose out of the matcher — the difference between correcting
    /// "Corsair" and mangling "course". Every word of a multi-word candidate
    /// must be capitalized, or a phrase window swallows the function word next
    /// to a real term ("NavGurukul and" is close enough to "NavGurukul"
    /// phonetically to match, and correcting it would delete the "and").
    fn is_candidate(&self, phrase: &str) -> bool {
        let mut words = 0;
        for token in phrase.split_whitespace() {
            let core = token.trim_start_matches(|c: char| !c.is_alphanumeric());
            let Some(first) = core.chars().next() else {
                continue;
            };
            if !(first.is_uppercase() || first.is_numeric()) {
                return false;
            }
            words += 1;
        }
        words > 0
    }
}

fn record_hit(hits: &mut Vec<GlossaryHit>, heard: &str, canonical: &str) {
    if let Some(existing) = hits
        .iter_mut()
        .find(|h| h.heard.eq_ignore_ascii_case(heard) && h.canonical == canonical)
    {
        existing.count += 1;
    } else {
        hits.push(GlossaryHit {
            heard: heard.to_string(),
            canonical: canonical.to_string(),
            count: 1,
        });
    }
}

/// Strips punctuation the speaker did not say, keeping it to re-attach after a
/// correction so "Corsair," stays comma-terminated.
fn split_trailing_punctuation(phrase: &str) -> (&str, &str) {
    let end = phrase
        .rfind(|c: char| c.is_alphanumeric())
        .map(|i| i + phrase[i..].chars().next().map_or(1, |c| c.len_utf8()))
        .unwrap_or(0);
    (&phrase[..end], &phrase[end..])
}

/// Double Metaphone input: letters only, so spacing and punctuation in a
/// multi-word term cannot change its code.
fn phonetic_input(phrase: &str) -> String {
    phrase.chars().filter(|c| c.is_alphabetic()).collect()
}

/// Persistence for the per-workspace glossary.
pub struct GlossaryStore {
    path: PathBuf,
}

impl GlossaryStore {
    pub fn new(vault_dir: &Path) -> Self {
        Self {
            path: vault_dir.join("glossary.json"),
        }
    }

    pub fn load_terms(&self) -> Vec<GlossaryTerm> {
        let Ok(raw) = fs::read_to_string(&self.path) else {
            return Vec::new();
        };
        serde_json::from_str(&raw).unwrap_or_else(|e| {
            tracing::warn!("Glossary: ignoring unreadable glossary.json: {}", e);
            Vec::new()
        })
    }

    pub fn load(&self) -> Glossary {
        Glossary::from_terms(self.load_terms())
    }

    pub fn save_terms(&self, terms: &[GlossaryTerm]) -> Result<(), String> {
        let content = serde_json::to_string_pretty(terms)
            .map_err(|e| format!("Failed to serialize glossary: {}", e))?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create vault dir: {}", e))?;
        }
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, content).map_err(|e| format!("Failed to write glossary: {}", e))?;
        fs::rename(&tmp, &self.path).map_err(|e| format!("Failed to commit glossary: {}", e))?;
        Ok(())
    }

    /// Adds a term, or merges into the existing one when the canonical spelling
    /// already exists. Returns the full list.
    pub fn upsert(
        &self,
        canonical: &str,
        aliases: &[String],
        source: GlossarySource,
    ) -> Result<Vec<GlossaryTerm>, String> {
        let canonical = canonical.trim();
        if canonical.is_empty() {
            return Err("A glossary term cannot be empty".to_string());
        }

        let mut terms = self.load_terms();
        match terms
            .iter_mut()
            .find(|t| t.canonical.eq_ignore_ascii_case(canonical))
        {
            Some(existing) => {
                for alias in aliases {
                    let alias = alias.trim();
                    if !alias.is_empty()
                        && !existing
                            .aliases
                            .iter()
                            .any(|a| a.eq_ignore_ascii_case(alias))
                    {
                        existing.aliases.push(alias.to_string());
                    }
                }
            }
            None => {
                let mut term = GlossaryTerm::new(canonical, source);
                term.aliases = aliases
                    .iter()
                    .map(|a| a.trim().to_string())
                    .filter(|a| !a.is_empty())
                    .collect();
                terms.push(term);
            }
        }

        self.save_terms(&terms)?;
        Ok(terms)
    }

    pub fn remove(&self, term_id: &str) -> Result<Vec<GlossaryTerm>, String> {
        let mut terms = self.load_terms();
        terms.retain(|t| t.id != term_id);
        self.save_terms(&terms)?;
        Ok(terms)
    }

    /// Seeds names from a calendar attendee list. Existing terms are untouched.
    pub fn seed_from_attendees(&self, attendees: &[String]) -> Result<Vec<GlossaryTerm>, String> {
        let mut terms = self.load_terms();
        let mut added = false;
        for attendee in attendees {
            let name = attendee.trim();
            if name.is_empty() || name.contains('@') {
                continue;
            }
            if !terms
                .iter()
                .any(|t| t.canonical.eq_ignore_ascii_case(name))
            {
                terms.push(GlossaryTerm::new(name, GlossarySource::Calendar));
                added = true;
            }
        }
        if added {
            self.save_terms(&terms)?;
        }
        Ok(terms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The team vocabulary behind the fixtures in `Meeting-rules/`.
    fn fixture_glossary() -> Glossary {
        Glossary::from_terms(vec![
            GlossaryTerm::new("alumni", GlossarySource::Manual),
            GlossaryTerm::new("Coursera", GlossarySource::Manual),
            GlossaryTerm::new("Pay Forward", GlossarySource::Manual),
            GlossaryTerm::new("NavGurukul", GlossarySource::Manual),
        ])
    }

    #[test]
    fn maps_the_mishearings_from_the_rules_files() {
        // The four cases in meeting_transcript_summary.md §4.4.
        let g = fixture_glossary();
        assert_eq!(g.lookup("Aluminium").as_deref(), Some("alumni"));
        assert_eq!(g.lookup("Corsair").as_deref(), Some("Coursera"));
        assert_eq!(g.lookup("PayFour Art").as_deref(), Some("Pay Forward"));
        assert_eq!(g.lookup("Nagpur Kul").as_deref(), Some("NavGurukul"));
    }

    #[test]
    fn leaves_a_term_with_no_near_match_untouched() {
        // "Poga" and "Goki" are listed as unmappable: inventing a meaning for
        // them is worse than dropping the point.
        let g = fixture_glossary();
        assert_eq!(g.lookup("Poga"), None);
        assert_eq!(g.lookup("Goki"), None);
        assert_eq!(g.lookup("Bangalore"), None);
    }

    #[test]
    fn does_not_rewrite_ordinary_prose() {
        let g = fixture_glossary();
        let (out, hits) = g.normalize_text(
            "the course was fine and we paid forward the payment to the alumni",
        );
        assert_eq!(
            out, "the course was fine and we paid forward the payment to the alumni",
            "lowercase prose must never be phonetically rewritten"
        );
        assert!(hits.is_empty());
    }

    #[test]
    fn corrects_in_place_and_keeps_punctuation() {
        let g = fixture_glossary();
        let (out, hits) = g.normalize_text(
            "We shared the Aluminium data with Corsair, then Nagpur Kul reviewed it.",
        );
        assert!(out.contains("alumni data"), "got: {out}");
        assert!(out.contains("Coursera,"), "trailing comma must survive: {out}");
        assert!(out.contains("NavGurukul reviewed"), "got: {out}");
        assert_eq!(hits.len(), 3);
    }

    #[test]
    fn counts_repeated_corrections_once_with_a_tally() {
        let g = fixture_glossary();
        let (_, hits) = g.normalize_text("Corsair said Corsair would email Corsair.");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].count, 3);
        assert_eq!(hits[0].canonical, "Coursera");
    }

    #[test]
    fn a_confirmed_alias_matches_exactly_whatever_it_sounds_like() {
        let g = Glossary::from_terms(vec![
            GlossaryTerm::new("Pay Forward", GlossarySource::Manual).with_aliases(&["PayFour Art"]),
        ]);
        assert_eq!(g.lookup("payfour art").as_deref(), Some("Pay Forward"));
    }

    #[test]
    fn a_canonical_term_is_returned_unchanged() {
        let g = fixture_glossary();
        let (out, hits) = g.normalize_text("NavGurukul and Coursera are partners");
        assert_eq!(out, "NavGurukul and Coursera are partners");
        assert!(hits.is_empty(), "an already-correct term is not a correction");
    }

    #[test]
    fn ambiguity_between_two_terms_produces_no_correction() {
        // Two canonical spellings of the same sound: correcting to either would
        // be a coin flip.
        let g = Glossary::from_terms(vec![
            GlossaryTerm::new("Marc", GlossarySource::Calendar),
            GlossaryTerm::new("Mark", GlossarySource::Calendar),
        ]);
        assert_eq!(g.lookup("Marck"), None);
    }

    #[test]
    fn an_empty_glossary_is_a_no_op() {
        let g = Glossary::default();
        let text = "Aluminium Corsair Nagpur Kul";
        let (out, hits) = g.normalize_text(text);
        assert_eq!(out, text);
        assert!(hits.is_empty());
    }

    #[test]
    fn store_round_trips_and_merges_aliases() {
        let dir = std::env::temp_dir().join(format!("relay_gloss_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let store = GlossaryStore::new(&dir);

        assert!(store.load_terms().is_empty());
        store
            .upsert("NavGurukul", &["Nagpur Kul".to_string()], GlossarySource::Manual)
            .unwrap();
        store
            .upsert("NavGurukul", &["NGB".to_string()], GlossarySource::Correction)
            .unwrap();

        let terms = store.load_terms();
        assert_eq!(terms.len(), 1, "the same canonical must merge, not duplicate");
        assert_eq!(terms[0].aliases.len(), 2);

        let id = terms[0].id.clone();
        assert!(store.remove(&id).unwrap().is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn attendee_seeding_skips_email_addresses_and_existing_terms() {
        let dir = std::env::temp_dir().join(format!("relay_gloss_seed_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let store = GlossaryStore::new(&dir);

        store
            .upsert("Pranjal", &[], GlossarySource::Manual)
            .unwrap();
        let terms = store
            .seed_from_attendees(&[
                "Pranjal".to_string(),
                "Nitin Sudarshan".to_string(),
                "someone@example.com".to_string(),
                "  ".to_string(),
            ])
            .unwrap();

        let names: Vec<&str> = terms.iter().map(|t| t.canonical.as_str()).collect();
        assert_eq!(names, vec!["Pranjal", "Nitin Sudarshan"]);
        let _ = fs::remove_dir_all(dir);
    }
}
