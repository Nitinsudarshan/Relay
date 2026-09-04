import React, { useState } from 'react';
import { CalendarDays, Link2, Loader2 } from 'lucide-react';
import type { EventMatch, MeetingCalendarLink, NoMatchReason } from '../../types';

interface MeetingCalendarLinkProps {
  link: MeetingCalendarLink | null;
  /** False when no calendar is connected — the control explains rather than fails. */
  isConnected: boolean;
  onMatch: () => Promise<void>;
  onChoose: (eventId: string | null) => Promise<void>;
}

/** What to say when nothing was matched. Each reason needs a different action. */
const NO_MATCH_COPY: Record<NoMatchReason, string> = {
  NOTHING_SCHEDULED: 'Nothing in your calendar overlapped this recording.',
  TOO_LITTLE_OVERLAP:
    'Nothing in your calendar lines up closely enough with this recording. Anything near it is below — pick one if it was the meeting.',
  AMBIGUOUS:
    'More than one event fits this recording equally well, so Relay did not choose. Pick the one it was.',
};

function whenLabel(event: EventMatch['event']): string {
  const start = new Date(event.starts_at);
  const end = new Date(event.ends_at);
  return `${start.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })}–${end.toLocaleTimeString(
    [],
    { hour: 'numeric', minute: '2-digit' },
  )}`;
}

const CandidateRow: React.FC<{
  candidate: EventMatch;
  onChoose: () => void;
  isBusy: boolean;
}> = ({ candidate, onChoose, isBusy }) => (
  <li className="flex items-center gap-2 text-[11px]">
    <button
      type="button"
      onClick={onChoose}
      disabled={isBusy}
      className="flex-1 min-w-0 text-left px-2 py-1 rounded-md border border-border bg-card hover:bg-accent transition-colors cursor-pointer disabled:opacity-50"
    >
      <span className="text-foreground">{candidate.event.title}</span>
      <span className="text-muted-foreground">
        {' '}
        · {whenLabel(candidate.event)} · {Math.round(candidate.overlap * 100)}% overlap
      </span>
    </button>
  </li>
);

/**
 * The calendar event a recording was, on the meeting itself.
 *
 * A recording knows there were three voices; only the calendar knows the
 * meeting was the placement review, that Ayush was invited, and that it existed
 * to decide a launch date.
 *
 * Where Relay could not choose, this shows the candidates rather than the word
 * "no" — the user knows which meeting it was, and one click is cheaper for them
 * than Relay guessing and being wrong.
 */
export const MeetingCalendarLinkPanel: React.FC<MeetingCalendarLinkProps> = ({
  link,
  isConnected,
  onMatch,
  onChoose,
}) => {
  const [isBusy, setIsBusy] = useState(false);

  const run = async (action: () => Promise<void>) => {
    setIsBusy(true);
    try {
      await action();
    } finally {
      setIsBusy(false);
    }
  };

  if (!isConnected) {
    return (
      <p className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
        <CalendarDays className="w-3.5 h-3.5 shrink-0" aria-hidden="true" />
        Connect Google Calendar in Settings › Meetings to pull in the title,
        who was invited, and the agenda.
      </p>
    );
  }

  const matched = link?.outcome.kind === 'MATCHED' ? link.outcome : null;

  if (matched) {
    const invited = matched.event.attendees.filter((a) => a.response !== 'DECLINED');
    return (
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px]">
        <span className="inline-flex items-center gap-1.5 text-foreground">
          <CalendarDays className="w-3.5 h-3.5 text-primary" aria-hidden="true" />
          <span className="font-medium">{matched.event.title}</span>
          <span className="text-muted-foreground">{whenLabel(matched.event)}</span>
        </span>
        {invited.length > 0 && (
          <span className="text-muted-foreground">
            · {invited.length} invited
          </span>
        )}
        {link?.chosen_by_user && (
          <span
            className="text-emerald-600 dark:text-emerald-400 font-mono"
            title="You picked this event"
          >
            ✓
          </span>
        )}
        <button
          type="button"
          onClick={() => run(() => onChoose(null))}
          disabled={isBusy}
          className="text-muted-foreground hover:text-foreground underline transition-colors cursor-pointer disabled:opacity-50"
        >
          not this meeting
        </button>
      </div>
    );
  }

  const candidates = link?.outcome.kind === 'NONE' ? link.outcome.candidates : [];
  const reason = link?.outcome.kind === 'NONE' ? link.outcome.reason : null;

  return (
    <div className="space-y-1.5">
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={() => run(onMatch)}
          disabled={isBusy}
          className="inline-flex items-center gap-1.5 px-2 py-1 rounded-md border border-border bg-card hover:bg-accent text-[11px] text-foreground transition-colors cursor-pointer disabled:opacity-50"
        >
          {isBusy ? (
            <Loader2 className="w-3 h-3 animate-spin text-primary" />
          ) : (
            <Link2 className="w-3 h-3 text-primary" />
          )}
          {link ? 'Look again' : 'Find this in my calendar'}
        </button>
        {reason && (
          <span className="text-[11px] text-muted-foreground">
            {NO_MATCH_COPY[reason]}
          </span>
        )}
      </div>

      {candidates.length > 0 && (
        <ul className="space-y-1">
          {candidates.map((candidate) => (
            <CandidateRow
              key={candidate.event.id}
              candidate={candidate}
              isBusy={isBusy}
              onChoose={() => run(() => onChoose(candidate.event.id))}
            />
          ))}
        </ul>
      )}
    </div>
  );
};
