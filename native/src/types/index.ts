export interface ProcessedPipelineResult {
  mode: 'scribble' | 'trigger' | 'chat';
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

export interface HotkeySettings {
  show_hide_hotkey: string;
  dictation_hotkey: string;
  /** One press starts recording, a second press stops it, instead of
   * holding the key down the whole time. Defaults to false (hold-to-talk). */
  toggle_to_talk: boolean;
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
  total_audio_bytes: number;
  transcript_segment_count: number;
  pending_transcription_chunks: number;
  error_message?: string | null;
}

export type TranscriptSegmentStatus = 'SUCCESS' | 'EMPTY' | 'FAILED';

export interface TranscriptSegment {
  chunk_index: number;
  start_time_s: number;
  end_time_s: number;
  text: string;
  created_at: string;
  status: TranscriptSegmentStatus;
}

export interface LiveTranscriptUpdate {
  segment_id: string;
  session_id: string;
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
  mic_rms: number;
  sys_rms: number;
  error?: string | null;
}





