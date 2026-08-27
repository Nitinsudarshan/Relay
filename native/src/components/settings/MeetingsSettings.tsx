import React, { useState } from 'react';
import { Plus, Trash2 } from 'lucide-react';
import { Switch } from '@/components/ui/switch';
import type {
  AppSettings,
  DefaultSummaryModeSetting,
  MeetingExtensionSetting,
  MeetingSettings,
  SpeakerIdentificationSetting,
} from '../../types';

interface MeetingsSettingsProps {
  settings: AppSettings;
  /** Persists and lifts the change; the parent owns the settings object. */
  onChange: (next: AppSettings) => Promise<void> | void;
}

export const DEFAULT_MEETING_SETTINGS: MeetingSettings = {
  show_raw_transcript: true,
  generate_conversation_transcript: true,
  auto_generate_summary: true,
  default_summary_mode: 'standard',
  default_extension_id: 'default',
  speaker_identification: 'automatic',
  extensions: [],
};

const SUMMARY_MODE_OPTIONS: {
  value: DefaultSummaryModeSetting;
  label: string;
  hint: string;
}[] = [
  { value: 'concise', label: 'Concise', hint: 'Key points only' },
  { value: 'standard', label: 'Standard', hint: 'Recommended' },
  { value: 'detailed', label: 'Detailed', hint: 'More context, still not a transcript' },
];

/**
 * Settings › Meetings.
 *
 * Controls behavior, not the pipeline. The processing pipeline has seven
 * internal stages; none of them appears here. What is exposed is what someone
 * would actually want to change.
 */
export const MeetingsSettings: React.FC<MeetingsSettingsProps> = ({
  settings,
  onChange,
}) => {
  const meetings = settings.meetings ?? DEFAULT_MEETING_SETTINGS;
  const [draftExtension, setDraftExtension] = useState<MeetingExtensionSetting>({
    id: '',
    name: '',
    instructions: '',
  });
  const [extensionError, setExtensionError] = useState<string | null>(null);

  const update = (patch: Partial<MeetingSettings>) =>
    onChange({ ...settings, meetings: { ...meetings, ...patch } });

  const addExtension = () => {
    const name = draftExtension.name.trim();
    const instructions = draftExtension.instructions.trim();
    if (!name || !instructions) {
      setExtensionError('An extension needs a name and instructions.');
      return;
    }

    // Slugged from the name so the id is readable, and suffixed if it collides —
    // ids are referenced by generated summaries and must stay unique.
    const base =
      name
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, '_')
        .replace(/^_+|_+$/g, '') || 'extension';
    const taken = new Set([
      'default',
      'executive_brief',
      'project_update',
      'decision_log',
      ...meetings.extensions.map((e) => e.id),
    ]);
    let id = base;
    let suffix = 2;
    while (taken.has(id)) {
      id = `${base}_${suffix}`;
      suffix += 1;
    }

    setExtensionError(null);
    setDraftExtension({ id: '', name: '', instructions: '' });
    update({ extensions: [...meetings.extensions, { id, name, instructions }] });
  };

  const removeExtension = (id: string) =>
    update({ extensions: meetings.extensions.filter((e) => e.id !== id) });

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h2 className="text-sm font-semibold text-foreground">Meetings</h2>
        <p className="text-[11px] text-muted-foreground mt-1">
          How Relay processes a meeting after it is recorded. Recording, the
          30-second audio chunks, and live transcription are unaffected by anything
          on this page.
        </p>
      </div>

      <div className="flex flex-col gap-4 p-4 rounded-xl border border-border/60 bg-card/40">
        <div className="flex items-center justify-between gap-4">
          <div>
            <p className="text-xs font-medium text-foreground">Show raw transcript</p>
            <p className="text-[11px] text-muted-foreground">
              Offers the Raw Transcript tab — the unedited speech-to-text output, per
              30-second chunk. Turning this off only hides the tab; the transcript
              stays on disk and remains the source for everything Relay derives from
              a meeting.
            </p>
          </div>
          <Switch
            checked={meetings.show_raw_transcript}
            onCheckedChange={(checked) => update({ show_raw_transcript: checked })}
          />
        </div>

        <div className="h-px bg-border/60" />

        <div className="flex items-center justify-between gap-4">
          <div>
            <p className="text-xs font-medium text-foreground">
              Generate conversation transcript
            </p>
            <p className="text-[11px] text-muted-foreground">
              Builds the readable, speaker-labelled version of the transcript. Costs
              nothing — no model is involved.
            </p>
          </div>
          <Switch
            checked={meetings.generate_conversation_transcript}
            onCheckedChange={(checked) =>
              update({ generate_conversation_transcript: checked })
            }
          />
        </div>

        <div className="h-px bg-border/60" />

        <div className="flex items-center justify-between gap-4">
          <div>
            <p className="text-xs font-medium text-foreground">
              Summarize automatically
            </p>
            <p className="text-[11px] text-muted-foreground">
              Starts a summary once a recording finishes. It runs in the background
              and never blocks the recorder or opening the meeting.
            </p>
          </div>
          <Switch
            checked={meetings.auto_generate_summary}
            onCheckedChange={(checked) => update({ auto_generate_summary: checked })}
          />
        </div>

        <div className="h-px bg-border/60" />

        <div className="flex items-center justify-between gap-4">
          <div>
            <p className="text-xs font-medium text-foreground">
              Speaker identification
            </p>
            <p className="text-[11px] text-muted-foreground">
              Tells your microphone apart from everyone else on the call. No
              voiceprints are created and no biometric data is stored.
            </p>
          </div>
          <select
            value={meetings.speaker_identification}
            onChange={(e) =>
              update({
                speaker_identification: e.target
                  .value as SpeakerIdentificationSetting,
              })
            }
            className="text-xs bg-input border border-border rounded-md px-2 py-1.5 text-foreground"
          >
            <option value="automatic">Automatic</option>
            <option value="off">Off</option>
          </select>
        </div>
      </div>

      <div className="flex flex-col gap-3 p-4 rounded-xl border border-border/60 bg-card/40">
        <div>
          <p className="text-xs font-medium text-foreground">Default summary</p>
          <p className="text-[11px] text-muted-foreground">
            How long a summary is by default. You can change it per meeting without
            re-reading the transcript.
          </p>
        </div>
        <div className="flex gap-2">
          {SUMMARY_MODE_OPTIONS.map((option) => (
            <button
              key={option.value}
              type="button"
              onClick={() => update({ default_summary_mode: option.value })}
              className={`flex-1 flex flex-col gap-0.5 px-3 py-2 rounded-lg border text-left transition-all ${
                meetings.default_summary_mode === option.value
                  ? 'border-primary bg-accent text-accent-foreground'
                  : 'border-border/60 hover:bg-muted text-muted-foreground'
              }`}
            >
              <span className="text-xs font-semibold">{option.label}</span>
              <span className="text-[10px] opacity-80">{option.hint}</span>
            </button>
          ))}
        </div>
      </div>

      <div className="flex flex-col gap-3 p-4 rounded-xl border border-border/60 bg-card/40">
        <div>
          <p className="text-xs font-medium text-foreground">Extensions</p>
          <p className="text-[11px] text-muted-foreground">
            Named presentations of the same meeting. An extension changes how the
            summary reads, never what was extracted from the transcript — so two
            extensions of one meeting cannot disagree about what was decided.
          </p>
        </div>

        <div className="flex flex-col gap-1.5">
          {['Default', 'Executive Brief', 'Project Update', 'Decision Log'].map(
            (name) => (
              <div
                key={name}
                className="flex items-center justify-between px-3 py-2 rounded-lg bg-muted/40 border border-border/40"
              >
                <span className="text-xs text-foreground">{name}</span>
                <span className="text-[10px] text-muted-foreground font-mono">
                  built in
                </span>
              </div>
            ),
          )}

          {meetings.extensions.map((extension) => (
            <div
              key={extension.id}
              className="flex items-start justify-between gap-3 px-3 py-2 rounded-lg bg-muted/40 border border-border/40"
            >
              <div className="min-w-0">
                <p className="text-xs text-foreground">{extension.name}</p>
                <p className="text-[10px] text-muted-foreground line-clamp-2">
                  {extension.instructions}
                </p>
              </div>
              <button
                type="button"
                onClick={() => removeExtension(extension.id)}
                className="p-1 rounded-md text-destructive hover:bg-destructive/10 shrink-0"
                aria-label={`Delete the ${extension.name} extension`}
              >
                <Trash2 className="w-3.5 h-3.5" />
              </button>
            </div>
          ))}
        </div>

        <div className="flex flex-col gap-2 pt-1">
          <input
            value={draftExtension.name}
            onChange={(e) =>
              setDraftExtension((d) => ({ ...d, name: e.target.value }))
            }
            placeholder="Extension name, e.g. Board Update"
            className="text-xs bg-input border border-border rounded-md px-2.5 py-1.5 text-foreground placeholder:text-muted-foreground"
          />
          <textarea
            value={draftExtension.instructions}
            onChange={(e) =>
              setDraftExtension((d) => ({ ...d, instructions: e.target.value }))
            }
            placeholder="How should this summary read? e.g. Lead with revenue impact, keep it to five bullets, no procedural detail."
            rows={3}
            className="text-xs bg-input border border-border rounded-md px-2.5 py-1.5 text-foreground placeholder:text-muted-foreground resize-y"
          />
          {extensionError && (
            <p className="text-[11px] text-destructive">{extensionError}</p>
          )}
          <button
            type="button"
            onClick={addExtension}
            className="self-start flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-primary text-primary-foreground text-xs font-medium hover:opacity-90 transition-opacity"
          >
            <Plus className="w-3.5 h-3.5" />
            <span>Add extension</span>
          </button>
        </div>
      </div>
    </div>
  );
};
