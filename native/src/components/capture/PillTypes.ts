export type PillState =
  | 'hidden_notch'
  | 'collapsed'
  | 'expanded'
  | 'listening'
  | 'transcribing'
  | 'processing'
  | 'success'
  /// Showing what the Tier 2 cleanup proposes, as a diff, awaiting a decision.
  | 'cleanup'
  | 'error'
  | 'warning';

export type CleanupStyle = 'faithful' | 'clean' | 'professional' | 'concise';
export type SpeechLanguage = 'english' | 'hinglish' | 'hindi' | 'es' | 'auto';

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
  activeApp: string;
}

/** One span of the comparison between what was dictated and what cleanup
 *  proposes. `same` is byte-identical; a whitespace-only change is reported
 *  rather than absorbed. */
export type DiffSpan =
  | { kind: 'same'; text: string }
  | { kind: 'removed'; text: string }
  | { kind: 'added'; text: string };

/** A proposed cleanup and the evidence of what it changed. `changed` is false
 *  when the model returned the text as-is, which is the common case on clean
 *  dictation and not worth interrupting anybody for. */
export interface RewriteProposal {
  original: string;
  rewritten: string;
  spans: DiffSpan[];
  changed: boolean;
}

export interface CleanupTarget {
  available: boolean;
  text: string;
}

export type CleanupApplied =
  | { state: 'replaced' }
  | { state: 'moved'; message: string }
  | { state: 'failed'; message: string };
