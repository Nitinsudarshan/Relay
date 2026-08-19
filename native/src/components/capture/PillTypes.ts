export type PillState =
  | 'hidden_notch'
  | 'collapsed'
  | 'expanded'
  | 'listening'
  | 'transcribing'
  | 'processing'
  | 'success'
  | 'error'
  | 'warning';

export type CleanupStyle = 'faithful' | 'clean' | 'professional' | 'concise';
export type SpeechLanguage = 'english' | 'hinglish' | 'auto';

export interface WhisperStatusInfo {
  status: 'ready' | 'download_required' | 'downloading' | 'failed' | 'checking';
  modelPath?: string;
  message?: string;
}

export interface OllamaStatusInfo {
  status: 'ready' | 'not_installed' | 'model_missing' | 'unreachable' | 'checking' | 'cloud_active';
  host: string;
  model: string;
  message?: string;
}

export interface HotkeyStatusInfo {
  status: 'registered' | 'conflict' | 'checking';
  hotkey: string;
  message?: string;
}

export interface DiagnosticInfo {
  state: PillState;
  sttStatus: string;
  llmStatus: string;
  hotkeyStatus: string;
  windowMode: 'resting' | 'expanded' | 'popover';
  promptMode: boolean;
  activeApp: string;
}
