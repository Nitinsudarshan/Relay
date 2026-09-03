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

export interface TtsSettings {
  piper_binary_path?: string | null;
  piper_voice_path?: string | null;
}

/** How Relay found the Piper executable it is using. */
export type PiperOrigin = 'configured' | 'managed' | 'bundled' | 'system_path';

/** A Piper voice model Relay can offer without the user browsing. */
export interface PiperVoice {
  path: string;
  /** Display name from the filename: `en_US-amy-medium`. */
  label: string;
  language?: string | null;
  /** Piper needs a `.onnx.json` beside the model; false means it's absent. */
  has_config: boolean;
}

/**
 * Everything the voice settings UI needs, from one backend call.
 *
 * Mirrors `tts::TtsStatus`. `problems` are already phrased for display —
 * the frontend never composes an error message about TTS itself.
 */
export interface TtsStatus {
  engine: string;
  ready: boolean;
  binaryPath?: string | null;
  binaryOrigin?: PiperOrigin | null;
  voicePath?: string | null;
  voiceLabel?: string | null;
  voiceLanguage?: string | null;
  availableVoices: PiperVoice[];
  problems: string[];
  /** Advanced only — where a manual install would go. */
  installDir: string;
  /** Advanced only — where voice models live. */
  voicesDir: string;
  /** `piper.exe` on Windows, `piper` elsewhere. */
  executableName: string;
  /** Whether Relay can set the voice up itself on this machine. */
  canInstall: boolean;
  /** Why not, already phrased for display. */
  installBlockedReason?: string | null;
  /** What first-run setup installs, so the user need not choose. */
  recommendedVoice?: CatalogueVoice | null;
  /** Every voice Relay offers. */
  catalogue: CatalogueVoice[];
  /** Approximate bytes a first-time setup downloads. */
  downloadBytes: number;
  engineVersion?: string | null;
}

/** A voice as the picker shows it — no URLs, no checksums, no paths. */
export interface CatalogueVoice {
  id: string;
  displayName: string;
  languageLabel: string;
  description: string;
  recommended: boolean;
  installed: boolean;
  downloadBytes: number;
}

/** Which part of setup is running. Mirrors `tts::InstallStage`. */
export type InstallStage =
  | 'preparing'
  | 'downloading_engine'
  | 'downloading_voice'
  | 'installing'
  | 'validating'
  | 'testing'
  | 'done';

/** A progress report from an in-flight voice setup. */
export interface InstallProgress {
  stage: InstallStage;
  label: string;
  item?: string | null;
  receivedBytes: number;
  totalBytes?: number | null;
  /** Whole-setup progress, 0–1. */
  overall: number;
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

export interface ClipboardSettings {
  /** Automatically paste/type transcribed text into the active app when dictation finishes. */
  auto_paste: boolean;
  /** Keep transcribed text in OS clipboard so you can paste it manually if needed. */
  copy_to_clipboard: boolean;
  autoPaste?: boolean;
  copyToClipboard?: boolean;
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
  auto_learn_words: boolean;
  preferBuiltinMic?: boolean;
  selectedDevice?: string | null;
  keepMicrophoneWarm?: string;
  autoLearnWords?: boolean;
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
  tts: TtsSettings;
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
  meetings?: MeetingSettings;
  talkback?: TalkbackSettings;
  dictionary?: string[];
  snippets?: SnippetItem[];
}

export type SpeakerIdentificationSetting = 'automatic' | 'off';
export type DefaultSummaryModeSetting = 'concise' | 'standard' | 'detailed';

/** A user-defined summary extension, stored in settings. */
export interface MeetingExtensionSetting {
  id: string;
  name: string;
  instructions: string;
}

/**
 * Meeting behavior the user controls. Deliberately small — the pipeline's
 * internal stages are not settings.
 */
export interface MeetingSettings {
  /**
   * Whether the Raw Transcript tab is offered. Visibility only: turning this off
   * never deletes the transcript, which remains the source for everything
   * derived from a meeting.
   */
  show_raw_transcript: boolean;
  generate_conversation_transcript: boolean;
  /** Whether a summary is generated automatically once a recording finishes. */
  auto_generate_summary: boolean;
  default_summary_mode: DefaultSummaryModeSetting;
  default_extension_id: string;
  speaker_identification: SpeakerIdentificationSetting;
  extensions: MeetingExtensionSetting[];
  /**
   * Standing instructions for how summaries should read. Presentation only —
   * subordinate to the accuracy rules, so nothing written here can make Relay
   * assign an owner or a deadline the meeting did not establish.
   */
  summary_instructions: string;
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

export type NotificationSurfaceMode = 'system' | 'tauri' | 'both';

export interface DeveloperSettings {
  force_onboarding_on_launch: boolean;
  notification_surface_mode: NotificationSurfaceMode;
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
  | 'meeting';

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
  node_type: 'scribble' | 'topic' | 'entity' | 'source' | 'project' | 'document' | 'task' | 'meeting' | 'voice_note' | 'person' | 'organization' | 'place' | string;
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
  item_type: 'scribble' | 'voice_note' | 'meeting' | string;
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

// =========================================================================
// MEETINGS V2 TYPES
// =========================================================================

export type MeetingState =
  | 'IDLE'
  | 'STARTING'
  | 'RECORDING'
  | 'PAUSED'
  | 'STOPPING'
  | 'FINALIZING'
  | 'COMPLETED'
  | 'INTERRUPTED'
  | 'RECOVERED'
  | 'ERROR';

export interface MeetingSession {
  id: string;
  title: string;
  state: MeetingState;
  created_at: string;
  updated_at: string;
  started_at?: string | null;
  ended_at?: string | null;
  duration_seconds: number;
  chunk_count: number;
  mic_active: boolean;
  sys_audio_active: boolean;
  /** Whether the source was ever audible, as opposed to merely connected. */
  mic_heard: boolean;
  sys_audio_heard: boolean;
  /** Seconds spent paused, already excluded from `duration_seconds`. */
  paused_seconds: number;
  /** Set when capture came up degraded, e.g. no system-audio device. */
  capture_warning?: string | null;
  total_audio_bytes: number;
  transcript_segment_count: number;
  word_count?: number;
  /**
   * @deprecated Legacy derived fields, retained so meetings summarized before
   * the processing pipeline existed remain readable. New summaries live in
   * `MeetingProcessing`; nothing writes these any more.
   */
  summary?: string | null;
  /** @deprecated See `summary`. Superseded by `MeetingFacts.action_items`. */
  action_items?: string[];
  pending_transcription_chunks: number;
  error_message?: string | null;
}

/**
 * Derived meeting data — everything the processing pipeline produces from a raw
 * transcript, mirroring `meetings_v2::processing::model` in the Rust backend.
 *
 * The distinction that matters throughout: `MeetingSession` and
 * `TranscriptSegment` are *source* records written by the recorder;
 * everything below is *derived* and can be regenerated at any time. Nothing in
 * the UI should write to the former.
 */

export type SegmentChannel = 'MIC' | 'SYSTEM' | 'MIXED' | 'UNKNOWN';
export type SpeakerOrigin = 'CHANNEL' | 'DIARIZATION' | 'MANUAL';

/**
 * A meeting participant. `id` is the stable identifier used by every derived
 * object; `display_name` is presentation and may be changed at any time.
 */
export interface Speaker {
  id: string;
  display_name?: string | null;
  /** What to call this speaker before anyone has named them — "Me", "Speaker 1". */
  fallback_label: string;
  origin: SpeakerOrigin;
  channel: SegmentChannel;
  is_local_user: boolean;
  segment_count: number;
}

export interface NormalizedSegment {
  id: string;
  chunk_index: number;
  /**
   * Which utterance within the chunk this is. Null for a whole-chunk segment —
   * a transcript recorded before v2.5, or a chunk Whisper returned no timed
   * spans for.
   */
  utterance_index?: number | null;
  start_time_s: number;
  end_time_s: number;
  text: string;
  /** The raw STT text this was cleaned from, carried through for comparison. */
  raw_text: string;
  channel: SegmentChannel;
  speaker_id?: string | null;
  applied_rules: string[];
}

export interface NormalizedTranscript {
  segments: NormalizedSegment[];
  rule_hits: Record<string, number>;
  source_char_count: number;
  output_char_count: number;
  dropped_segment_count: number;
}

export interface ConversationTurn {
  id: string;
  /** Null where channel attribution was ambiguous. Never guessed. */
  speaker_id?: string | null;
  start_time_s: number;
  end_time_s: number;
  text: string;
  segment_ids: string[];
}

export interface Conversation {
  turns: ConversationTurn[];
  unattributed_turn_count: number;
}

/**
 * Notes a person wrote about a meeting.
 *
 * A source artifact, not derived: generating or regenerating a summary reads
 * them and never writes them.
 */
export interface MeetingNotes {
  /** Written during or after the meeting. The common case. */
  during: string;
  /** Written before it, if anything was. Optional enrichment, roughly 1 in 100. */
  before: string;
  updated_at?: string | null;
}

export type OwnerType = 'ME' | 'SPEAKER' | 'EXTERNAL' | 'GROUP' | 'UNASSIGNED';
export type ActionItemStatus = 'OPEN' | 'DONE';

export interface ActionItem {
  id: string;
  description: string;
  owner_type: OwnerType;
  /** Set only for ME and SPEAKER owners; the name is resolved at render time. */
  owner_speaker_id?: string | null;
  /** A name for owners who are not captured speakers. */
  owner_label?: string | null;
  /** ISO date, and only when a date was actually spoken. */
  deadline?: string | null;
  status: ActionItemStatus;
  source_segment_ids: string[];
  confidence: number;
  /**
   * The Kanban card this to-do was added to, if it has been. Null means it has
   * not left the meeting yet.
   */
  kanban_card_id?: string | null;
}

export interface Decision {
  id: string;
  statement: string;
  /**
   * Why it was settled this way, when the meeting said so. Null when no reason
   * was given — never filled in with a plausible one.
   */
  rationale?: string | null;
  decided_by_speaker_id?: string | null;
  source_segment_ids: string[];
  confidence: number;
}

export type RiskKind = 'RISK' | 'BLOCKER' | 'DEPENDENCY' | 'CONSTRAINT';

/** A risk, blocker, dependency, or constraint the meeting actually raised. */
export interface MeetingRisk {
  id: string;
  statement: string;
  kind: RiskKind;
  raised_by_speaker_id?: string | null;
  source_segment_ids: string[];
}

export interface MeetingTopic {
  id: string;
  label: string;
  segment_ids: string[];
}

export type EntityKind =
  | 'PERSON'
  | 'ORGANIZATION'
  | 'PRODUCT'
  | 'PROJECT'
  | 'TECHNOLOGY'
  | 'OTHER';

export interface MeetingEntity {
  id: string;
  name: string;
  kind: EntityKind;
  segment_ids: string[];
}

export interface OpenQuestion {
  id: string;
  question: string;
  source_segment_ids: string[];
}

/**
 * What kind of claim a key point is. Keeps a proposal from being read — or
 * rendered — as something the meeting settled.
 */
export type KeyPointKind =
  | 'DISCUSSION'
  | 'PROPOSAL'
  | 'RECOMMENDATION'
  | 'DISAGREEMENT'
  | 'TRADEOFF';

export interface KeyPoint {
  id: string;
  text: string;
  kind: KeyPointKind;
  topic_id?: string | null;
  source_segment_ids: string[];
}

export type MeetingType =
  | 'SCRUM'
  | 'ONE_ON_ONE'
  | 'PROJECT_REVIEW'
  | 'CLIENT_MEETING'
  | 'PLANNING'
  | 'INTERVIEW'
  | 'GENERAL';

/** The structured intermediate representation every derived view projects from. */
export interface MeetingFacts {
  title: string;
  meeting_type: MeetingType;
  key_points: KeyPoint[];
  topics: MeetingTopic[];
  decisions: Decision[];
  action_items: ActionItem[];
  open_questions: OpenQuestion[];
  risks: MeetingRisk[];
  entities: MeetingEntity[];
  speaker_ids: string[];
  /** True when no model was reachable and these came from the cue-based extractor. */
  deterministic: boolean;
}

export type SummaryMode = 'CONCISE' | 'STANDARD' | 'DETAILED';

export interface MeetingExtension {
  id: string;
  name: string;
  instructions: string;
  builtin: boolean;
}

export type IssueSeverity = 'WARNING' | 'ERROR';

export interface ValidationIssue {
  code: string;
  severity: IssueSeverity;
  message: string;
}

export interface ValidationReport {
  passed: boolean;
  issues: ValidationIssue[];
}

/**
 * What became of the model's proposed prose — independent of whether the summary
 * stage succeeded. A rejected draft followed by a valid deterministic render is
 * a successful summary that happens to have rejected the model.
 */
export type ProviderOutputStatus =
  | 'NOT_ATTEMPTED'
  | 'ACCEPTED'
  | 'REJECTED'
  | 'UNAVAILABLE';

/**
 * Where the prose came from. `DETERMINISTIC_PRESENTATION` means a model
 * understood the meeting but did not write this text; `DETERMINISTIC_EXTRACTION`
 * means no model was involved at any stage. Neither is an AI summary.
 */
export type SummarySource =
  | 'MODEL'
  | 'DETERMINISTIC_PRESENTATION'
  | 'DETERMINISTIC_EXTRACTION';

export interface SummaryArtifact {
  markdown: string;
  mode: SummaryMode;
  extension_id: string;
  generated_at: string;
  provider: string;
  model: string;
  processing_version: number;
  rules_version: string;
  /** True when the prose was rendered from facts without a model. */
  deterministic: boolean;
  source: SummarySource;
  provider_output_status: ProviderOutputStatus;
  /** True when the deterministic renderer produced the text being shown. */
  fallback_used: boolean;
  /**
   * Why a model draft was rejected. Deliberately separate from `validation`,
   * which describes only the prose actually on screen.
   */
  rejected_issues: ValidationIssue[];
  /**
   * True when the first draft failed validation and a corrected one was asked
   * for. "Needed a second try" and "could not do it" are different signals.
   */
  repair_attempted: boolean;
  /** The word ceiling this meeting's own length allowed. */
  length_budget_words?: number | null;
  /** True when a speaker was renamed after this prose was written. */
  speaker_names_stale: boolean;
  validation: ValidationReport;
}

export type StageStatus = 'NOT_RUN' | 'RUNNING' | 'SUCCESS' | 'FAILED' | 'SKIPPED';

/**
 * What the action-item gate did with each candidate. Counts only — the
 * candidates' own words never leave the backend.
 */
export interface ActionDiagnostics {
  candidates: number;
  rejected: number;
  deduplicated: number;
  capped: number;
  retained: number;
  unassigned: number;
  with_deadlines: number;
  owners_downgraded: number;
}

export interface StageState {
  status: StageStatus;
  started_at?: string | null;
  finished_at?: string | null;
  duration_ms?: number | null;
  error?: string | null;
  provider?: string | null;
  model?: string | null;
  input_chars?: number | null;
  output_chars?: number | null;
  validation?: ValidationReport | null;
  /** Extraction only. */
  action_diagnostics?: ActionDiagnostics | null;
}

export interface StageStates {
  normalization: StageState;
  speakers: StageState;
  conversation: StageState;
  extraction: StageState;
  summary: StageState;
}

/**
 * Processing status, rolled up from the stages. Never conflated with
 * `MeetingState`, which describes recording.
 */
export type ProcessingStatus = 'NOT_STARTED' | 'RUNNING' | 'READY' | 'PARTIAL' | 'FAILED';

export interface ScribbleRef {
  scribble_id: string;
  created_at: string;
  title: string;
}

/** A meeting's complete derived artifact (`processing.json`). */
export interface MeetingProcessing {
  meeting_id: string;
  processing_version: number;
  rules_version: string;
  updated_at: string;
  status: ProcessingStatus;
  stages: StageStates;
  normalized?: NormalizedTranscript | null;
  speakers: Speaker[];
  conversation?: Conversation | null;
  facts?: MeetingFacts | null;
  summary?: SummaryArtifact | null;
  scribble_ref?: ScribbleRef | null;
}

export interface RelatedSignals {
  shared_topics: string[];
  shared_entities: string[];
  shared_speakers: string[];
  same_meeting_type: boolean;
  title_similarity: number;
}

export interface RelatedMeeting {
  meeting_id: string;
  title: string;
  created_at: string;
  meeting_type: MeetingType;
  score: number;
  signals: RelatedSignals;
}

export interface ProcessingLogEntry {
  meeting_id: string;
  stage: string;
  status: string;
  at: string;
  duration_ms?: number | null;
  provider?: string | null;
  model?: string | null;
  input_chars?: number | null;
  output_chars?: number | null;
  validator_passed?: boolean | null;
  validator_issue_codes: string[];
  error?: string | null;
  processing_version: number;
  rules_version: string;
}

/** Compact per-meeting processing info for the meetings list. */
export interface MeetingProcessingIndexEntry {
  meeting_id: string;
  title?: string | null;
  status: ProcessingStatus;
  meeting_type?: string | null;
  has_summary: boolean;
  open_action_item_count: number;
  action_item_count: number;
}

export type TranscriptSegmentStatus = 'SUCCESS' | 'EMPTY' | 'FAILED';

export interface TranscriptSegment {
  chunk_index: number;
  start_time_s: number;
  end_time_s: number;
  text: string;
  created_at: string;
  status: TranscriptSegmentStatus;
  /**
   * Whether each capture source was audible in this chunk. The basis for
   * channel-based speaker attribution; both false means the channel is unknown,
   * as it is for transcripts recorded before this was persisted.
   */
  mic_had_audio?: boolean;
  sys_had_audio?: boolean;
  /**
   * Whisper's own utterance spans within this chunk, each already resolved to a
   * channel from the chunk's per-second energy track. Empty for transcripts
   * recorded before v2.5, which are still read from the chunk-level flags above.
   */
  utterances?: TranscriptUtterance[];
}

/**
 * A live transcript update. Updates sharing a `segment_id` belong to the same
 * utterance and *replace* one another as it grows; the last one has
 * `is_final: true`. Key on `segment_id` — appending every update would read as
 * duplicated speech.
 */
export interface LiveTranscriptUpdate {
  segment_id: string;
  session_id: string;
  utterance_index: number;
  start_time_s: number;
  end_time_s: number;
  text: string;
  is_final: boolean;
  latency_ms: number;
}

export interface AudioLevels {
  mic_level: number;
  sys_level: number;
}

export interface MeetingDiagnostics {
  session_id: string;
  state: MeetingState;
  duration_seconds: number;
  last_audio_saved_at?: string | null;
  chunk_count: number;
  total_audio_bytes: number;
  last_transcription_at?: string | null;
  transcript_segment_count: number;
  pending_transcription_chunks: number;
  mic_active: boolean;
  sys_audio_active: boolean;
  mic_heard: boolean;
  sys_audio_heard: boolean;
  mic_rms: number;
  sys_rms: number;
  error?: string | null;
}

/** One utterance inside a chunk, with the channel live while it was spoken. */
export interface TranscriptUtterance {
  index: number;
  /** Absolute session time, not an offset within the chunk. */
  start_time_s: number;
  end_time_s: number;
  text: string;
  mic_had_audio: boolean;
  sys_had_audio: boolean;
  /** Whisper's own no-speech probability for the span, for diagnostics. */
  no_speech_prob?: number;
}

/** The outcome of adding one meeting to-do to the Kanban board. */
export interface MeetingTaskPushResult {
  action_item_id: string;
  kanban_card_id?: string | null;
  title: string;
  assignee: string;
  /** Set only when this to-do could not be added. */
  error?: string | null;
}

// ── Talkback ────────────────────────────────────────────────────────────
//
// Mirrors `native/src-tauri/src/talkback/`. The backend owns the state
// machine; these types are what it broadcasts and the UI renders.

/** Where a piece of retrieved context came from. */
/**
 * Every source Talkback can retrieve from — the serialized form of
 * `talkback::retrieval::SourceType`. Keep it exhaustive: the settings UI
 * derives its toggle list from a `Record` keyed by this union, so a source
 * missing here is a source the user can silently switch off and never back on.
 */
export type TalkbackSourceType =
  | 'VOICE_NOTE'
  | 'SCRIBBLE'
  | 'MEETING'
  | 'MEETING_FACTS'
  | 'FILE'
  | 'CAPTURE';

/** One retrieved piece of context, with its provenance intact. */
export interface TalkbackContextItem {
  source_type: TalkbackSourceType;
  source_id: string;
  title: string;
  timestamp: string;
  relevance: number;
  excerpt: string;
  detail?: string | null;
  /** Reached through a relationship/topic hop rather than by matching. */
  expanded?: boolean;
}

export interface TalkbackRetrievalResult {
  items: TalkbackContextItem[];
  searchedSources: TalkbackSourceType[];
  totalCandidates: number;
}

/**
 * The authoritative conversation state. Rendered, never invented — the
 * frontend has no path that sets this itself.
 */
export type TalkbackStateName =
  | 'OFF'
  | 'STARTING'
  | 'LISTENING'
  | 'USER_SPEAKING'
  | 'TRANSCRIBING'
  | 'THINKING'
  | 'SPEAKING'
  | 'INTERRUPTED'
  | 'ERROR';

export type TalkbackIntent =
  | 'PERSONAL_MEMORY'
  | 'START_VOICE_NOTE'
  | 'STOP_VOICE_NOTE'
  | 'CREATE_SCRIBBLE'
  | 'SHOW_SOURCES'
  | 'GENERAL';

export interface TalkbackTurn {
  turn_id: string;
  role: 'user' | 'agent';
  text: string;
  timestamp: string;
  sources: TalkbackContextItem[];
  intent?: TalkbackIntent | null;
  typed?: boolean;
}

export interface TalkbackSession {
  session_id: string;
  started_at: string;
  turns: TalkbackTurn[];
  voice_note_buffer?: string | null;
}

/** Durations, counts and ids only — never transcript or context text. */
export interface TalkbackMetrics {
  session_id: string;
  turn_id: string;
  provider: string;
  model: string;
  stt_ms?: number | null;
  retrieval_ms: number;
  retrieved_count: number;
  candidate_count: number;
  llm_first_token_ms?: number | null;
  llm_total_ms?: number | null;
  /** Turn start → first audio available. What the user actually waited. */
  tts_first_audio_ms?: number | null;
  /** How long the first synthesis call alone took. */
  tts_first_synthesis_ms?: number | null;
  tts_total_synthesis_ms?: number;
  tts_phrases?: number;
  /** Voice switched off mid-turn because the configuration is broken. */
  tts_disabled?: boolean;
  total_ms: number;
  interrupted: boolean;
  deterministic: boolean;
  intent: TalkbackIntent;
}

export type TalkbackActivationMode = 'toggle' | 'wake_word';

export interface TalkbackSettings {
  activation_mode: TalkbackActivationMode;
  speak_responses: boolean;
  allow_barge_in: boolean;
  /** Empty means every source. */
  sources: TalkbackSourceType[];
  end_of_turn_silence_ms: number;
}
