export type ModelStatus =
  | 'ready'
  | 'active'
  | 'available'
  | 'missing'
  | 'unavailable'
  | 'checking';

export interface ModelDescriptor {
  name: string;
  displayName: string;
  provider: 'ollama' | 'cloud_openai' | 'cloud_gemini' | 'cloud_anthropic' | 'whisper';
  type: 'llm' | 'stt';
  installed: boolean;
  available: boolean;
  usable: boolean;
  active: boolean;
  status: ModelStatus;
  details?: string;
  sizeBytes?: number;
  path?: string;
  capabilities?: string[];
}

export interface OllamaModelDetails {
  name: string;
  model: string;
  size?: number | null;
  digest?: string | null;
  modified_at?: string | null;
  parameter_size?: string | null;
  quantization_level?: string | null;
  format?: string | null;
  family?: string | null;
}

export interface OllamaPromptTestResult {
  success: boolean;
  latency_ms: number;
  response?: string | null;
  error?: string | null;
  model: string;
}

export interface SttModelInfo {
  name: string;
  filename: string;
  path: string;
  size_bytes: number;
  exists: boolean;
  is_managed: boolean;
  profile?: 'fast' | 'accurate' | 'custom' | null;
  status: 'ready' | 'available' | 'missing' | string;
}

export interface SttModelsOverview {
  active_model_name: string;
  active_model_path: string;
  active_profile: 'fast' | 'accurate' | 'custom' | string;
  models_dir: string;
  models: SttModelInfo[];
}

export interface SttModelTestResult {
  success: boolean;
  path: string;
  size_bytes: number;
  latency_ms: number;
  error?: string | null;
}
