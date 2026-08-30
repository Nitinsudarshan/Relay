import React from 'react';
import { Switch } from '@/components/ui/switch';
import type {
  AppSettings,
  TalkbackSettings,
  TalkbackSourceType,
} from '../../types';

interface TalkbackSettingsViewProps {
  settings: AppSettings;
  /** Persists and lifts the change; the parent owns the settings object. */
  onChange: (next: AppSettings) => Promise<void> | void;
}

export const DEFAULT_TALKBACK_SETTINGS: TalkbackSettings = {
  activation_mode: 'toggle',
  speak_responses: true,
  allow_barge_in: true,
  sources: [],
  end_of_turn_silence_ms: 700,
};

const SOURCE_OPTIONS: { value: TalkbackSourceType; label: string; hint: string }[] = [
  {
    value: 'MEETING_FACTS',
    label: 'Meeting intelligence',
    hint: 'Decisions, action items and key points — the strongest source',
  },
  { value: 'SCRIBBLE', label: 'Scribbles', hint: 'Your structured thoughts' },
  { value: 'MEETING', label: 'Meeting summaries', hint: 'Generated meeting prose' },
  { value: 'VOICE_NOTE', label: 'Voice Notes', hint: 'Verbatim dictation history' },
];

/** Silence before Talkback decides you have finished speaking. */
const SILENCE_OPTIONS = [
  { value: 400, label: 'Snappy', hint: '0.4s — may cut off a pause' },
  { value: 700, label: 'Natural', hint: '0.7s — recommended' },
  { value: 1200, label: 'Patient', hint: '1.2s — for slower speech' },
];

/**
 * Settings › Talkback.
 *
 * Deliberately small. Everything here is a real preference with a visible
 * effect; the pipeline's internals (retrieval weights, phrase length,
 * context budget) are not settings, they are decisions with reasons
 * recorded in `docs/talkback/ARCHITECTURE.md`.
 */
export const TalkbackSettingsView: React.FC<TalkbackSettingsViewProps> = ({
  settings,
  onChange,
}) => {
  const talkback = settings.talkback ?? DEFAULT_TALKBACK_SETTINGS;

  const update = (patch: Partial<TalkbackSettings>) => {
    void onChange({ ...settings, talkback: { ...talkback, ...patch } });
  };

  const toggleSource = (source: TalkbackSourceType) => {
    const current = talkback.sources ?? [];
    // Empty means "all" in the backend, so unchecking the last remaining
    // source would silently re-enable everything. Materialize the full
    // list first, then remove.
    const materialized =
      current.length === 0 ? SOURCE_OPTIONS.map((o) => o.value) : current;
    const next = materialized.includes(source)
      ? materialized.filter((s) => s !== source)
      : [...materialized, source];
    update({ sources: next });
  };

  const isSourceEnabled = (source: TalkbackSourceType) =>
    (talkback.sources?.length ?? 0) === 0 || talkback.sources.includes(source);

  return (
    <div className="space-y-8 max-w-2xl">
      <section className="space-y-3">
        <div>
          <h3 className="text-sm font-semibold text-foreground">Activation</h3>
          <p className="text-xs text-muted-foreground mt-1">
            Talkback is switched on from its own page. Relay deliberately adds
            no third global keyboard shortcut for it.
          </p>
        </div>
        <div className="rounded-lg border border-border bg-muted/40 px-3 py-2.5">
          <p className="text-xs font-medium text-foreground">Toggle</p>
          <p className="text-[11px] text-muted-foreground mt-0.5">
            Wake-word activation is designed for but not built — no always-on
            listener ships today.
          </p>
        </div>
      </section>

      <section className="space-y-4">
        <h3 className="text-sm font-semibold text-foreground">Conversation</h3>

        <label className="flex items-start justify-between gap-4 cursor-pointer">
          <span>
            <span className="block text-xs font-medium text-foreground">
              Speak answers aloud
            </span>
            <span className="block text-[11px] text-muted-foreground mt-0.5">
              Needs a local TTS engine configured under General. Without one,
              answers stay text-only.
            </span>
          </span>
          <Switch
            checked={talkback.speak_responses}
            onCheckedChange={(checked) => update({ speak_responses: checked })}
          />
        </label>

        <label className="flex items-start justify-between gap-4 cursor-pointer">
          <span>
            <span className="block text-xs font-medium text-foreground">
              Let me interrupt
            </span>
            <span className="block text-[11px] text-muted-foreground mt-0.5">
              Speaking over Relay stops it mid-sentence. On laptop speakers it
              may hear itself — headphones make this reliable.
            </span>
          </span>
          <Switch
            checked={talkback.allow_barge_in}
            onCheckedChange={(checked) => update({ allow_barge_in: checked })}
          />
        </label>

        <div className="space-y-2">
          <p className="text-xs font-medium text-foreground">
            Pause before Relay answers
          </p>
          <div className="grid grid-cols-3 gap-2">
            {SILENCE_OPTIONS.map((option) => (
              <button
                key={option.value}
                type="button"
                onClick={() => update({ end_of_turn_silence_ms: option.value })}
                className={`rounded-lg border px-3 py-2 text-left transition-colors ${
                  talkback.end_of_turn_silence_ms === option.value
                    ? 'border-primary bg-accent text-accent-foreground'
                    : 'border-border hover:bg-muted'
                }`}
              >
                <span className="block text-xs font-medium">{option.label}</span>
                <span className="block text-[10px] text-muted-foreground mt-0.5">
                  {option.hint}
                </span>
              </button>
            ))}
          </div>
        </div>
      </section>

      <section className="space-y-3">
        <div>
          <h3 className="text-sm font-semibold text-foreground">Memory sources</h3>
          <p className="text-xs text-muted-foreground mt-1">
            What Talkback may read when answering. Turning everything off leaves
            it with nothing to ground a question about your own history in.
          </p>
        </div>
        <div className="space-y-2">
          {SOURCE_OPTIONS.map((option) => (
            <label
              key={option.value}
              className="flex items-start justify-between gap-4 cursor-pointer rounded-lg border border-border px-3 py-2.5"
            >
              <span>
                <span className="block text-xs font-medium text-foreground">
                  {option.label}
                </span>
                <span className="block text-[11px] text-muted-foreground mt-0.5">
                  {option.hint}
                </span>
              </span>
              <Switch
                checked={isSourceEnabled(option.value)}
                onCheckedChange={() => toggleSource(option.value)}
              />
            </label>
          ))}
        </div>
      </section>

      <section className="rounded-lg border border-border bg-muted/40 px-3 py-2.5">
        <p className="text-xs font-medium text-foreground">Privacy</p>
        <p className="text-[11px] text-muted-foreground mt-1 leading-relaxed">
          With Talkback off, no Talkback microphone stream exists at all. When
          it is on, retrieved excerpts go wherever your configured LLM provider
          goes — localhost with Ollama, that vendor with a cloud provider.
          Conversations are never saved unless you ask for a Voice Note or a
          Scribble.
        </p>
      </section>
    </div>
  );
};
