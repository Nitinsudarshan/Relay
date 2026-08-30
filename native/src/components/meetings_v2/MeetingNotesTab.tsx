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
      <div className="flex-1 flex items-center justify-center text-muted-foreground">
        <Loader2 className="w-4 h-4 animate-spin text-primary" />
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto p-6 space-y-4">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2 text-foreground">
          <NotebookPen className="w-4 h-4 text-primary" aria-hidden="true" />
          <h4 className="text-sm font-semibold">Your notes</h4>
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
              <Check className="w-3 h-3 text-emerald-600 dark:text-emerald-400" aria-hidden="true" />
              Saved
            </>
          )}
        </span>
      </div>

      <p className="text-xs text-muted-foreground max-w-prose">
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
            <p className="text-xs text-muted-foreground max-w-prose">
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
              className="w-full rounded-md bg-background border border-input px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring resize-y"
            />
          </div>
        )}
      </div>
    </div>
  );
};

