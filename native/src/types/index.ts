export interface ProcessedPipelineResult {
  mode: 'voice_note' | 'scribble' | 'trigger' | 'chat';
  transcript: string;
  note_id?: string;
  kanban_cards_created: number;
  output_markdown: string;
  /** Vault note titles used as grounding context (chat mode only). */
  sources: string[];
  /** Base64 WAV of the answer spoken aloud, if local TTS is configured. */
  spoken_audio_base64?: string | null;
}

export interface VaultNote {
  id: string;
  title: string;
  note_type: string;
  created_at: string;
  updated_at: string;
  tags: string[];
  source_audio?: string | null;
  content: string;
  merged_from?: string[] | null;
}

export interface VaultFile {
  id: string;
  original_filename: string;
  file_type: string;
  mime_type: string;
  size_bytes: number;
  content_hash: string;
  created_at: string;
  updated_at: string;
  last_known_source_path: string;
  vault_path: string;
  extraction_status: 'extracted' | 'pending' | 'failed' | 'unsupported';
  processing_status: 'ready' | 'processing' | 'failed';
  content: string;
  summary?: string | null;
  tags: string[];
  topics: string[];
  entities: string[];
  relationships: ScribbleRelationship[];
  ai_metadata: ScribbleAiMetadata;
  linked_scribble_id?: string | null;
  /**
   * Present only on web captures. Provenance — where the content came from
   * and how completely it was acquired. Semantic fields (`summary`, `tags`,
   * `topics`, `entities`) are produced later by analysis and are kept out of
   * here on purpose.
   */
  capture?: CaptureProvenance | null;
}

/**
 * How much of a page a capture can honestly claim to contain.
 *
 * Relay's four completeness states, in the vocabulary stored on artifacts:
 * `full_document` is FULL, `partial` is PARTIAL, `rendered_dom` is
 * LOADED_ONLY, and `failed` is FAILED. `unknown` means nothing measurable —
 * which is not the same as a failure.
 */
export type CaptureCoverage =
  | 'full_document'
  | 'rendered_dom'
  | 'partial'
  | 'failed'
  | 'unknown';

/** How many elements the reveal pass saw in each availability state. */
export interface AvailabilityCounts {
  outside_viewport: number;
  visually_truncated: number;
  collapsed: number;
  not_loaded: number;
  virtualized: number;
  inaccessible: number;
}

/**
 * What the reveal pass measured. Absent on captures made before v0.27.0.
 *
 * The discovered/captured pairs are the useful part: they are what separates
 * "this is the whole conversation" from "this is a quarter of it".
 */
export interface CaptureTraversal {
  performed: boolean;
  plan: string;
  termination: string;
  steps: number;
  samples: number;
  scroll_span_px: number;
  duration_ms: number;
  scroll_restored: boolean;
  virtualized: boolean;
  settle_timeouts: number;
  expansions_found: number;
  expansions_opened: number;
  expansions_refused: number;
  expansions_failed: number;
  /** Sections whose content was already present, so nothing was clicked. */
  expansions_unnecessary: number;
  messages_discovered: number;
  messages_captured: number;
  messages_missing?: number | null;
  duplicates_dropped: number;
  attachments_discovered: number;
  attachments_captured: number;
  images_discovered: number;
  images_captured: number;
  availability: AvailabilityCounts;
  inaccessible: string[];
}

/** How the content was obtained, best first. */
export type CaptureFidelity = 'structured' | 'generic' | 'text_only';

export interface CaptureProvenance {
  source_type: string;
  /** `conversation` | `article` | `repository` | `issue` | `pull_request` | `discussion` | `code` | `page` */
  capture_type: string;
  application: string;
  domain: string;
  url: string;
  page_title: string;
  /** Relay's own clock, RFC 3339. Authoritative. */
  captured_at: string;
  browser_captured_at?: string | null;
  browser?: string | null;
  extractor_id: string;
  extractor_version: number;
  /**
   * How downstream Relay systems may use this content. Always
   * `external_untrusted` for a web capture, whatever site it came from.
   */
  trust: string;
  fidelity: CaptureFidelity | string;
  coverage: CaptureCoverage | string;
  /** Plain-language statements about what was and was not captured. */
  notes: string[];
  message_count?: number | null;
  block_count: number;
  skipped_block_count: number;
  truncated: boolean;
  canonical_url?: string | null;
  author?: string | null;
  published_at?: string | null;
  language?: string | null;
  version: number;
  previous_capture_id?: string | null;
  recapture_count: number;
  traversal?: CaptureTraversal | null;
}

/** Live state of the local bridge the browser extension talks to. */
export interface CaptureBridgeStatus {
  enabled: boolean;
  running: boolean;
  port: number;
  configured_port: number;
  pairing_token?: string | null;
  protocol_version: number;
  analyze_on_capture: boolean;
  capture_hotkey: string;
  last_error?: string | null;
}

/** Stages broadcast on the `capture-progress` event. */
export type CaptureStage = 'SAVING' | 'SAVED' | 'ANALYSING' | 'ANALYSED' | 'FAILED';

export interface CaptureProgress {
  stage: CaptureStage | string;
  capture_id?: string | null;
  title?: string | null;
  application?: string | null;
  message?: string | null;
}

export interface ContextDecision {
  id: string;
  decision: string;
  rationale?: string | null;
  status: 'CURRENT' | 'SUPERSEDED' | 'MODIFIED' | 'REJECTED';
  source_turn_ordinals: number[];
}

export interface ContextRequirement {
  id: string;
  statement: string;
  source_turn_ordinals: number[];
}

export interface ContextConstraint {
  id: string;
  statement: string;
  reason?: string | null;
  source_turn_ordinals: number[];
}

export interface RejectedApproach {
  approach: string;
  reason_rejected: string;
  source_turn_ordinals: number[];
}

export interface ContextOpenQuestion {
  id: string;
  question: string;
  context_note?: string | null;
  source_turn_ordinals: number[];
}

export interface ContextActionItem {
  id: string;
  description: string;
  owner?: string | null;
  status: string;
  source_turn_ordinals: number[];
}

export interface ContextArtifact {
  name: string;
  kind: string;
  reference_or_path?: string | null;
  description?: string | null;
}

export interface ConversationContext {
  capture_id: string;
  title: string;
  objective: string;
  background: string[];
  current_state: string;
  decisions: ContextDecision[];
  requirements: ContextRequirement[];
  constraints: ContextConstraint[];
  preferences: string[];
  rejected_approaches: RejectedApproach[];
  open_questions: ContextOpenQuestion[];
  action_items: ContextActionItem[];
  important_facts: string[];
  key_artifacts: ContextArtifact[];
  generated_at: string;
  model?: string | null;
  deterministic: boolean;
}

export interface RepositoryContext {
  capture_id: string;
  repository_name: string;
  objective: string;
  stack: string[];
  features: string[];
  user_base: string[];
  licensing?: string | null;
  generated_at: string;
  model?: string | null;
  deterministic: boolean;
}

/**
 * General derived structured context for any captured source (conversations, repositories, documents).
 */
/**
 * How a derived artifact was produced. Mirrors the Rust `AnalysisMetadata`.
 *
 * `status` is the field that matters: `insufficient_evidence` means Relay ran
 * the analysis and the source did not carry what was asked for — a successful,
 * honest outcome, and not the same as `failed`.
 */
export interface AnalysisMetadata {
  analysis_type: 'summary' | 'context' | 'enrichment' | 'extraction';
  status:
    | 'requested'
    | 'running'
    | 'succeeded'
    | 'insufficient_evidence'
    | 'failed'
    | 'cancelled';
  prompt_id: string;
  prompt_version: number;
  provider?: string | null;
  /** The model that actually answered, not the configured provider. */
  model?: string | null;
  deterministic?: boolean;
  source_coverage?: string | null;
  generated_at: string;
  prompt_tokens?: number | null;
  completion_tokens?: number | null;
  failure?: { kind: string; detail?: unknown } | null;
}

export type SourceContext = {
  capture_id: string;
  generated_at: string;
  model?: string | null;
  deterministic: boolean;
  /** Absent on contexts written before the analysis contract existed. */
  analysis?: AnalysisMetadata | null;
} & (
  | { kind: 'conversation'; data: ConversationContext }
  | { kind: 'repository'; data: RepositoryContext }
  | (ConversationContext & { kind?: undefined })
);

export interface ConversationExportItem {
  id: string;
  title: string;
  message_count: number;
  created_at?: string | null;
  updated_at?: string | null;
  has_assets: boolean;
  asset_count: number;
  already_imported_id?: string | null;
}

export interface ExportInspection {
  provider: string;
  provider_display: string;
  total_conversations: number;
  conversations: ConversationExportItem[];
}

export interface VaultLocationInfo {
  /** Absolute path currently in use, whether chosen or defaulted. */
  path: string;
  /** What "Use Default Relay Vault" would set `path` to. */
  default_path: string;
  /** Whether the user has explicitly chosen/confirmed a location. */
  configured: boolean;
  /** Whether `path` currently exists (or can be created) and is usable. */
  accessible: boolean;
}

export interface KanbanCard {
  id: string;
  title: string;
  assignee: string;
  status: 'todo' | 'in_progress' | 'done';
  priority: 'high' | 'medium' | 'low';
  due_date?: string;
  created_at: string;
  description: string;
  source_note_id?: string;
}

export interface TriggerConfig {
  id: string;
  phrase: string;
  action_type: 'mcp_calendar' | 'local_reminder' | 'mcp_notion' | 'mcp_gdrive';
  target_tool: string;
  parameters: Record<string, unknown>;
  enabled: boolean;
}

export interface ProviderSettings {
  active_provider: 'ollama' | 'cloud_openai' | 'cloud_gemini' | 'cloud_anthropic';
  ollama_host: string;
  ollama_model: string;
  cloud_api_key?: string;
  cloud_model?: string;
}

export interface SttSettings {
  /** Path to a GGML Whisper model file (e.g. ggml-small.bin). */
  whisper_model_path?: string | null;
  /** Universal Dictation performance profile: 'fast' (~0.8s, Base model) or 'accurate' (~2.4s, Small model). */
  dictation_quality?: 'fast' | 'accurate';
  dictationQuality?: 'fast' | 'accurate';
  /** Explicit override for dictation thread count (defaults to optimal clamped core allocation). */
  dictation_threads?: number | null;
  dictationThreads?: number | null;
  /** Whether domain vocabulary initial prompting is enabled. Defaults to false. */
  enable_initial_prompt?: boolean;
  /** Optional user-defined technical vocabulary prompt. */
  custom_initial_prompt?: string | null;
  /**
   * Decode quality preset. Trades decode time for how much borderline speech
   * survives: 'fast' is greedy at whisper's stock no-speech threshold,
   * 'quality' is a wider beam at a lower one so little is dropped silently.
   *
   * `''` — the default — means the user has not chosen, and dictation uses
   * what suits it (Fast). Any other value is an explicit override
   * and applies everywhere. The empty string is a real state rather than a
   * missing one, which is why it is in the union: it is what distinguishes
   * "default" from "the user explicitly picked Fast".
   */
  preset?: '' | 'fast' | 'balanced' | 'quality';
  sttPreset?: '' | 'fast' | 'balanced' | 'quality';
  /**
   * Whether dictated text is offered to the Tier 2 cleanup layer.
   *
   * Off by default. The layer costs a model call and may change words, so it
   * is something the user turns on rather than something they discover has
   * been happening.
   */
  text_transform?: boolean;
  textTransform?: boolean;
  /** How far that cleanup may go. Empty means `faithful`, the only style that
   *  cannot change meaning. */
  cleanup_style?: '' | 'faithful' | 'clean' | 'professional' | 'concise';
  cleanupStyle?: '' | 'faithful' | 'clean' | 'professional' | 'concise';
  enableInitialPrompt?: boolean;
  customInitialPrompt?: string | null;
}

export interface SttDiagnosticSnapshot {
  timestamp_epoch_ms: number;
  session_mode: string;
  audio_file?: string | null;

  // Audio characteristics
  original_duration_seconds: number;
  processed_duration_seconds: number;
  sample_rate: number;
  channels: number;
  rms: number;
  peak_amplitude: number;
  near_zero_percent: number;
  has_non_finite: boolean;

  // VAD activity
  speech_detected: boolean;
  vad_start_seconds: number;
  vad_end_seconds: number;
  vad_trimmed_duration_seconds: number;
  silence_removed_percent: number;
  noise_floor: number;
  onset_threshold: number;

  // Model & language resolution
  model_filename: string;
  model_path: string;
  primary_dictation_language: string;
  spoken_languages: string[];
  resolved_whisper_language?: string | null;
  translate: boolean;

  // Effective decoding configuration
  strategy: string;
  best_of: number;
  beam_size?: number | null;
  temperature: number;
  temperature_inc: number;
  used_initial_prompt: boolean;
  initial_prompt_text?: string | null;
  no_speech_thold: number;
  entropy_thold: number;
  logprob_thold: number;

  // Performance & transcription outcome
  inference_duration_ms: number;
  real_time_factor: number;
  segment_count: number;
  transcript: string;
  transcript_char_count: number;
  error?: string | null;
}

export interface AccuracyMetrics {
  reference: string;
  hypothesis: string;
  word_count: number;
  substitutions: number;
  deletions: number;
  insertions: number;
  wer: number;
  cer: number;
  technical_term_accuracy?: number | null;
}

export interface EvaluationResult {
  test_id: string;
  audio_file: string;
  configuration: string;
  language_setting: string;
  resolved_whisper_language?: string | null;
  original_duration_seconds: number;
  processed_duration_seconds: number;
  inference_duration_ms: number;
  real_time_factor: number;
  transcript: string;
  audio_rms: number;
  audio_peak: number;
  near_zero_percent: number;
  speech_detected: boolean;
  vad_trimmed_duration: number;
  model_filename: string;
  sampling_strategy: string;
  best_of: number;
  beam_size?: number | null;
  temperature: number;
  temperature_increment: number;
  initial_prompt_used: boolean;
  no_speech_threshold: number;
  entropy_threshold: number;
  logprob_threshold: number;
  accuracy?: AccuracyMetrics | null;
  fallback_triggered: boolean;
  error?: string | null;
}

export interface CorpusItem {
  test_id: string;
  audio_filename: string;
  category: string;
  language: string;
  reference?: string | null;
  reference_available: boolean;
  description: string;
}


export interface CaptureSettings {
  bridge_enabled: boolean;
  bridge_port: number;
  pairing_token?: string | null;
  analyze_on_capture: boolean;
}

export interface HotkeySettings {
  show_hide_hotkey: string;
  dictation_hotkey: string;
  /** One press starts recording, a second press stops it, instead of
   * holding the key down the whole time. Defaults to false (hold-to-talk). */
  toggle_to_talk: boolean;
  /** Brings the Captures surface forward. Reading a web page is triggered
   * from inside the browser, not from here — see `docs/capture.md`. */
  capture_hotkey: string;
}

export type PillPosition = 'bottom_left' | 'bottom_center' | 'bottom_right' | 'top_center' | 'left_center' | 'right_center';

export interface UiSettings {
  /** Which edge of the active monitor's work area the floating pill anchors to. */
  pill_position: PillPosition;
}

export interface VaultSettings {
  /** Absolute path the user explicitly chose or confirmed, or null/absent
   * if unconfigured (Relay is using its process-relative default). */
  directory?: string | null;
}

export interface LanguageSettings {
  /** Primary language for dictation (ISO code, e.g. "en", "hi", "kn", "ta"). */
  primary_dictation_language: string;
  /** Languages the user speaks (ISO codes, e.g. ["en", "hi"]). */
  spoken_languages: string[];
  /** Target language for generated notes and summaries (ISO code, e.g. "en", "hi"). */
  notes_language: string;
  /** Writing script rule for dictation/notes: "latin" (Romanized) or "native". */
  output_script: string;
  // Optional camelCase aliases for interoperability
  primaryDictationLanguage?: string;
  spokenLanguages?: string[];
  notesLanguage?: string;
  outputScript?: string;
}

export interface DiagnosticsSettings {
  allow_anonymous_diagnostics: boolean;
  first_run_completed: boolean;
  allowAnonymousDiagnostics?: boolean;
  firstRunCompleted?: boolean;
}

export interface CloudSettings {
  supabase_url?: string | null;
  supabase_anon_key?: string | null;
  supabaseUrl?: string | null;
  supabaseAnonKey?: string | null;
}

export interface SoundSettings {
  /** Whether sound effects (start/stop tones) are played during dictation. */
  dictation_sounds: boolean;
  dictationSounds?: boolean;
}

export type InjectionMethod = 'clipboard_paste' | 'keystrokes';

export interface ClipboardSettings {
  /** Automatically paste/type transcribed text into the active app when dictation finishes. */
  auto_paste: boolean;
  /** Keep transcribed text in OS clipboard so you can paste it manually if needed. */
  copy_to_clipboard: boolean;
  /** Injection method: instant clipboard paste (Ctrl+V) or simulated keystrokes. */
  injection_method?: InjectionMethod;
  autoPaste?: boolean;
  copyToClipboard?: boolean;
  injectionMethod?: InjectionMethod;
}

export interface StartupSettings {
  /** Start Relay in the background when logging into the OS. */
  launch_at_login: boolean;
  /** Launch Relay minimized without showing the main control panel window. */
  start_minimized: boolean;
  launchAtLogin?: boolean;
  startMinimized?: boolean;
}

export interface AudioInputSettings {
  /** Prefer system built-in microphone for lower latency. */
  prefer_builtin_mic: boolean;
  /** Explicitly selected microphone device name (null = OS default). */
  selected_device?: string | null;
  /** Keep microphone stream warm ("off", "15s", "30s", "1m", "5m") to avoid warm-up clipping. */
  keep_microphone_warm: string;
  /** Auto-learn corrections made in the target app into user dictionary. */
  preferBuiltinMic?: boolean;
  selectedDevice?: string | null;
  keepMicrophoneWarm?: string;
}

export interface SnippetItem {
  id: string;
  trigger: string;
  snippet_text: string;
  label?: string | null;
  enabled: boolean;
}

export interface AudioDeviceInfo {
  name: string;
  is_default: boolean;
}

/** Mirrors the Rust `AppSettings` struct persisted at `.relay/config/settings.json`. */
export interface AppSettings {
  provider: ProviderSettings;
  stt: SttSettings;
  hotkeys: HotkeySettings;
  ui: UiSettings;
  vault: VaultSettings;
  language: LanguageSettings;
  diagnostics: DiagnosticsSettings;
  cloud?: CloudSettings;
  sound?: SoundSettings;
  clipboard?: ClipboardSettings;
  startup?: StartupSettings;
  audio_input?: AudioInputSettings;
  dictionary?: string[];
  snippets?: SnippetItem[];
  /** Learned "what Whisper said" → "what it meant" repairs, added only when
   *  the user ticks "Teach Relay this correction" on a Voice Note. Distinct
   *  from `dictionary`, which primes the recognizer before it guesses. */
  vocabulary_corrections?: VocabularyCorrection[];
}

export type AccountMode = 'local' | 'hybrid';
export type SubscriptionPlan = 'free' | 'hybrid';

export interface SubscriptionInfo {
  plan: SubscriptionPlan;
  status: string;
  renewal_date?: string | null;
  capabilities: string[];
}

export interface RelayAccount {
  authenticated: boolean;
  user_id?: string | null;
  email?: string | null;
  display_name?: string | null;
  profile_image?: string | null;
  provider?: string | null;
  created_at?: string | null;
  last_authenticated_at?: string | null;
  subscription: SubscriptionInfo;
  account_mode: AccountMode;
  capabilities: string[];
}

export interface RelayProfile {
  id: string;
  display_name: string;
  onboarding_completed: boolean;
  account_mode: AccountMode;
  auth_provider?: string | null;
  email?: string | null;
  profile_image?: string | null;
  installation_id: string;
  created_at: string;
  updated_at: string;
}


export interface DeveloperSettings {
  force_onboarding_on_launch: boolean;
}

export interface InstallationInfo {
  installation_id: string;
  first_installed_at: string;
  platform: string;
  os_version: string;
  app_version: string;
}

export interface UpdateInfo {
  current_version: string;
  latest_version: string;
  update_available: boolean;
  release_notes?: string | null;
  minimum_supported_version: string;
  download_url?: string | null;
  is_offline: boolean;
}

export interface ChangelogItem {
  category: string;
  domain: string;
  text: string;
}

export interface ChangelogEntry {
  version: string;
  date: string;
  release_type: 'major' | 'minor' | 'patch' | string;
  title: string;
  tags: string[];
  domains: string[];
  items: ChangelogItem[];
}

export type ScribbleSourceType =
  | 'voice'
  | 'text'
  | 'file'
  | 'clipboard'
  | 'browser_selection'
  | 'browser_page'
  | 'browser_conversation'
  | 'screenshot'
  | 'image'

export type ScribbleRelationshipType =
  | 'RELATED_TO'
  | 'MENTIONS'
  | 'SAME_TOPIC'
  | 'SAME_PROJECT'
  | 'CONTRADICTS'
  | 'EXTENDS'
  | 'DERIVED_FROM';

export interface ScribbleRelationship {
  id: string;
  target_id: string;
  relationship_type: ScribbleRelationshipType | string;
  confidence: number;
  source: 'ai' | 'user' | 'system' | string;
}

export interface ScribbleAttachment {
  id: string;
  filename: string;
  path: string;
  mime_type?: string | null;
  size_bytes?: number | null;
}

export interface ScribbleAiMetadata {
  enrichment_status: 'pending' | 'enriched' | 'failed' | 'none' | string;
  suggested_concepts: string[];
  suggested_questions: string[];
  suggested_relations: string[];
  last_enriched_at?: string | null;
}

export interface Scribble {
  id: string;
  title: string;
  content: string;
  summary?: string | null;
  source_type: ScribbleSourceType | string;
  source_metadata: Record<string, any>;
  created_at: string;
  updated_at: string;
  tags: string[];
  topics: string[];
  entities: string[];
  relationships: ScribbleRelationship[];
  attachments: ScribbleAttachment[];
  status: 'active' | 'archived' | string;
  ai_metadata: ScribbleAiMetadata;
}

export interface KnowledgeNode {
  id: string;
  node_type: 'scribble' | 'topic' | 'entity' | 'source' | 'project' | 'document' | 'task' | 'voice_note' | 'person' | 'organization' | 'place' | string;
  label: string;
  summary?: string | null;
  metadata: Record<string, any>;
  degree: number;
  source_type?: string | null;
  created_at?: string | null;
  resolved?: boolean;
}

export interface KnowledgeEdge {
  id: string;
  source_id: string;
  target_id: string;
  relationship: string;
  confidence: number;
  source: string;
  is_explicit?: boolean;
}

export interface KnowledgeGraphData {
  nodes: KnowledgeNode[];
  edges: KnowledgeEdge[];
}

export interface GraphFilter {
  include_scribbles?: boolean;
  include_topics?: boolean;
  include_entities?: boolean;
  include_sources?: boolean;
  orphans_only?: boolean;
  query?: string;
}

export interface KnowledgeSearchResult {
  direct_matches: Scribble[];
  related_scribbles: Scribble[];
  matched_topics: string[];
  matched_entities: string[];
  total_count: number;
}

export interface TrashItem {
  id: string;
  original_id: string;
  item_type: 'scribble' | 'voice_note' | string;
  title: string;
  snippet: string;
  deleted_at: string;
  expires_at: string;
}

export interface GraphFiltersSettings {
  searchQuery: string;
  showScribbles?: boolean;
  showVoiceNotes?: boolean;
  showTags: boolean; // Topics
  showEntities?: boolean;
  showAttachments: boolean;
  existingFilesOnly: boolean;
  showOrphans: boolean;
  showUnresolved?: boolean;
}

export interface GraphGroup {
  id: string;
  query: string;
  color: string;
}

export interface GraphDisplaySettings {
  showArrows: boolean;
  textFadeThreshold: number; // 0.00 to 2.00
  nodeSizeMultiplier: number; // 0.50 to 3.00
  linkThickness: number; // 0.50 to 3.00
}

export interface GraphForcesSettings {
  centerForce: number; // 0.00 to 1.00 (normalized)
  repelForce: number; // 0.00 to 20.00 (normalized)
  linkForce: number; // 0.00 to 1.00 (normalized)
  linkDistance: number; // 30 to 500
}

export interface LocalGraphSettings {
  enabled: boolean;
  rootNodeId: string | null;
  depth: number;
}

export type GraphPositionMap = Record<string, { x: number; y: number }>;

export * from './models';

// ── Foundation Roadmap 11-20 Types ──────────────────────────────────────────

export type RetrievalSourceType =
  | 'voice_note'
  | 'scribble'
  | 'file'
  | 'capture'
  | 'memory'
  | 'derived_artifact';

export interface RetrievalProvenance {
  source_id: string;
  source_type: RetrievalSourceType;
  source_origin?: string | null;
  capture_id?: string | null;
  derived_id?: string | null;
  evidence?: string | null;
}

export interface TimeFilter {
  created_after?: string | null;
  created_before?: string | null;
}

export interface RetrievalFilter {
  source_types?: RetrievalSourceType[];
  tags?: string[];
  time_filter?: TimeFilter | null;
  entity_keys?: string[];
}

export interface RetrievalQuery {
  text: string;
  filter?: RetrievalFilter;
  limit?: number | null;
  char_budget?: number | null;
  include_evidence?: boolean;
}

export type MatchType =
  | 'exact_phrase'
  | 'title_match'
  | 'heading_match'
  | 'topic_match'
  | 'entity_match'
  | 'derived_abstraction'
  | 'term_coverage'
  | 'recency_only';

export interface Explainability {
  matched_terms: string[];
  match_types: MatchType[];
  why: string[];
  base_score: number;
  boosts_applied: string[];
  final_score: number;
}

export interface RetrievedItem {
  id: string;
  source_type: RetrievalSourceType;
  title: string;
  content: string;
  snippet: string;
  score: number;
  timestamp?: string | null;
  provenance: RetrievalProvenance;
  topics: string[];
  explainability?: Explainability | null;
  metadata?: any;
}

export interface RetrievalResult {
  query: string;
  items: RetrievedItem[];
  total_matches: number;
  budget_used: number;
}

export type RelationshipType =
  | 'derived_from'
  | 'summarizes'
  | 'analyses'
  | 'references'
  | 'belongs_to'
  | 'supersedes';

export interface RelationshipRecord {
  id: string;
  source_id: string;
  target_id: string;
  relationship_type: RelationshipType;
  confidence: number;
  created_at: string;
  provenance?: string | null;
  metadata?: any;
}

export type EntityCategory =
  | 'person'
  | 'organization'
  | 'project'
  | 'product'
  | 'technology'
  | 'location'
  | 'date'
  | 'url'
  | 'identifier';

export interface EntityMention {
  source_id: string;
  evidence: string;
  confidence: number;
  timestamp?: string | null;
}

export interface ResolvedEntity {
  id: string;
  canonical_name: string;
  category: EntityCategory;
  aliases: string[];
  source_identifiers: string[];
  urls: string[];
  confidence: number;
  mentions: EntityMention[];
}

export type MemoryType =
  | 'fact'
  | 'preference'
  | 'decision'
  | 'project_context'
  | 'relationship'
  | 'instruction';

export type MemoryStatus = 'active' | 'superseded' | 'archived' | 'deleted';

export type EpistemicState = 'current' | 'no_longer_current' | 'known_false' | 'unverified';

export interface MemoryProvenance {
  source_id: string;
  source_type: string;
  evidence: string;
  confidence: number;
  extracted_by: string;
}

export interface MemoryItem {
  id: string;
  memory_type: MemoryType;
  subject: string;
  content: string;
  status: MemoryStatus;
  epistemic_state: EpistemicState;
  confidence: number;
  provenance: MemoryProvenance[];
  superseded_by?: string | null;
  supersedes_id?: string | null;
  created_at: string;
  updated_at: string;
  metadata?: any;
}

export type ContextPackType =
  | 'repository'
  | 'project'
  | 'conversation'
  | 'document'
  | 'general';

export interface ContextPackItem {
  id: string;
  source_id: string;
  item_type: string;
  title: string;
  content: string;
  is_external: boolean;
  provenance: string;
}

export interface ContextPack {
  id: string;
  pack_type: ContextPackType;
  query: string;
  intent?: string | null;
  items: ContextPackItem[];
  entities: ResolvedEntity[];
  memories: MemoryItem[];
  relationships: RelationshipRecord[];
  char_budget: number;
  total_chars: number;
  created_at: string;
}

export type ActionType =
  | 'open_url'
  | 'open_source'
  | 'create_note'
  | 'create_task'
  | 'save_capture'
  | 'copy_content'
  | string;

export type ActionStatus =
  | 'pending'
  | 'requires_confirmation'
  | 'confirmed'
  | 'executing'
  | 'completed'
  | 'failed'
  | 'cancelled';

export interface UniversalAction {
  id: string;
  action_type: ActionType;
  intent?: string | null;
  target: string;
  parameters: any;
  source_context?: string | null;
  requires_confirmation: boolean;
  status: ActionStatus;
  result?: any;
  error_message?: string | null;
  provenance?: string | null;
  created_at: string;
  executed_at?: string | null;
}

export interface CandidateMemory {
  memory_type: MemoryType;
  subject: string;
  content: string;
  evidence: string;
  source_id: string;
  confidence: number;
  reason_for_retention: string;
}

export type FormationAction = 'created' | 'superseded' | 'deduplicated' | 'rejected';

export interface MemoryFormationOutcome {
  action: FormationAction;
  memory?: MemoryItem | null;
  superseded_memory_id?: string | null;
  reason: string;
}

export interface KnowledgeTelemetrySnapshot {
  total_memories: number;
  active_memories: number;
  total_entities: number;
  total_relationships: number;
  total_scribbles: number;
  total_notes: number;
  total_files: number;
  total_captures: number;
}



export * from './navigation';

/** A phrase Whisper keeps getting wrong, and what it should say instead. */
export interface VocabularyCorrection {
  source: string;
  replacement: string;
  enabled: boolean;
  created_at: string;
}

/** One correction applied to a note, kept so it can be reversed and seen.
 *  `start` is a character offset, which is what makes undo a reversal of the
 *  range rather than a snapshot of the whole note. */
export interface CorrectionRecord {
  id: string;
  note_id: string;
  original: string;
  replacement: string;
  start: number;
  corrected_at: string;
  /** Whether the user also ticked "Teach Relay this correction". */
  learned: boolean;
}
