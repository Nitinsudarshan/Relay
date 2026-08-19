export interface ProcessedPipelineResult {
  mode: 'meeting' | 'scribble' | 'trigger' | 'chat';
  transcript: string;
  note_id?: string;
  kanban_cards_created: number;
  output_markdown: string;
  /** Vault note titles used as grounding context (chat mode only). */
  sources: string[];
  /** Base64 WAV of the answer spoken aloud, if local TTS is configured. */
  spoken_audio_base64?: string | null;
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
  /** Path to a GGML Whisper model file (e.g. ggml-base.en.bin). */
  whisper_model_path?: string | null;
}

export interface TtsSettings {
  piper_binary_path?: string | null;
  piper_voice_path?: string | null;
}

export interface HotkeySettings {
  show_hide_hotkey: string;
  dictation_hotkey: string;
}

export interface UiSettings {
  /** Show the "Click to dictate" pill as a floating always-on-top desktop
   * overlay window (outside the main app window) rather than only inline. */
  show_floating_pill: boolean;
}

/** Mirrors the Rust `AppSettings` struct persisted at `.relay/config/settings.json`. */
export interface AppSettings {
  provider: ProviderSettings;
  stt: SttSettings;
  tts: TtsSettings;
  hotkeys: HotkeySettings;
  ui: UiSettings;
}
