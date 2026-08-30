/**
 * Typed builders for meeting domain objects.
 *
 * These exist so a test can say what it is actually about — "an action item
 * owned by a speaker who has been renamed" — without restating twenty
 * irrelevant fields. Every factory returns a complete, valid object and takes
 * a `Partial` override, so adding a required field to a type breaks here once
 * rather than in every test.
 */
import type {
  ActionItem,
  MeetingFacts,
  MeetingProcessing,
  MeetingSession,
  NormalizedSegment,
  NormalizedTranscript,
  Speaker,
  StageState,
  StageStates,
} from '../types';

export const makeSpeaker = (overrides: Partial<Speaker> = {}): Speaker => ({
  id: 'spk_1',
  display_name: null,
  fallback_label: 'Speaker 1',
  origin: 'CHANNEL',
  channel: 'MIC',
  is_local_user: false,
  segment_count: 4,
  ...overrides,
});

export const makeActionItem = (overrides: Partial<ActionItem> = {}): ActionItem => ({
  id: 'act_1',
  description: 'Send the revised deck',
  owner_type: 'SPEAKER',
  owner_speaker_id: 'spk_1',
  owner_label: null,
  deadline: null,
  status: 'OPEN',
  source_segment_ids: ['seg_1'],
  confidence: 0.9,
  kanban_card_id: null,
  ...overrides,
});

export const makeSession = (overrides: Partial<MeetingSession> = {}): MeetingSession => ({
  id: 'mtg_1',
  title: 'Meeting - Aug 26, 2026 02:03PM',
  state: 'COMPLETED',
  created_at: '2026-08-26T14:03:00Z',
  updated_at: '2026-08-26T14:48:00Z',
  started_at: '2026-08-26T14:03:00Z',
  ended_at: '2026-08-26T14:48:00Z',
  duration_seconds: 2700,
  chunk_count: 12,
  mic_active: true,
  sys_audio_active: true,
  mic_heard: true,
  sys_audio_heard: true,
  paused_seconds: 0,
  capture_warning: null,
  total_audio_bytes: 1024,
  transcript_segment_count: 40,
  pending_transcription_chunks: 0,
  error_message: null,
  ...overrides,
});

const makeStage = (overrides: Partial<StageState> = {}): StageState => ({
  status: 'SUCCESS',
  started_at: null,
  finished_at: null,
  duration_ms: null,
  error: null,
  provider: null,
  model: null,
  input_chars: null,
  output_chars: null,
  validation: null,
  action_diagnostics: null,
  ...overrides,
});

export const makeStages = (overrides: Partial<StageStates> = {}): StageStates => ({
  normalization: makeStage(),
  speakers: makeStage(),
  conversation: makeStage(),
  extraction: makeStage(),
  summary: makeStage(),
  ...overrides,
});

export const makeSegment = (overrides: Partial<NormalizedSegment> = {}): NormalizedSegment => ({
  id: 'seg_1',
  chunk_index: 0,
  utterance_index: 0,
  start_time_s: 0,
  end_time_s: 4,
  text: 'We should ship on Friday.',
  raw_text: 'we should ship on friday',
  channel: 'MIC',
  speaker_id: 'spk_1',
  applied_rules: [],
  ...overrides,
});

export const makeNormalized = (
  overrides: Partial<NormalizedTranscript> = {},
): NormalizedTranscript => ({
  segments: [makeSegment()],
  rule_hits: {},
  source_char_count: 100,
  output_char_count: 90,
  dropped_segment_count: 0,
  ...overrides,
});

export const makeFacts = (overrides: Partial<MeetingFacts> = {}): MeetingFacts => ({
  title: 'Pricing for the enterprise tier',
  meeting_type: 'GENERAL',
  key_points: [],
  topics: [],
  decisions: [],
  action_items: [],
  open_questions: [],
  risks: [],
  entities: [],
  speaker_ids: ['spk_1'],
  deterministic: false,
  ...overrides,
});

export const makeProcessing = (
  overrides: Partial<MeetingProcessing> = {},
): MeetingProcessing => ({
  meeting_id: 'mtg_1',
  processing_version: 2,
  rules_version: '2026-08-27',
  updated_at: '2026-08-26T14:50:00Z',
  status: 'READY',
  stages: makeStages(),
  normalized: makeNormalized(),
  speakers: [makeSpeaker()],
  conversation: null,
  facts: makeFacts(),
  summary: null,
  scribble_ref: null,
  ...overrides,
});
