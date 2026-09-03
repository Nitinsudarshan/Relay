//! The unified context retriever.
//!
//! One canonical way to ask "what does Relay already know about this?",
//! across every capture surface. Talkback deliberately owns no storage of
//! its own (`docs/talkback/ARCHITECTURE.md` §12), so this module is the
//! only place that decides *which* of the user's own words reach a model.
//!
//! Scoring is split from gathering on purpose: [`rank`] is a pure function
//! over [`CandidateDoc`]s, so retrieval quality is unit-testable without a
//! vault, a meeting store, or a filesystem. `sources.rs` does the I/O.
//!
//! Retrieval today is lexical. Relay has no embedding pipeline — the
//! LanceDB committed to in `docs/decisions.md` Decision 6 was never built
//! (see `docs/talkback/RESEARCH.md` §E.1) — and pretending otherwise would
//! be worse than saying so. [`score_candidate`] is the single seam a
//! hybrid lexical+embedding score slots into later.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Where a piece of retrieved context came from.
///
/// This is the provenance the user can ask about ("where did you get
/// that?"), so it is carried end-to-end rather than flattened into a
/// title string the way `pipeline::chat` used to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SourceType {
    VoiceNote,
    Scribble,
    Meeting,
    MeetingFacts,
    File,
    /// A web page or conversation captured from the browser.
    Capture,
}

impl SourceType {
    /// Every source Talkback can retrieve from, in the order they are
    /// reported. Adding a variant here is what makes it searchable.
    pub const ALL: [SourceType; 6] = [
        SourceType::MeetingFacts,
        SourceType::Scribble,
        SourceType::Capture,
        SourceType::Meeting,
        SourceType::File,
        SourceType::VoiceNote,
    ];

    /// Whether this source is material Relay captured from outside, rather
    /// than something the user wrote, said or imported.
    ///
    /// The distinction that matters downstream: everything else in the vault
    /// is the user's own record and can be spoken back to them as theirs. A
    /// capture is a record of what a *website* said, and Talkback has to be
    /// able to tell the difference — both to attribute it honestly and to
    /// avoid treating a page's text as an instruction. See
    /// `pipeline::source_boundary`.
    pub fn is_external(self) -> bool {
        matches!(self, SourceType::Capture)
    }

    pub fn label(self) -> &'static str {
        match self {
            SourceType::VoiceNote => "Voice Note",
            SourceType::Scribble => "Scribble",
            SourceType::Meeting => "Meeting",
            SourceType::MeetingFacts => "Meeting Intelligence",
            SourceType::File => "Imported File",
            SourceType::Capture => "Web Capture",
        }
    }

    /// How much this source's material is worth relative to the others.
    ///
    /// Derived intelligence outranks raw capture — the lesson from
    /// Granola/Notion in `RESEARCH.md` §A, and the reason `MeetingFacts`
    /// exists in the meetings pipeline at all. Voice Notes sit lowest not
    /// because they matter least but because they are verbatim dictation:
    /// high recall, low signal density per character of context budget.
    pub fn weight(self) -> f32 {
        match self {
            SourceType::MeetingFacts => 1.25,
            SourceType::Scribble => 1.10,
            // Captured pages and conversations are acquired source material,
            // like an imported document — worth the same as one, and worth
            // more than verbatim dictation.
            SourceType::Capture => 1.05,
            SourceType::File => 1.05,
            SourceType::Meeting => 1.00,
            SourceType::VoiceNote => 0.95,
        }
    }
}

/// A document projected into the one shape the ranker understands.
///
/// Voice Notes, Scribbles, meeting summaries and MeetingFacts rows all
/// collapse into this so the scoring logic never grows a per-source
/// branch.
#[derive(Debug, Clone, PartialEq)]
pub struct CandidateDoc {
    pub source_type: SourceType,
    pub source_id: String,
    pub title: String,
    pub body: String,
    /// RFC3339. Empty when the underlying record has no usable timestamp,
    /// which disables recency weighting for that document rather than
    /// guessing a date.
    pub timestamp: String,
    pub topics: Vec<String>,
    pub entities: Vec<String>,
    /// Ids this document links to, used for the single-hop expansion.
    pub related_ids: Vec<String>,
    /// Human-readable qualifier shown in the source chip — "decision",
    /// "action item", "summary". Not searched.
    pub detail: Option<String>,
}

impl CandidateDoc {
    pub fn new(source_type: SourceType, source_id: &str, title: &str, body: &str) -> Self {
        Self {
            source_type,
            source_id: source_id.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            timestamp: String::new(),
            topics: Vec::new(),
            entities: Vec::new(),
            related_ids: Vec::new(),
            detail: None,
        }
    }

    pub fn with_timestamp(mut self, timestamp: &str) -> Self {
        self.timestamp = timestamp.to_string();
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_topics(mut self, topics: Vec<String>) -> Self {
        self.topics = topics;
        self
    }

    pub fn with_entities(mut self, entities: Vec<String>) -> Self {
        self.entities = entities;
        self
    }

    pub fn with_related(mut self, related_ids: Vec<String>) -> Self {
        self.related_ids = related_ids;
        self
    }
}

/// One retrieved piece of context, with the provenance intact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextItem {
    pub source_type: SourceType,
    pub source_id: String,
    pub title: String,
    pub timestamp: String,
    pub relevance: f32,
    /// The text actually sent to the model, already trimmed to the budget.
    pub excerpt: String,
    #[serde(default)]
    pub detail: Option<String>,
    /// True when this arrived via relationship/topic expansion rather than
    /// matching the query itself. Surfaced so a chip can say "related".
    #[serde(default)]
    pub expanded: bool,
}

/// What to retrieve, and how much of it.
#[derive(Debug, Clone, PartialEq)]
pub struct RetrievalQuery {
    pub text: String,
    pub source_types: Vec<SourceType>,
    pub max_results: usize,
    /// Total characters of excerpt the assembler is willing to spend.
    /// Derived from the provider's context window, never a fixed constant.
    pub char_budget: usize,
    /// Lower bound on `timestamp`, RFC3339. Set for temporal questions
    /// ("what did I say last week") where similarity alone retrieves the
    /// wrong month confidently.
    pub since: Option<String>,
}

impl RetrievalQuery {
    /// A general-purpose query over every source.
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            source_types: SourceType::ALL.to_vec(),
            max_results: DEFAULT_MAX_RESULTS,
            char_budget: DEFAULT_CHAR_BUDGET,
            since: None,
        }
    }

    pub fn with_sources(mut self, source_types: Vec<SourceType>) -> Self {
        self.source_types = source_types;
        self
    }

    pub fn with_max_results(mut self, max_results: usize) -> Self {
        self.max_results = max_results;
        self
    }

    pub fn with_char_budget(mut self, char_budget: usize) -> Self {
        self.char_budget = char_budget;
        self
    }

    pub fn with_since(mut self, since: Option<String>) -> Self {
        self.since = since;
        self
    }
}

/// Enough for a grounded answer without spending the whole window on
/// context — a spoken answer is short, so the model does not need twenty
/// documents to produce three sentences.
pub const DEFAULT_MAX_RESULTS: usize = 6;

/// Conservative default for an 8k-token local model, which is what
/// `ProviderConfig::default` actually runs. The engine overrides this from
/// the configured `context_tokens`.
pub const DEFAULT_CHAR_BUDGET: usize = 6_000;

/// Shortest excerpt worth including — below this a document contributes a
/// title and no usable content, which reads to a model as noise.
const MIN_EXCERPT_CHARS: usize = 80;

/// Score multiplier applied to documents pulled in by expansion rather
/// than by matching the query. Deliberately harsh: expansion exists to
/// add the one linked thought the user meant, not to flood the budget.
const EXPANSION_DISCOUNT: f32 = 0.45;

/// The result of one retrieval, including what was searched — an empty
/// `items` with a populated `searched_sources` is how the engine tells
/// "nothing matched" apart from "nothing was looked at".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalResult {
    pub items: Vec<ContextItem>,
    pub searched_sources: Vec<SourceType>,
    pub total_candidates: usize,
}

impl RetrievalResult {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Words carrying no retrieval signal. Kept deliberately small: an
/// aggressive stoplist strips the distinguishing word out of short spoken
/// questions ("what did we decide about *cost*").
const STOPWORDS: &[&str] = &[
    "a", "about", "all", "am", "an", "and", "any", "are", "as", "at", "be", "been", "but", "by",
    "can", "could", "did", "do", "does", "for", "from", "get", "had", "has", "have", "he", "her",
    "him", "his", "how", "i", "if", "in", "into", "is", "it", "its", "just", "me", "my", "of",
    "on", "or", "our", "out", "over", "she", "should", "so", "some", "than", "that", "the",
    "their", "them", "then", "there", "these", "they", "this", "to", "was", "we", "were", "what",
    "when", "where", "which", "who", "why", "will", "with", "would", "you", "your",
];

/// Splits text into lowercase alphanumeric terms, dropping stopwords and
/// single characters.
///
/// Deliberately not shared with `vault::tokenize`: that one backs the
/// existing knowledge-search UI and changing it would change search
/// results across surfaces this feature has no business touching.
pub fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() > 1)
        .map(|t| t.to_lowercase())
        .filter(|t| !STOPWORDS.contains(&t.as_str()))
        .collect()
}

/// Inverse-document-frequency weights for the query terms.
///
/// Without this, "meeting" — a word in almost every document — outscores
/// the one rare term that actually identifies what the user meant.
fn idf_weights(query_terms: &[String], docs: &[CandidateDoc]) -> HashMap<String, f32> {
    let n = docs.len().max(1) as f32;
    let mut weights = HashMap::new();
    for term in query_terms {
        if weights.contains_key(term) {
            continue;
        }
        let df = docs
            .iter()
            .filter(|d| {
                let haystack = format!("{} {}", d.title, d.body).to_lowercase();
                haystack.contains(term.as_str())
            })
            .count() as f32;
        // +1 smoothing keeps a term present in every document at a small
        // positive weight rather than zero, so an all-common-terms query
        // still ranks by frequency instead of collapsing to no signal.
        weights.insert(term.clone(), ((n + 1.0) / (df + 1.0)).ln().max(0.05));
    }
    weights
}

/// Age in days between an RFC3339 timestamp and `now`, or `None` when the
/// timestamp is missing or unparseable.
fn age_days(timestamp: &str, now: chrono::DateTime<chrono::Utc>) -> Option<f32> {
    let parsed = chrono::DateTime::parse_from_rfc3339(timestamp).ok()?;
    let delta = now.signed_duration_since(parsed.with_timezone(&chrono::Utc));
    Some((delta.num_seconds() as f32 / 86_400.0).max(0.0))
}

/// Gentle recency preference: recent material wins ties, old material is
/// not buried.
///
/// Halving every ~90 days and floored at 0.6 — a decision from last year
/// is still the answer to "what did we decide", so recency must never
/// dominate relevance. A document with no timestamp scores neutral.
fn recency_multiplier(timestamp: &str, now: chrono::DateTime<chrono::Utc>) -> f32 {
    match age_days(timestamp, now) {
        Some(days) => (0.5_f32.powf(days / 90.0)).clamp(0.6, 1.0),
        None => 0.85,
    }
}

/// Scores one document against the normalized query.
///
/// **This function is the seam for semantic retrieval.** A hybrid score
/// (`alpha * lexical + beta * cosine(embedding)`) replaces the body here
/// without any other module changing. See `RESEARCH.md` §B for the
/// candidates evaluated (`fastembed`, Ollama `/api/embed`) and why neither
/// ships in V1.
pub fn score_candidate(
    doc: &CandidateDoc,
    query_terms: &[String],
    query_phrase: &str,
    idf: &HashMap<String, f32>,
    now: chrono::DateTime<chrono::Utc>,
) -> f32 {
    if query_terms.is_empty() {
        return 0.0;
    }

    let title_lower = doc.title.to_lowercase();
    let body_lower = doc.body.to_lowercase();
    let tag_lower = doc
        .topics
        .iter()
        .chain(doc.entities.iter())
        .map(|t| t.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");

    let mut lexical = 0.0_f32;
    let mut matched_terms = 0_usize;

    for term in query_terms {
        let weight = idf.get(term).copied().unwrap_or(1.0);
        let in_title = title_lower.contains(term.as_str());
        let in_tags = tag_lower.contains(term.as_str());
        // Occurrence count, damped by sqrt so one long rambling note
        // cannot outrank a short precise one purely on length.
        let body_hits = body_lower.matches(term.as_str()).count();

        if !in_title && !in_tags && body_hits == 0 {
            continue;
        }
        matched_terms += 1;

        let mut term_score = (body_hits as f32).sqrt();
        if in_title {
            term_score += 2.0;
        }
        if in_tags {
            term_score += 1.5;
        }
        lexical += term_score * weight;
    }

    if matched_terms == 0 {
        return 0.0;
    }

    // Coverage: a document hitting four of five query terms is a far
    // better answer than one hitting the same term four times.
    let coverage = matched_terms as f32 / query_terms.len() as f32;
    let mut score = lexical * (0.5 + coverage);

    // Exact phrase is the strongest lexical signal there is.
    if !query_phrase.is_empty()
        && (body_lower.contains(query_phrase) || title_lower.contains(query_phrase))
    {
        score *= 1.6;
    }

    score * doc.source_type.weight() * recency_multiplier(&doc.timestamp, now)
}

/// Ranks, expands, deduplicates and budget-trims candidates for a query.
///
/// Pure: no filesystem, no clock beyond the `now` handed in, no vault.
/// That is what makes retrieval quality testable rather than anecdotal.
pub fn rank(
    candidates: &[CandidateDoc],
    query: &RetrievalQuery,
    now: chrono::DateTime<chrono::Utc>,
) -> RetrievalResult {
    let allowed: HashSet<SourceType> = query.source_types.iter().copied().collect();
    let in_scope: Vec<CandidateDoc> = candidates
        .iter()
        .filter(|d| allowed.contains(&d.source_type))
        .filter(|d| match &query.since {
            Some(since) => d.timestamp.is_empty() || d.timestamp.as_str() >= since.as_str(),
            None => true,
        })
        .cloned()
        .collect();

    let total_candidates = in_scope.len();
    let query_terms = tokenize(&query.text);
    let query_phrase = query.text.trim().to_lowercase();
    // Only treat the whole question as a phrase when it is short enough to
    // plausibly appear verbatim; a 20-word sentence never will, and the
    // check would just cost a scan per document.
    let query_phrase = if query_phrase.split_whitespace().count() <= 6 {
        query_phrase
    } else {
        String::new()
    };

    if query_terms.is_empty() || in_scope.is_empty() {
        return RetrievalResult {
            items: Vec::new(),
            searched_sources: query.source_types.clone(),
            total_candidates,
        };
    }

    let idf = idf_weights(&query_terms, &in_scope);

    let mut direct: Vec<(f32, &CandidateDoc)> = in_scope
        .iter()
        .map(|d| {
            (
                score_candidate(d, &query_terms, &query_phrase, &idf, now),
                d,
            )
        })
        .filter(|(score, _)| *score > 0.0)
        .collect();
    sort_by_score(&mut direct);

    // One-hop expansion from the strongest direct matches only. Expanding
    // from every hit turns a knowledge graph into a flood — the reason
    // `ARCHITECTURE.md` §6 caps this at one hop from the top few.
    let expansion_seeds: Vec<&CandidateDoc> = direct
        .iter()
        .take(3)
        .map(|(_, d)| *d)
        .collect();
    let direct_ids: HashSet<&str> = direct.iter().map(|(_, d)| d.source_id.as_str()).collect();

    let mut linked_ids: HashSet<String> = HashSet::new();
    let mut seed_topics: HashSet<String> = HashSet::new();
    for seed in &expansion_seeds {
        linked_ids.extend(seed.related_ids.iter().cloned());
        seed_topics.extend(seed.topics.iter().map(|t| t.to_lowercase()));
    }

    let mut expanded: Vec<(f32, &CandidateDoc)> = Vec::new();
    if !linked_ids.is_empty() || !seed_topics.is_empty() {
        let best_direct = direct.first().map(|(s, _)| *s).unwrap_or(0.0);
        for doc in &in_scope {
            if direct_ids.contains(doc.source_id.as_str()) {
                continue;
            }
            let links_back = expansion_seeds
                .iter()
                .any(|seed| doc.related_ids.contains(&seed.source_id));
            let linked = linked_ids.contains(&doc.source_id) || links_back;
            let shares_topic = doc
                .topics
                .iter()
                .any(|t| seed_topics.contains(&t.to_lowercase()));
            if linked || shares_topic {
                // An explicit user/AI relationship is a stronger signal
                // than merely sharing a topic label.
                let strength = if linked { 1.0 } else { 0.7 };
                expanded.push((best_direct * EXPANSION_DISCOUNT * strength, doc));
            }
        }
        sort_by_score(&mut expanded);
    }

    let mut seen: HashSet<(SourceType, String)> = HashSet::new();
    // A meeting and its MeetingFacts row describe the same event; keeping
    // both spends budget twice on one memory. Facts sort first (higher
    // source weight), so the raw meeting is the one dropped.
    let mut seen_meetings: HashSet<String> = HashSet::new();
    let mut items: Vec<ContextItem> = Vec::new();
    let mut spent = 0_usize;

    for (score, doc, was_expanded) in direct
        .iter()
        .map(|(s, d)| (*s, *d, false))
        .chain(expanded.iter().map(|(s, d)| (*s, *d, true)))
    {
        if items.len() >= query.max_results || spent >= query.char_budget {
            break;
        }
        if !seen.insert((doc.source_type, doc.source_id.clone())) {
            continue;
        }
        if matches!(
            doc.source_type,
            SourceType::Meeting | SourceType::MeetingFacts
        ) && !seen_meetings.insert(doc.source_id.clone())
        {
            continue;
        }

        let remaining = query.char_budget.saturating_sub(spent);
        // Give each item a fair slice rather than letting the first
        // document eat the whole budget.
        let per_item = (query.char_budget / query.max_results.max(1)).max(MIN_EXCERPT_CHARS);
        let allowance = per_item.min(remaining);
        if allowance < MIN_EXCERPT_CHARS && !items.is_empty() {
            break;
        }

        let excerpt = excerpt_for(&doc.body, &query_terms, allowance);
        spent += excerpt.chars().count();
        items.push(ContextItem {
            source_type: doc.source_type,
            source_id: doc.source_id.clone(),
            title: doc.title.clone(),
            timestamp: doc.timestamp.clone(),
            relevance: round2(score),
            excerpt,
            detail: doc.detail.clone(),
            expanded: was_expanded,
        });
    }

    RetrievalResult {
        items,
        searched_sources: query.source_types.clone(),
        total_candidates,
    }
}

/// Descending by score, then by id so equal scores order deterministically
/// rather than by whatever the filesystem returned.
fn sort_by_score(scored: &mut [(f32, &CandidateDoc)]) {
    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.source_id.cmp(&b.1.source_id))
    });
}

fn round2(value: f32) -> f32 {
    (value * 100.0).round() / 100.0
}

/// Trims `body` to `allowance` characters, centred on the densest run of
/// query terms rather than always taking the opening.
///
/// The head of a long dictated note is throat-clearing; the answer is
/// usually in the middle. Taking the first N characters is why naive RAG
/// over voice notes retrieves the right document and still misses.
fn excerpt_for(body: &str, query_terms: &[String], allowance: usize) -> String {
    let chars: Vec<char> = body.chars().collect();
    if chars.len() <= allowance {
        return body.trim().to_string();
    }

    let lower: Vec<char> = body.to_lowercase().chars().collect();
    let lower_str: String = lower.iter().collect();

    // Best window start = the position maximising how many query terms
    // begin inside it.
    let mut best_start = 0_usize;
    let mut best_hits = 0_usize;
    let mut positions: Vec<usize> = Vec::new();
    for term in query_terms {
        let mut from = 0;
        while let Some(byte_idx) = lower_str[from..].find(term.as_str()) {
            let absolute = from + byte_idx;
            positions.push(lower_str[..absolute].chars().count());
            from = absolute + term.len();
            if positions.len() > 256 {
                break;
            }
        }
    }
    positions.sort_unstable();

    for &pos in &positions {
        let start = pos.saturating_sub(allowance / 4);
        let end = start + allowance;
        let hits = positions.iter().filter(|p| **p >= start && **p < end).count();
        if hits > best_hits {
            best_hits = hits;
            best_start = start;
        }
    }

    let start = best_start.min(chars.len().saturating_sub(allowance));
    let end = (start + allowance).min(chars.len());
    let mut slice: String = chars[start..end].iter().collect();
    slice = slice.trim().to_string();
    if start > 0 {
        slice.insert(0, '…');
    }
    if end < chars.len() {
        slice.push('…');
    }
    slice
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-08-30T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    fn days_ago(n: i64) -> String {
        (now() - chrono::Duration::days(n)).to_rfc3339()
    }

    fn corpus() -> Vec<CandidateDoc> {
        vec![
            CandidateDoc::new(
                SourceType::Scribble,
                "scribble_pricing",
                "Pricing model rethink",
                "We keep coming back to usage based pricing versus a flat seat licence. \
                 The flat seat licence is simpler to explain to procurement.",
            )
            .with_timestamp(&days_ago(5))
            .with_topics(vec!["pricing".into()])
            .with_related(vec!["scribble_procure".into()]),
            CandidateDoc::new(
                SourceType::Scribble,
                "scribble_procure",
                "Procurement objections",
                "Procurement always asks for an annual number they can budget against.",
            )
            .with_timestamp(&days_ago(9))
            .with_topics(vec!["pricing".into()]),
            CandidateDoc::new(
                SourceType::MeetingFacts,
                "meeting_042",
                "Pricing review",
                "Decision: ship flat seat licence for the first year. \
                 Rationale: procurement predictability outweighs upside.",
            )
            .with_timestamp(&days_ago(2))
            .with_detail("decision"),
            CandidateDoc::new(
                SourceType::Meeting,
                "meeting_042",
                "Pricing review (transcript summary)",
                "A long discussion about pricing that ended with the flat seat licence.",
            )
            .with_timestamp(&days_ago(2)),
            CandidateDoc::new(
                SourceType::VoiceNote,
                "note_rambling",
                "so anyway I was thinking about the garden",
                "So anyway I was thinking about the garden and the fence needs replacing.",
            )
            .with_timestamp(&days_ago(1)),
        ]
    }

    /// Measured, not guessed — `docs/talkback/BENCHMARKS.md` records the
    /// numbers this produces. Ignored by default because a timing test in
    /// CI is a flaky test; run it explicitly:
    ///
    /// ```text
    /// cargo test --release retrieval_scaling -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "benchmark, not a correctness test"]
    fn retrieval_scaling() {
        let vocabulary = [
            "pricing", "procurement", "licence", "kubernetes", "hiring", "retention",
            "onboarding", "latency", "roadmap", "infra", "budget", "launch",
        ];

        for corpus_size in [100_usize, 500, 1_000, 5_000] {
            let docs: Vec<CandidateDoc> = (0..corpus_size)
                .map(|i| {
                    // ~1.2 kB of body each: the size of a real dictated
                    // voice note or a meeting-facts row.
                    let body = (0..40)
                        .map(|w| vocabulary[(i + w) % vocabulary.len()])
                        .collect::<Vec<_>>()
                        .join(" ");
                    CandidateDoc::new(
                        if i % 4 == 0 { SourceType::MeetingFacts } else { SourceType::VoiceNote },
                        &format!("doc_{i}"),
                        vocabulary[i % vocabulary.len()],
                        &body.repeat(3),
                    )
                    .with_timestamp(&days_ago((i % 400) as i64))
                    .with_topics(vec![vocabulary[i % vocabulary.len()].to_string()])
                })
                .collect();

            let query = RetrievalQuery::new("what did we decide about pricing and procurement");
            let started = std::time::Instant::now();
            const RUNS: u32 = 20;
            for _ in 0..RUNS {
                let result = rank(&docs, &query, now());
                std::hint::black_box(result);
            }
            let per_run = started.elapsed() / RUNS;
            println!(
                "retrieval: {:>5} docs -> {:>8.2} ms/query",
                corpus_size,
                per_run.as_secs_f64() * 1000.0
            );
        }
    }

    #[test]
    fn tokenize_drops_stopwords_and_shorts() {
        assert_eq!(
            tokenize("What did we decide about the pricing?"),
            vec!["decide", "pricing"]
        );
    }

    #[test]
    fn tokenize_keeps_a_bare_topic_word() {
        assert_eq!(tokenize("pricing"), vec!["pricing"]);
    }

    #[test]
    fn ranks_the_relevant_documents_and_drops_the_irrelevant_one() {
        let result = rank(&corpus(), &RetrievalQuery::new("pricing decision"), now());
        assert!(!result.is_empty());
        assert!(
            !result.items.iter().any(|i| i.source_id == "note_rambling"),
            "an unrelated voice note must not be retrieved: {:?}",
            result.items.iter().map(|i| &i.source_id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn meeting_facts_outrank_the_raw_meeting_and_collapse_to_one_item() {
        let result = rank(&corpus(), &RetrievalQuery::new("pricing decision"), now());
        let meeting_items: Vec<&ContextItem> = result
            .items
            .iter()
            .filter(|i| i.source_id == "meeting_042")
            .collect();
        assert_eq!(meeting_items.len(), 1, "meeting and its facts must collapse");
        assert_eq!(meeting_items[0].source_type, SourceType::MeetingFacts);
    }

    #[test]
    fn provenance_survives_retrieval() {
        let result = rank(&corpus(), &RetrievalQuery::new("pricing decision"), now());
        let item = result
            .items
            .iter()
            .find(|i| i.source_id == "meeting_042")
            .expect("meeting facts retrieved");
        assert_eq!(item.title, "Pricing review");
        assert_eq!(item.detail.as_deref(), Some("decision"));
        assert!(!item.timestamp.is_empty());
        assert!(item.relevance > 0.0);
    }

    #[test]
    fn source_filter_is_honoured() {
        let query = RetrievalQuery::new("pricing").with_sources(vec![SourceType::Scribble]);
        let result = rank(&corpus(), &query, now());
        assert!(!result.items.is_empty());
        assert!(result
            .items
            .iter()
            .all(|i| i.source_type == SourceType::Scribble));
    }

    #[test]
    fn expansion_pulls_in_a_linked_scribble_and_marks_it() {
        // "usage based" only appears in scribble_pricing; scribble_procure
        // is reachable only through the relationship and shared topic.
        let query = RetrievalQuery::new("usage based licence")
            .with_sources(vec![SourceType::Scribble])
            .with_max_results(5);
        let result = rank(&corpus(), &query, now());
        let procure = result
            .items
            .iter()
            .find(|i| i.source_id == "scribble_procure")
            .expect("linked scribble reached by expansion");
        assert!(procure.expanded);
        let direct = result
            .items
            .iter()
            .find(|i| i.source_id == "scribble_pricing")
            .expect("direct match present");
        assert!(!direct.expanded);
        assert!(direct.relevance > procure.relevance);
    }

    #[test]
    fn empty_query_retrieves_nothing_rather_than_everything() {
        let result = rank(&corpus(), &RetrievalQuery::new("   "), now());
        assert!(result.is_empty());
        assert_eq!(result.total_candidates, 5);
    }

    #[test]
    fn stopword_only_query_retrieves_nothing() {
        let result = rank(&corpus(), &RetrievalQuery::new("what about the"), now());
        assert!(result.is_empty());
    }

    #[test]
    fn since_filter_excludes_older_documents() {
        let query = RetrievalQuery::new("pricing").with_since(Some(days_ago(3)));
        let result = rank(&corpus(), &query, now());
        assert!(!result.is_empty());
        assert!(
            result.items.iter().all(|i| i.source_id == "meeting_042"),
            "only documents newer than the cutoff survive"
        );
    }

    #[test]
    fn char_budget_is_respected() {
        let long_body = "pricing ".repeat(4_000);
        let docs = vec![
            CandidateDoc::new(SourceType::Scribble, "a", "Pricing A", &long_body)
                .with_timestamp(&days_ago(1)),
            CandidateDoc::new(SourceType::Scribble, "b", "Pricing B", &long_body)
                .with_timestamp(&days_ago(2)),
        ];
        let query = RetrievalQuery::new("pricing").with_char_budget(600);
        let result = rank(&docs, &query, now());
        let total: usize = result.items.iter().map(|i| i.excerpt.chars().count()).sum();
        assert!(total <= 700, "excerpts overshot the budget: {}", total);
        assert!(!result.items.is_empty());
    }

    #[test]
    fn max_results_caps_the_item_count() {
        let docs: Vec<CandidateDoc> = (0..20)
            .map(|i| {
                CandidateDoc::new(
                    SourceType::VoiceNote,
                    &format!("n{}", i),
                    "pricing note",
                    "pricing pricing pricing",
                )
                .with_timestamp(&days_ago(i))
            })
            .collect();
        let result = rank(&docs, &RetrievalQuery::new("pricing").with_max_results(3), now());
        assert_eq!(result.items.len(), 3);
    }

    #[test]
    fn ranking_is_deterministic_for_tied_scores() {
        let docs: Vec<CandidateDoc> = ["c", "a", "b"]
            .iter()
            .map(|id| {
                CandidateDoc::new(SourceType::VoiceNote, id, "pricing", "pricing")
                    .with_timestamp(&days_ago(1))
            })
            .collect();
        let first = rank(&docs, &RetrievalQuery::new("pricing"), now());
        let second = rank(&docs, &RetrievalQuery::new("pricing"), now());
        assert_eq!(first.items, second.items);
        assert_eq!(
            first.items.iter().map(|i| i.source_id.as_str()).collect::<Vec<_>>(),
            vec!["a", "b", "c"]
        );
    }

    #[test]
    fn recency_breaks_a_tie_towards_the_newer_document() {
        let docs = vec![
            CandidateDoc::new(SourceType::Scribble, "old", "pricing", "pricing model")
                .with_timestamp(&days_ago(400)),
            CandidateDoc::new(SourceType::Scribble, "new", "pricing", "pricing model")
                .with_timestamp(&days_ago(1)),
        ];
        let result = rank(&docs, &RetrievalQuery::new("pricing model"), now());
        assert_eq!(result.items[0].source_id, "new");
    }

    #[test]
    fn recency_never_beats_relevance() {
        let docs = vec![
            CandidateDoc::new(
                SourceType::Scribble,
                "old_relevant",
                "Pricing model rethink",
                "usage based pricing versus flat seat licence, pricing pricing",
            )
            .with_timestamp(&days_ago(500)),
            CandidateDoc::new(
                SourceType::Scribble,
                "new_irrelevant",
                "Garden",
                "the fence needs replacing but pricing came up once",
            )
            .with_timestamp(&days_ago(0)),
        ];
        let result = rank(&docs, &RetrievalQuery::new("pricing model licence"), now());
        assert_eq!(result.items[0].source_id, "old_relevant");
    }

    #[test]
    fn idf_prefers_the_rare_term() {
        // "meeting" is in every document, "kubernetes" in one.
        let mut docs: Vec<CandidateDoc> = (0..8)
            .map(|i| {
                CandidateDoc::new(
                    SourceType::VoiceNote,
                    &format!("common{}", i),
                    "meeting",
                    "meeting meeting meeting notes",
                )
                .with_timestamp(&days_ago(1))
            })
            .collect();
        docs.push(
            CandidateDoc::new(
                SourceType::VoiceNote,
                "rare",
                "infra",
                "meeting about kubernetes upgrades",
            )
            .with_timestamp(&days_ago(1)),
        );
        let result = rank(&docs, &RetrievalQuery::new("meeting kubernetes"), now());
        assert_eq!(result.items[0].source_id, "rare");
    }

    #[test]
    fn excerpt_centres_on_the_query_terms_not_the_opening() {
        let body = format!("{}{}", "filler ".repeat(200), "the answer is kubernetes upgrades");
        let doc = CandidateDoc::new(SourceType::Scribble, "x", "notes", &body)
            .with_timestamp(&days_ago(1));
        let query = RetrievalQuery::new("kubernetes").with_char_budget(300);
        let result = rank(&[doc], &query, now());
        assert!(
            result.items[0].excerpt.contains("kubernetes"),
            "excerpt lost the matched term: {}",
            result.items[0].excerpt
        );
    }

    #[test]
    fn excerpt_returns_short_bodies_whole() {
        let doc = CandidateDoc::new(SourceType::Scribble, "x", "notes", "short pricing note")
            .with_timestamp(&days_ago(1));
        let result = rank(&[doc], &RetrievalQuery::new("pricing"), now());
        assert_eq!(result.items[0].excerpt, "short pricing note");
    }

    #[test]
    fn missing_timestamps_do_not_panic_or_exclude() {
        let doc = CandidateDoc::new(SourceType::Scribble, "x", "pricing", "pricing note");
        let result = rank(&[doc], &RetrievalQuery::new("pricing"), now());
        assert_eq!(result.items.len(), 1);
        assert!(result.items[0].timestamp.is_empty());
    }

    #[test]
    fn no_candidates_reports_the_searched_sources() {
        let result = rank(&[], &RetrievalQuery::new("pricing"), now());
        assert!(result.is_empty());
        assert_eq!(result.searched_sources.len(), SourceType::ALL.len());
        assert_eq!(result.total_candidates, 0);
    }

    #[test]
    fn source_weights_order_derived_above_raw() {
        assert!(SourceType::MeetingFacts.weight() > SourceType::Scribble.weight());
        assert!(SourceType::Scribble.weight() > SourceType::Meeting.weight());
        assert!(SourceType::Meeting.weight() > SourceType::VoiceNote.weight());
    }
}
