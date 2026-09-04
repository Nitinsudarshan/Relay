import React, { useCallback, useEffect, useRef, useState } from 'react';
import {
  AlertTriangle,
  Check,
  ChevronDown,
  Loader2,
  NotebookPen,
  Plus,
  Trash2,
} from 'lucide-react';
import type {
  DirectiveKind,
  MeetingDirective,
  MeetingNotes,
  Speaker,
  UnresolvedDirective,
} from '../../types';

interface MeetingNotesTabProps {
  notes: MeetingNotes | null;
  /** The roster, so a name correction can offer the speakers that exist. */
  speakers: Speaker[];
  /** Instructions the pipeline could not apply, reported back. */
  unresolved: UnresolvedDirective[];
  /** False while the meeting is still loading. */
  isLoaded: boolean;
  onSave: (notes: { during?: string; before?: string }) => Promise<void>;
  onAddDirective: (
    kind: DirectiveKind,
    subject: string | null,
    value: string,
  ) => Promise<void>;
  onRemoveDirective: (directiveId: string) => Promise<void>;
}

/** How long typing has to stop before a save is issued. */
const AUTOSAVE_DELAY_MS = 800;

/**
 * The kinds of note a person can add, in the order they are needed.
 *
 * This list is the whole design of this tab. A paragraph box asks the user to
 * write prose and hope a model acts on it; these ask for the one specific thing
 * each kind needs and hand it to the stage that can actually apply it. A name
 * correction reaches the speaker registry, a misheard term reaches the
 * normalizer, and neither depends on a model noticing a sentence.
 */
const KINDS: {
  kind: DirectiveKind;
  label: string;
  /** What the subject field asks for, when the kind needs one. */
  subjectLabel?: string;
  valueLabel: string;
  hint: string;
  effect: string;
}[] = [
  {
    kind: 'SPEAKER_NAME',
    label: 'Name a speaker',
    subjectLabel: 'Which speaker',
    valueLabel: 'Their name',
    hint: 'Speaker 2 → Pranjali',
    effect: 'Renames them everywhere. No transcript is rewritten.',
  },
  {
    kind: 'TERM',
    label: 'Fix a misheard word',
    subjectLabel: 'What was heard',
    valueLabel: 'What was said',
    hint: 'Lance TV → LanceDB',
    effect: 'Corrected in the readable transcript and the summary.',
  },
  {
    kind: 'PARTICIPANT',
    label: 'Add a participant',
    valueLabel: 'Their name',
    hint: 'Somebody who was there but did not speak',
    effect: 'Listed as a participant, marked as not heard.',
  },
  {
    kind: 'AGENDA',
    label: 'What this was for',
    valueLabel: 'The agenda item',
    hint: 'Decide the launch date',
    effect: 'Read as intent, never as evidence something was decided.',
  },
  {
    kind: 'NOTE',
    label: 'Remember this',
    valueLabel: 'The note',
    hint: 'The vault rewrite is still blocked',
    effect: 'Read when the summary is generated. Never rewritten.',
  },
];

const KIND_LABELS: Record<DirectiveKind, string> = {
  SPEAKER_NAME: 'Name',
  TERM: 'Wording',
  PARTICIPANT: 'Participant',
  AGENDA: 'Agenda',
  NOTE: 'Note',
};

/** One stored directive, with its own delete affordance. */
const DirectiveRow: React.FC<{
  directive: MeetingDirective;
  unresolvedReason?: string;
  onRemove: () => void;
  isBusy: boolean;
}> = ({ directive, unresolvedReason, onRemove, isBusy }) => (
  <li
    className={`group flex items-start gap-2 px-2.5 py-1.5 rounded-md border text-xs ${
      unresolvedReason
        ? 'border-amber-500/30 bg-amber-500/5'
        : 'border-border bg-card'
    }`}
  >
    <span className="shrink-0 px-1.5 py-0.5 rounded bg-muted text-[10px] font-medium text-muted-foreground uppercase tracking-wide">
      {KIND_LABELS[directive.kind]}
    </span>
    <span className="flex-1 min-w-0 text-foreground">
      {directive.subject ? (
        <>
          <span className="text-muted-foreground">{directive.subject}</span>
          <span className="text-muted-foreground mx-1">→</span>
          <span>{directive.value}</span>
        </>
      ) : (
        directive.value
      )}
      {unresolvedReason && (
        <span className="block mt-0.5 text-[11px] text-amber-800 dark:text-amber-200">
          Not applied — {unresolvedReason}
        </span>
      )}
    </span>
    <button
      type="button"
      onClick={onRemove}
      disabled={isBusy}
      className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100 p-1 rounded hover:bg-destructive/20 hover:text-destructive text-muted-foreground transition-all shrink-0 cursor-pointer disabled:opacity-40"
      aria-label={`Remove: ${directive.value}`}
    >
      <Trash2 className="w-3 h-3" />
    </button>
  </li>
);

/**
 * The user's own input on a meeting.
 *
 * Structured first, prose second. Most of what somebody wants to tell Relay
 * about a meeting is a correction of a specific thing — a name the recogniser
 * mangled, a term it did not know, somebody who was in the room — and a
 * paragraph is the wrong shape for that. The paragraph box is still here,
 * below, for the things that genuinely are prose.
 */
export const MeetingNotesTab: React.FC<MeetingNotesTabProps> = ({
  notes,
  speakers,
  unresolved,
  isLoaded,
  onSave,
  onAddDirective,
  onRemoveDirective,
}) => {
  const [during, setDuring] = useState<string>('');
  const [before, setBefore] = useState<string>('');
  const [showBefore, setShowBefore] = useState<boolean>(false);
  const [showProse, setShowProse] = useState<boolean>(false);
  const [isSaving, setIsSaving] = useState<boolean>(false);
  const [savedAt, setSavedAt] = useState<number | null>(null);

  const [kind, setKind] = useState<DirectiveKind>('SPEAKER_NAME');
  const [subject, setSubject] = useState<string>('');
  const [value, setValue] = useState<string>('');
  const [isAdding, setIsAdding] = useState<boolean>(false);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const loadedFor = useRef<string | null>(null);

  useEffect(() => {
    const signature = notes
      ? `${notes.updated_at ?? ''}:${notes.during.length}:${notes.before.length}`
      : null;
    if (loadedFor.current === signature) return;
    loadedFor.current = signature;
    setDuring(notes?.during ?? '');
    setBefore(notes?.before ?? '');
    setShowBefore(Boolean(notes?.before));
    setShowProse(Boolean(notes?.during) || Boolean(notes?.before));
  }, [notes]);

  useEffect(
    () => () => {
      if (saveTimer.current) clearTimeout(saveTimer.current);
    },
    [],
  );

  const scheduleSave = useCallback(
    (next: { during?: string; before?: string }) => {
      if (saveTimer.current) clearTimeout(saveTimer.current);
      saveTimer.current = setTimeout(async () => {
        setIsSaving(true);
        try {
          await onSave(next);
          setSavedAt(Date.now());
        } finally {
          setIsSaving(false);
        }
      }, AUTOSAVE_DELAY_MS);
    },
    [onSave],
  );

  const active = KINDS.find((k) => k.kind === kind) ?? KINDS[0];
  const needsSubject = Boolean(active.subjectLabel);
  const canSubmit =
    value.trim().length > 0 && (!needsSubject || subject.trim().length > 0);

  const submit = async () => {
    if (!canSubmit || isAdding) return;
    setIsAdding(true);
    setError(null);
    try {
      await onAddDirective(kind, needsSubject ? subject.trim() : null, value.trim());
      setSubject('');
      setValue('');
    } catch (err) {
      setError(
        typeof err === 'object' && err !== null && 'message' in err
          ? String((err as { message: unknown }).message)
          : String(err),
      );
    } finally {
      setIsAdding(false);
    }
  };

  const remove = async (directiveId: string) => {
    setBusyId(directiveId);
    try {
      await onRemoveDirective(directiveId);
    } finally {
      setBusyId(null);
    }
  };

  if (!isLoaded) {
    return (
      <div className="flex-1 flex items-center justify-center text-muted-foreground">
        <Loader2 className="w-4 h-4 animate-spin text-primary" />
      </div>
    );
  }

  const directives = notes?.directives ?? [];
  const reasonFor = (id: string) =>
    unresolved.find((u) => u.directive_id === id)?.reason;

  return (
    <div className="flex-1 overflow-y-auto p-6 space-y-5">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2 text-foreground">
          <NotebookPen className="w-4 h-4 text-primary" aria-hidden="true" />
          <h4 className="text-sm font-semibold">Your input</h4>
        </div>
        <span
          className="text-xs text-muted-foreground flex items-center gap-1.5"
          aria-live="polite"
        >
          {isSaving && (
            <>
              <Loader2 className="w-3 h-3 animate-spin text-primary" aria-hidden="true" />
              Saving…
            </>
          )}
          {!isSaving && savedAt && (
            <>
              <Check
                className="w-3 h-3 text-emerald-600 dark:text-emerald-400"
                aria-hidden="true"
              />
              Saved
            </>
          )}
        </span>
      </div>

      <p className="text-xs text-muted-foreground max-w-prose">
        Tell Relay the things the recording could not know. Each kind of note is
        read by the part of Relay that can act on it — a name correction renames
        the speaker, a misheard word is fixed in the readable transcript — so
        none of them depends on a summary being regenerated to take effect.
      </p>

      {/* Add a directive */}
      <div className="space-y-2 p-3 rounded-lg border border-border bg-muted/20">
        <div className="flex flex-wrap gap-1.5">
          {KINDS.map((option) => (
            <button
              key={option.kind}
              type="button"
              onClick={() => {
                setKind(option.kind);
                setSubject('');
                setError(null);
              }}
              className={`px-2.5 py-1 rounded-md border text-xs font-medium transition-colors cursor-pointer ${
                kind === option.kind
                  ? 'border-primary bg-primary/10 text-foreground'
                  : 'border-border bg-card text-muted-foreground hover:bg-accent hover:text-foreground'
              }`}
            >
              {option.label}
            </button>
          ))}
        </div>

        <div className="flex flex-wrap items-end gap-2">
          {needsSubject && (
            <label className="flex flex-col gap-1 min-w-[10rem] flex-1">
              <span className="text-[11px] text-muted-foreground">
                {active.subjectLabel}
              </span>
              {kind === 'SPEAKER_NAME' && speakers.length > 0 ? (
                <select
                  value={subject}
                  onChange={(e) => setSubject(e.target.value)}
                  className="w-full rounded-md bg-background border border-input px-2 py-1.5 text-xs text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                >
                  <option value="">Choose…</option>
                  {speakers.map((speaker) => (
                    <option key={speaker.id} value={speaker.id}>
                      {speaker.display_name?.trim() || speaker.fallback_label}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  value={subject}
                  onChange={(e) => setSubject(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') submit();
                  }}
                  className="w-full rounded-md bg-background border border-input px-2 py-1.5 text-xs text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                />
              )}
            </label>
          )}

          <label className="flex flex-col gap-1 min-w-[12rem] flex-[2]">
            <span className="text-[11px] text-muted-foreground">
              {active.valueLabel}
            </span>
            <input
              value={value}
              onChange={(e) => setValue(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') submit();
              }}
              placeholder={active.hint}
              className="w-full rounded-md bg-background border border-input px-2 py-1.5 text-xs text-foreground placeholder:text-muted-foreground/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            />
          </label>

          <button
            type="button"
            onClick={submit}
            disabled={!canSubmit || isAdding}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-primary hover:bg-primary/90 text-primary-foreground text-xs font-semibold transition-colors disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            {isAdding ? (
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
            ) : (
              <Plus className="w-3.5 h-3.5" />
            )}
            Add
          </button>
        </div>

        <p className="text-[11px] text-muted-foreground">{active.effect}</p>

        {error && (
          <p className="flex items-start gap-1.5 text-[11px] text-amber-800 dark:text-amber-200">
            <AlertTriangle className="w-3.5 h-3.5 shrink-0 mt-px" aria-hidden="true" />
            {error}
          </p>
        )}
      </div>

      {directives.length > 0 && (
        <ul className="space-y-1.5">
          {directives.map((directive) => (
            <DirectiveRow
              key={directive.id}
              directive={directive}
              unresolvedReason={reasonFor(directive.id)}
              onRemove={() => remove(directive.id)}
              isBusy={busyId === directive.id}
            />
          ))}
        </ul>
      )}

      {/* Prose, second and collapsed by default. */}
      <div className="border-t border-border pt-4">
        <button
          type="button"
          onClick={() => setShowProse((v) => !v)}
          aria-expanded={showProse}
          className="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded"
        >
          <ChevronDown
            className={`w-3.5 h-3.5 transition-transform ${showProse ? '' : '-rotate-90'}`}
            aria-hidden="true"
          />
          Write it as a paragraph instead
        </button>

        {showProse && (
          <div className="mt-3 space-y-3">
            <p className="text-xs text-muted-foreground max-w-prose">
              For the things that genuinely are prose. Read when the summary is
              generated, never rewritten, and never overwritten by one.
            </p>

            <label htmlFor="meeting-notes-during" className="sr-only">
              Notes taken during or after this meeting
            </label>
            <textarea
              id="meeting-notes-during"
              value={during}
              onChange={(e) => {
                setDuring(e.target.value);
                scheduleSave({ during: e.target.value });
              }}
              rows={8}
              placeholder="What mattered, what was decided, what you need to remember…"
              className="w-full rounded-md bg-background border border-input px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring resize-y"
            />

            <div>
              <button
                type="button"
                onClick={() => setShowBefore((v) => !v)}
                aria-expanded={showBefore}
                className="text-xs text-muted-foreground hover:text-foreground transition-colors cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded"
              >
                {showBefore ? 'Hide' : 'Add'} notes written before the meeting
              </button>

              {showBefore && (
                <div className="mt-2 space-y-2">
                  <label htmlFor="meeting-notes-before" className="sr-only">
                    Notes written before this meeting
                  </label>
                  <textarea
                    id="meeting-notes-before"
                    value={before}
                    onChange={(e) => {
                      setBefore(e.target.value);
                      scheduleSave({ before: e.target.value });
                    }}
                    rows={5}
                    placeholder="Agenda, questions to ask, context you already have…"
                    className="w-full rounded-md bg-background border border-input px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring resize-y"
                  />
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
