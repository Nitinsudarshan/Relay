import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Check, Loader2, NotebookPen } from 'lucide-react';
import type { MeetingNotes } from '../../types';

interface MeetingNotesTabProps {
  notes: MeetingNotes | null;
  /** False while the meeting is still loading. */
  isLoaded: boolean;
  onSave: (notes: { during?: string; before?: string }) => Promise<void>;
}

/** How long typing has to stop before a save is issued. */
const AUTOSAVE_DELAY_MS = 800;

/**
 * The notes a person writes about a meeting.
 *
 * The cheapest quality signal Relay has: three bullets somebody typed while a
 * ninety-minute call was happening outperform any amount of prompt tuning at
 * telling the extraction stage which part of it mattered. Notes are a *source*
 * artifact — saving one never regenerates anything, and generating a summary
 * never edits one.
 *
 * Two fields, because the pipeline treats them differently. The first is the
 * normal case. The second is deliberately tucked behind a disclosure: writing an
 * agenda in advance is rare, and a form that leads with it would imply Relay
 * needs one.
 */
export const MeetingNotesTab: React.FC<MeetingNotesTabProps> = ({
  notes,
  isLoaded,
  onSave,
}) => {
  const [during, setDuring] = useState<string>('');
  const [before, setBefore] = useState<string>('');
  const [showBefore, setShowBefore] = useState<boolean>(false);
  const [isSaving, setIsSaving] = useState<boolean>(false);
  const [savedAt, setSavedAt] = useState<number | null>(null);

  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Which meeting's notes are in the boxes, so switching meetings replaces the
  // text rather than letting a pending save write it into the wrong one.
  const loadedFor = useRef<string | null>(null);

  useEffect(() => {
    const signature = notes ? `${notes.updated_at ?? ''}:${notes.during.length}:${notes.before.length}` : null;
    if (loadedFor.current === signature) return;
    loadedFor.current = signature;
    setDuring(notes?.during ?? '');
    setBefore(notes?.before ?? '');
    setShowBefore(Boolean(notes?.before));
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

  if (!isLoaded) {
    return (
      <div className="flex-1 flex items-center justify-center text-zinc-500">
        <Loader2 className="w-4 h-4 animate-spin" />
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto p-6 space-y-4">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2 text-zinc-300">
          <NotebookPen className="w-4 h-4" aria-hidden="true" />
          <h4 className="text-sm font-semibold">Your notes</h4>
        </div>
        <span
          className="text-xs text-zinc-500 flex items-center gap-1.5"
          aria-live="polite"
        >
          {isSaving && (
            <>
              <Loader2 className="w-3 h-3 animate-spin" aria-hidden="true" />
              Saving…
            </>
          )}
          {!isSaving && savedAt && (
            <>
              <Check className="w-3 h-3 text-lime-400" aria-hidden="true" />
              Saved
            </>
          )}
        </span>
      </div>

      <p className="text-xs text-zinc-400 max-w-prose">
        Anything you type here is read when the summary is generated. It is not a
        second transcript — it tells Relay what mattered, corrects a name the
        recogniser mangled, and keeps something you want remembered. Your notes
        are never rewritten, and never overwritten by a summary.
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
        rows={14}
        placeholder="What mattered, what was decided, what you need to remember…"
        className="w-full rounded-md bg-white/5 border border-white/10 px-3 py-2 text-sm text-zinc-100 placeholder:text-zinc-600 focus-visible:outline focus-visible:outline-2 focus-visible:outline-lime-400 resize-y"
      />

      <div>
        <button
          type="button"
          onClick={() => setShowBefore((v) => !v)}
          aria-expanded={showBefore}
          className="text-xs text-zinc-500 hover:text-zinc-300 transition-colors cursor-pointer focus-visible:outline focus-visible:outline-2 focus-visible:outline-lime-400 rounded"
        >
          {showBefore ? 'Hide' : 'Add'} notes written before the meeting
        </button>

        {showBefore && (
          <div className="mt-2 space-y-2">
            <p className="text-xs text-zinc-500 max-w-prose">
              An agenda or the questions you wanted to ask. Optional — Relay uses
              it to understand what the meeting was <em>for</em>, never as
              evidence that something was decided.
            </p>
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
              className="w-full rounded-md bg-white/5 border border-white/10 px-3 py-2 text-sm text-zinc-100 placeholder:text-zinc-600 focus-visible:outline focus-visible:outline-2 focus-visible:outline-lime-400 resize-y"
            />
          </div>
        )}
      </div>
    </div>
  );
};
