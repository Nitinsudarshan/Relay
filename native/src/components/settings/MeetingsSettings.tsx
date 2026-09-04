import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { CalendarDays, Plus, Trash2 } from 'lucide-react';
import { Switch } from '@/components/ui/switch';
import type {
  AppSettings,
  CalendarConnection,
  DefaultSummaryModeSetting,
  DiarizationEngineId,
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
  identify_individual_speakers: true,
  expected_speakers: null,
  diarization_engine: 'VOICEPRINT',
  meetings_are_in_person: false,
  extensions: [],
  summary_instructions: '',
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

const formatRelativeSync = (iso: string): string => {
  const diffMs = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diffMs / 60000);
  if (mins < 1) return 'just now';
  if (mins === 1) return '1 minute ago';
  if (mins < 60) return `${mins} minutes ago`;
  const hours = Math.floor(mins / 60);
  return hours === 1 ? '1 hour ago' : `${hours} hours ago`;
};

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
  const [calendar, setCalendar] = useState<CalendarConnection | null>(null);
  const [calendarBusy, setCalendarBusy] = useState(false);
  const [calendarError, setCalendarError] = useState<string | null>(null);

  const loadCalendar = useCallback(async () => {
    try {
      setCalendar(await invoke<CalendarConnection>('get_calendar_connection'));
    } catch {
      // A connection that cannot be read is reported as disconnected rather
      // than as an error: nothing is broken until someone tries to use it.
      setCalendar({ connected: false });
    }
  }, []);

  useEffect(() => {
    void loadCalendar();
  }, [loadCalendar]);

  const connectCalendar = async () => {
    setCalendarBusy(true);
    setCalendarError(null);
    try {
      setCalendar(await invoke<CalendarConnection>('connect_google_calendar'));
    } catch (error) {
      setCalendarError(
        error instanceof Error ? error.message : String(error ?? 'Connecting failed.'),
      );
    } finally {
      setCalendarBusy(false);
    }
  };

  const disconnectCalendar = async () => {
    setCalendarBusy(true);
    setCalendarError(null);
    try {
      setCalendar(await invoke<CalendarConnection>('disconnect_google_calendar'));
    } catch (error) {
      setCalendarError(
        error instanceof Error ? error.message : String(error ?? 'Disconnecting failed.'),
      );
    } finally {
      setCalendarBusy(false);
    }
  };

  const syncCalendar = async () => {
    setCalendarBusy(true);
    setCalendarError(null);
    try {
      setCalendar(await invoke<CalendarConnection>('sync_google_calendar'));
    } catch (error) {
      setCalendarError(
        error instanceof Error ? error.message : String(error ?? 'Syncing failed.'),
      );
    } finally {
      setCalendarBusy(false);
    }
  };

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

      {/* Google Calendar Context */}
      <div className="flex flex-col gap-3 p-4 rounded-lg border border-border/60 bg-card/40">
        <div>
          <div className="flex items-center gap-2">
            <CalendarDays className="w-4 h-4 text-primary" />
            <p className="text-xs font-medium text-foreground">Google Calendar</p>
          </div>
          <p className="text-[11px] text-muted-foreground mt-0.5">
            Read-only contextual evidence for meeting titles and candidate attendee rosters. Relay never modifies your calendar or records events automatically.
          </p>
        </div>

        {calendar?.connected ? (
          <div className="flex flex-col gap-3 pt-1">
            <div className="flex items-center justify-between p-3 rounded-lg bg-emerald-500/10 border border-emerald-500/20">
              <div className="flex items-center gap-2.5">
                <div className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
                <div>
                  <p className="text-xs font-medium text-foreground">✓ Google Calendar connected</p>
                  <p className="text-[11px] text-muted-foreground">
                    Account: {calendar.account_email ?? 'Connected'}
                    {calendar.last_synced_at && (
                      <span className="ml-2 font-mono">
                        · Last synced: {formatRelativeSync(calendar.last_synced_at)}
                      </span>
                    )}
                  </p>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={syncCalendar}
                  disabled={calendarBusy}
                  className="px-2.5 py-1 text-xs font-medium rounded-md bg-secondary hover:bg-secondary/80 text-foreground border border-border cursor-pointer transition-colors"
                >
                  {calendarBusy ? 'Syncing…' : 'Sync now'}
                </button>
                <button
                  type="button"
                  onClick={disconnectCalendar}
                  disabled={calendarBusy}
                  className="px-2.5 py-1 text-xs font-medium rounded-md text-destructive hover:bg-destructive/10 border border-destructive/20 cursor-pointer transition-colors"
                >
                  Disconnect
                </button>
              </div>
            </div>
            {calendar.problem && (
              <p className="text-[11px] text-amber-600 dark:text-amber-400">
                {calendar.problem}
              </p>
            )}
          </div>
        ) : (
          <div className="flex items-center justify-between p-3 rounded-lg bg-muted/40 border border-border/40">
            <div>
              <p className="text-xs text-foreground font-medium">Not connected</p>
              <p className="text-[11px] text-muted-foreground">
                Connect your primary Google Calendar to auto-match recorded meetings.
              </p>
            </div>
            <button
              type="button"
              onClick={connectCalendar}
              disabled={calendarBusy}
              className="px-3 py-1.5 rounded-lg bg-primary text-primary-foreground text-xs font-medium hover:opacity-90 transition-opacity cursor-pointer"
            >
              {calendarBusy ? 'Connecting…' : 'Connect Google Calendar'}
            </button>
          </div>
        )}

        {calendarError && (
          <p className="text-[11px] text-destructive">{calendarError}</p>
        )}
      </div>

      <div className="flex flex-col gap-4 p-4 rounded-lg border border-border/60 bg-card/40">
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

        <div className="flex items-center justify-between gap-4">
          <div>
            <p className="text-xs font-medium text-foreground">
              Separate individual speakers
            </p>
            <p className="text-[11px] text-muted-foreground">
              Tells the people on the call apart from each other, not just from
              you, so a meeting of twenty does not read as one “Speaker 1”. Runs
              once after recording ends. Voice features are used for that run and
              never stored, so no voiceprint is created.
            </p>
          </div>
          <Switch
            checked={meetings.identify_individual_speakers}
            onCheckedChange={(checked) =>
              update({ identify_individual_speakers: checked })
            }
            disabled={meetings.speaker_identification === 'off'}
            aria-label="Separate individual speakers"
          />
        </div>

        {meetings.identify_individual_speakers &&
          meetings.speaker_identification !== 'off' && (
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="text-xs font-medium text-foreground">
                  How speakers are told apart
                </p>
                <p className="text-[11px] text-muted-foreground">
                  Three methods, because they fail differently. Diagnostics ›
                  Meeting Pipeline runs all three over one recording so this can
                  be chosen on evidence rather than guesswork.
                </p>
              </div>
              <select
                value={meetings.diarization_engine ?? 'VOICEPRINT'}
                onChange={(e) =>
                  update({
                    diarization_engine: e.target.value as DiarizationEngineId,
                  })
                }
                className="text-xs bg-input border border-border rounded-md px-2 py-1.5 text-foreground"
                aria-label="How speakers are told apart"
              >
                <option value="VOICEPRINT">Voice separation</option>
                <option value="LIVE">Live (as recorded)</option>
                <option value="CHANNEL">Channel only</option>
              </select>
            </div>
          )}

        {meetings.speaker_identification !== 'off' && (
          <div className="flex items-center justify-between gap-4">
            <div>
              <p className="text-xs font-medium text-foreground">
                Everyone shares one microphone
              </p>
              <p className="text-[11px] text-muted-foreground">
                For meetings held in a room rather than on a call. Relay stops
                trying to work out which voice is yours, because the channel
                split that normally finds it means nothing when every voice
                arrives on the same input — name yourself in the conversation
                tab instead.
              </p>
            </div>
            <Switch
              checked={meetings.meetings_are_in_person ?? false}
              onCheckedChange={(checked) =>
                update({ meetings_are_in_person: checked })
              }
              aria-label="Everyone shares one microphone"
            />
          </div>
        )}

        {meetings.identify_individual_speakers &&
          meetings.speaker_identification !== 'off' && (
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="text-xs font-medium text-foreground">
                  Expected speakers
                </p>
                <p className="text-[11px] text-muted-foreground">
                  A hint, not a target. Leave it on Auto unless the count is
                  known — and note it cannot conjure a speaker the recording
                  does not contain: twenty in the room and three on the audio is
                  still three.
                </p>
              </div>
              <input
                type="number"
                min={0}
                max={12}
                value={meetings.expected_speakers ?? ''}
                placeholder="Auto"
                onChange={(e) => {
                  const parsed = Number.parseInt(e.target.value, 10);
                  update({
                    expected_speakers:
                      Number.isFinite(parsed) && parsed > 0 ? parsed : null,
                  });
                }}
                className="w-20 text-xs bg-input border border-border rounded-md px-2 py-1.5 text-foreground"
                aria-label="Expected number of speakers"
              />
            </div>
          )}
      </div>

      <div className="flex flex-col gap-3 p-4 rounded-lg border border-border/60 bg-card/40">
        <div className="flex items-start justify-between gap-4">
          <div>
            <p className="text-xs font-medium text-foreground flex items-center gap-1.5">
              <CalendarDays className="w-3.5 h-3.5" />
              Google Calendar
            </p>
            <p className="text-[11px] text-muted-foreground mt-1">
              Read-only. Gives a recording the name it was invited under, the
              people who were invited to it, and whatever the agenda said —
              three things audio alone cannot supply. Relay cannot create, move
              or delete anything on your calendar.
            </p>
          </div>
          <button
            type="button"
            onClick={calendar?.connected ? disconnectCalendar : connectCalendar}
            disabled={calendarBusy}
            className={`shrink-0 px-3 py-1.5 rounded-lg text-xs font-medium transition-opacity disabled:opacity-50 ${
              calendar?.connected
                ? 'border border-border text-foreground hover:bg-muted'
                : 'bg-primary text-primary-foreground hover:opacity-90'
            }`}
          >
            {calendarBusy
              ? 'Working…'
              : calendar?.connected
                ? 'Disconnect'
                : 'Connect'}
          </button>
        </div>

        {calendar?.connected && (
          <p className="text-[11px] text-muted-foreground">
            Connected as{' '}
            <span className="text-foreground">
              {calendar.account_email ?? calendar.account_name ?? 'your Google account'}
            </span>
            . Open a finished meeting and choose “Match to calendar” to attach
            its event.
          </p>
        )}
        {calendar?.problem && (
          <p className="text-[11px] text-destructive">{calendar.problem}</p>
        )}
        {calendarError && (
          <p className="text-[11px] text-destructive">{calendarError}</p>
        )}
      </div>

      <div className="flex flex-col gap-3 p-4 rounded-lg border border-border/60 bg-card/40">
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

      <div className="flex flex-col gap-3 p-4 rounded-lg border border-border/60 bg-card/40">
        <div>
          <label
            htmlFor="summary-instructions"
            className="text-xs font-medium text-foreground"
          >
            Summary instructions
          </label>
          <p className="text-[11px] text-muted-foreground">
            How you want your summaries written — what to lead with, how formal
            to be, what you always care about. These shape presentation only:
            they cannot make Relay record a decision, an owner, or a deadline
            your meeting did not establish.
          </p>
        </div>
        <textarea
          id="summary-instructions"
          value={meetings.summary_instructions}
          onChange={(e) => update({ summary_instructions: e.target.value })}
          rows={3}
          placeholder="e.g. Lead with anything that affects the release date. Keep it blunt."
          className="w-full rounded-lg bg-muted/40 border border-border/60 px-3 py-2 text-xs text-foreground placeholder:text-muted-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary resize-y"
        />
      </div>

      <div className="flex flex-col gap-3 p-4 rounded-lg border border-border/60 bg-card/40">
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
