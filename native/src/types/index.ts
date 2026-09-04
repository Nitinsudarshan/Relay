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
  /**
   * Whether individual speakers are separated acoustically, rather than only
   * split into "me" and everyone else.
   *
   * Creates no biometric data: voice features live for the duration of one run
   * and are never stored or matched across meetings.
   */
  identify_individual_speakers: boolean;
  /**
   * A clustering hint: how many people are expected to speak. Null means work
   * it out. Setting it cannot invent a speaker the audio does not support.
   */
  expected_speakers?: number | null;
  /** Which method decides who spoke. */
  diarization_engine?: DiarizationEngineId;
  /**
   * Meetings are recorded with everybody sharing one microphone. Turns off the
   * local-user inference, because the channel split that finds the person at
   * this machine means nothing when every voice arrives on the same input.
   */
  meetings_are_in_person?: boolean;
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
  /**
   * Chunks whose decode was thrown away as something other than speech.
   *
   * The number that explains a thin summary: nine rejected chunks is four
   * minutes of the meeting that never reached the model.
   */
  rejected_chunk_count?: number;
  /** Voiced seconds across the whole recording. Against `duration_seconds`, the talk-to-silence ratio. */
  voiced_seconds?: number;
  /**
   * Distinct voices the recorder had heard by the time it last wrote this.
   * Live rather than final — the post-hoc pass may revise it — so the recording
   * pill can show speakers appearing during the meeting.
   */
  live_speaker_count?: number;
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
/**
 * What a directive is telling Relay.
 *
 * Each kind is read by the pipeline stage that can act on it, which is the
 * point: a name correction typed as a sentence in a paragraph only works if a
 * model notices it, whereas `SPEAKER_NAME` renames the speaker in the registry
 * and every derived view picks it up at read time.
 */
export type DirectiveKind =
  /** "Speaker 2 is Pranjali." Renames a speaker. Needs a subject. */
  | 'SPEAKER_NAME'
  /** "Ayush was on this call." Adds a participant whether or not they spoke. */
  | 'PARTICIPANT'
  /** "It is LanceDB, not Lance TV." Adds a glossary term. Needs a subject. */
  | 'TERM'
  /** What the meeting was for. Context, never evidence of a decision. */
  | 'AGENDA'
  /** Anything else worth remembering. */
  | 'NOTE';

/** One short, typed instruction a person gave about a meeting. */
export interface MeetingDirective {
  id: string;
  kind: DirectiveKind;
  /**
   * For `SPEAKER_NAME`, which speaker (id, display name, or `Speaker N`).
   * For `TERM`, the misheard spelling. Unused otherwise.
   */
  subject?: string | null;
  /** For `SPEAKER_NAME` the name, for `TERM` the correct spelling, else the content. */
  value: string;
  created_at: string;
}

export interface MeetingNotes {
  /** Short typed instructions. The primary surface for correcting a meeting. */
  directives: MeetingDirective[];
  /** Written during or after the meeting. The common case for prose. */
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
/**
 * How an acoustic speaker-separation run turned out.
 *
 * `well_separated` is the field the UI must respect: false means the clusters
 * sit no further from each other than their own members do, so the roster is
 * provisional and must not be presented as fact.
 */
export interface DiarizationReport {
  cluster_count: number;
  placed_count: number;
  unplaced_count: number;
  skipped_count: number;
  /**
   * Which cluster is the person using this machine, decided by comparing
   * microphone share between clusters rather than by any single threshold.
   * Null when no voice stands out — an in-person meeting through one mic, or a
   * recording where the user never spoke.
   */
  local_cluster?: number | null;
  well_separated: boolean;
  mean_within_distance: number;
  min_between_distance: number;
  /** Speakers heard exactly once — real, or a stray utterance that looked like one. */
  singleton_speaker_count?: number;
  /**
   * How well the roster describes the recording, as a mean silhouette in
   * -1..1. The number the speaker count was decided on, so the number to look
   * at when a roster is wrong. Above 0.8 the voices are clearly separate;
   * 0.7-0.8 is worth checking; below 0.7 no split is made. Zero means one
   * speaker, where a silhouette is undefined.
   */
  silhouette?: number;
  expected_speakers?: number | null;
  duration_ms: number;
}

export interface VoiceAssignment {
  segment_id: string;
  cluster?: number | null;
  distance: number;
}

export interface Diarization {
  report: DiarizationReport;
  assignments: VoiceAssignment[];
}

/**
 * Which method decides who spoke.
 *
 * Three, and selectable, because speaker identity is the part of meetings that
 * has been hardest to get right and they fail differently. `CHANNEL` reads only
 * which input carried the sound, so everyone remote shares one label.
 * `VOICEPRINT` clusters the whole recording after it ends — the most accurate,
 * and what summaries are built from. `LIVE` is the registry the recorder builds
 * as chunks land: available during the call, less certain.
 */
export type DiarizationEngineId = 'CHANNEL' | 'VOICEPRINT' | 'LIVE';

/** One engine's answer for a recording, with enough detail to compare it. */
export interface EngineOutcome {
  engine: DiarizationEngineId;
  id: string;
  label: string;
  summary: string;
  diarization: Diarization;
  /** Utterances per speaker, largest first — the shape of the answer at a glance. */
  speaker_sizes: number[];
  /** Set when the engine could not run. Reported rather than the row being dropped. */
  error?: string | null;
}

/** Every engine's answer for one recording. */
export interface EngineComparison {
  meeting_id: string;
  outcomes: EngineOutcome[];
  /** The engine in force now, so the comparison says what is used as well as what is possible. */
  active: DiarizationEngineId;
  expected_speakers?: number | null;
}

/** How a participant came to be on the list. */
export type ParticipantOrigin =
  | 'LOCAL_USER'
  | 'CHANNEL'
  | 'DIARIZATION'
  | 'SELF_INTRODUCED'
  | 'MENTIONED'
  | 'STATED'
  /** Invited, per the calendar event. Weaker evidence than a voice actually heard. */
  | 'INVITED';

/** One person the meeting involved, whether or not they were heard. */
export interface Participant {
  speaker_id?: string | null;
  /** Never invented: a name somebody supplied, or a `Speaker N` label. */
  label: string;
  is_named: boolean;
  /** True when a person confirmed the name, false when it was inferred. */
  is_confirmed: boolean;
  origin: ParticipantOrigin;
  is_local_user: boolean;
  speaking_seconds: number;
  turn_count: number;
  /** Share of attributed talking time, 0..1. */
  share_of_talk: number;
}

/**
 * What became of every recorded chunk.
 *
 * The numbers that explain a thin summary: nine rejected chunks is four and a
 * half minutes of the meeting that never reached the model.
 */
export interface TranscriptHealth {
  chunk_count: number;
  decoded_chunk_count: number;
  empty_chunk_count: number;
  rejected_chunk_count: number;
  failed_chunk_count: number;
  voiced_seconds: number;
  rejected_seconds: number;
  rejection_reasons: Record<string, number>;
  /** Spans withheld at read time, recorded before hallucination screening existed. */
  withheld_on_read: Record<string, number>;
  withheld_word_count: number;
}

/** What established the speaker roster. */
export type SpeakerMethod = 'NONE' | 'CHANNEL' | 'DIARIZATION';

/**
 * Everything about a meeting that is counted rather than inferred.
 *
 * Distinct from `MeetingFacts`, which is what a model read out of the
 * transcript and can be wrong. Nothing here can be wrong the way a generated
 * sentence can.
 */
export interface MeetingMetadata {
  title: string;
  date_iso: string;
  started_at?: string | null;
  ended_at?: string | null;
  duration_seconds: number;
  paused_seconds: number;
  speaking_participant_count: number;
  participants: Participant[];
  chunk_count: number;
  word_count: number;
  turn_count: number;
  health: TranscriptHealth;
  speaker_method: SpeakerMethod;
}

export type NameEvidence = 'SELF_INTRODUCTION' | 'DIRECT_ADDRESS';

/** A name the transcript offered, and what it was offered for. */
export interface NameCandidate {
  name: string;
  evidence: NameEvidence;
  /** Set only for a self-introduction. */
  speaker_id?: string | null;
  source_segment_ids: string[];
  mentions: number;
}

export interface NameFindings {
  /** Speaker id to the name that speaker gave for themselves. */
  self_introductions: Record<string, NameCandidate>;
  /** Names addressed in the meeting but not bound to any voice. */
  mentioned: NameCandidate[];
}

/**
 * An instruction that could not be applied — a name correction naming a
 * speaker this meeting does not have, say. Surfaced rather than swallowed, or
 * the user assumes the correction took.
 */
export interface UnresolvedDirective {
  directive_id: string;
  kind: DirectiveKind;
  summary: string;
  reason: string;
}

/**
 * One meeting-pipeline check and what it found.
 *
 * `detail` always carries the measurement behind the verdict: a check that
 * reports only pass/fail cannot be trusted, and a failure with no number
 * cannot be diagnosed.
 */
export interface MeetingSelfTestCheck {
  id: string;
  name: string;
  /** What a pass actually proves, written for whoever is reading the panel. */
  purpose: string;
  passed: boolean;
  detail: string;
  duration_ms: number;
}

export interface MeetingSelfTestReport {
  checks: MeetingSelfTestCheck[];
  passed: number;
  failed: number;
  duration_ms: number;
  /** False means no Whisper model is configured — not a failure. */
  whisper_checked: boolean;
  /**
   * What the installed Whisper model produced from thirty seconds of room tone.
   *
   * The most useful line in the report: subtitle boilerplate here is the exact
   * hallucination the pipeline exists to catch, produced by this model on this
   * machine.
   */
  whisper_on_silence?: string | null;
}

// =========================================================================
// CALENDAR
// =========================================================================

/** Whether somebody said they were coming. */
export type AttendanceResponse = 'ACCEPTED' | 'DECLINED' | 'TENTATIVE' | 'NO_RESPONSE';

export interface CalendarAttendee {
  /** A display name, or one recovered from the address. Never a bare email. */
  name: string;
  email?: string | null;
  response: AttendanceResponse;
  is_organizer: boolean;
  /** True for the account that authorized Relay — the person recording. */
  is_self: boolean;
}

/**
 * One event, reduced to what a meeting record needs: what it was called, who
 * was invited, and what it was for.
 *
 * External content. Titles and descriptions are written by whoever sent the
 * invitation, and are shown as data rather than followed as instructions.
 */
export interface CalendarEvent {
  id: string;
  title: string;
  starts_at: string;
  ends_at: string;
  description?: string | null;
  location?: string | null;
  attendees: CalendarAttendee[];
  conference_url?: string | null;
  organizer?: string | null;
}

export interface EventMatch {
  event: CalendarEvent;
  /** Share of the recording that fell inside the event, 0..1. */
  overlap: number;
}

/**
 * Why no event was matched. Three reasons rather than a bare null, because
 * "nothing was scheduled" and "two things fit equally" need different responses
 * from the user.
 */
export type NoMatchReason = 'NOTHING_SCHEDULED' | 'TOO_LITTLE_OVERLAP' | 'AMBIGUOUS';

export type MatchOutcome =
  | ({ kind: 'MATCHED' } & EventMatch)
  | { kind: 'NONE'; reason: NoMatchReason; candidates: EventMatch[] };

/** What the calendar had to say about one recording. */
export interface MeetingCalendarLink {
  outcome: MatchOutcome;
  linked_at: string;
  /** True when a person chose this event rather than Relay matching it. */
  chosen_by_user: boolean;
}

/** Whether Relay can read the calendar, and as whom. */
export interface CalendarConnection {
  connected: boolean;
  account_email?: string | null;
  account_name?: string | null;
  /** A stored connection that cannot currently be used, in words naming the fix. */
  problem?: string | null;
}

/** A meeting rendered as one Markdown document, for sharing. */
export interface SharedDocument {
  filename: string;
  contents: string;
  /** What went in, for the confirmation shown after copying. */
  includes: string;
}

export interface MeetingProcessing {
  meeting_id: string;
  processing_version: number;
  rules_version: string;
  updated_at: string;
  status: ProcessingStatus;
  stages: StageStates;
  normalized?: NormalizedTranscript | null;
  speakers: Speaker[];
  /** The acoustic separation the roster was built from. Null means channel only. */
  diarization?: Diarization | null;
  conversation?: Conversation | null;
  /** Counted facts: participants, timing, transcript health. */
  metadata?: MeetingMetadata | null;
  /** Names the transcript itself offered. Never treated as confirmed. */
  names?: NameFindings | null;
  unresolved_directives?: UnresolvedDirective[];
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

/**
 * `REJECTED` is distinct from `EMPTY` on purpose. `EMPTY` means the recorder
 * heard no speech in the chunk and never decoded it; `REJECTED` means Whisper
 * decoded it and the result was thrown away as something other than speech —
 * a decoder loop, subtitle filler over silence, or more words than the voiced
 * time could hold. Conflating them hides the failure.
 */
export type TranscriptSegmentStatus = 'SUCCESS' | 'EMPTY' | 'FAILED' | 'REJECTED';

/** Why a decode was rejected as something other than speech. */
export type HallucinationReason =
  | { kind: 'REPETITION_LOOP'; phrase: string; repeats: number }
  | { kind: 'FILLER_OVER_SILENCE'; phrase: string; voiced_seconds: number; voiced_ratio: number }
  | { kind: 'NO_SPEECH'; probability: number }
  | {
      kind: 'IMPLAUSIBLE_RATE';
      words: number;
      voiced_seconds: number;
      words_per_second: number;
    };

/**
 * A rejected decode, kept on the segment in place of text.
 *
 * The discarded text is retained so the rejection is auditable from the
 * artifact — a transcript that silently drops a chunk is not a diagnostic
 * source.
 */
export interface TranscriptRejection {
  reason: HallucinationReason;
  discarded_text: string;
  truncated: boolean;
  discarded_word_count: number;
}

/**
 * How much of a chunk's audio was actually voice, measured at 20 ms resolution
 * against the chunk's own noise floor.
 *
 * `rms` is the measurement the old silence gate used on its own, kept because
 * comparing it against `voiced_seconds` is what makes the old failure legible:
 * steady room tone has a healthy RMS and no voiced time at all.
 */
export interface SpeechProfile {
  voiced_seconds: number;
  total_seconds: number;
  peak_amplitude: number;
  rms: number;
  noise_floor_rms: number;
}

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
  /** Null for transcripts recorded before the speech gate existed. */
  speech?: SpeechProfile | null;
  /** Present exactly when `status` is `REJECTED`. */
  rejection?: TranscriptRejection | null;
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
  /**
   * Loudness of each source over this utterance's span. The booleans above are
   * this measurement already reduced to a verdict, and the reduction throws
   * away what identifying the local user needs: on speakers rather than
   * headphones both sources register on nearly every utterance.
   */
  mic_rms?: number;
  sys_rms?: number;
  /**
   * The speaker the recorder assigned while the meeting was still running.
   * Refined by the global pass afterwards; null where there was too little
   * voice to place the span.
   */
  live_speaker?: number | null;
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

export * from './models';

// ── Foundation Roadmap 11-20 Types ──────────────────────────────────────────

export type RetrievalSourceType =
  | 'voice_note'
  | 'scribble'
  | 'meeting'
  | 'meeting_facts'
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
  | 'meeting'
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


