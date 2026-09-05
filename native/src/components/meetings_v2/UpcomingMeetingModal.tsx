import React, { useEffect, useState } from 'react';
import {
  Calendar,
  Check,
  Clock,
  Copy,
  ExternalLink,
  FileText,
  MapPin,
  Play,
  User,
  Users,
  Video,
  X,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { CalendarEvent } from '../../types';

export function getMeetingJoinUrl(evt: CalendarEvent): string | null {
  if (
    evt.conference_url &&
    (evt.conference_url.startsWith('https://') || evt.conference_url.startsWith('http://'))
  ) {
    return evt.conference_url.trim();
  }
  if (
    evt.location &&
    (evt.location.startsWith('https://') || evt.location.startsWith('http://'))
  ) {
    return evt.location.trim();
  }
  if (evt.description) {
    const urlMatch = evt.description.match(/https?:\/\/[^\s<>"')]+/);
    if (urlMatch) {
      return urlMatch[0].trim();
    }
  }
  return null;
}

export function formatRelativeTimeToMeeting(startsAt: string, endsAt?: string | null): string {
  const start = new Date(startsAt);
  if (isNaN(start.getTime())) return '';

  const now = new Date();
  const diffMs = start.getTime() - now.getTime();
  const diffSec = Math.floor(diffMs / 1000);
  const diffMin = Math.floor(diffSec / 60);

  if (diffMs < 0) {
    if (endsAt) {
      const end = new Date(endsAt);
      if (!isNaN(end.getTime()) && now.getTime() < end.getTime()) {
        return 'In progress';
      }
    }
    const pastMin = Math.abs(diffMin);
    if (pastMin < 60) return `${pastMin}m ago`;
    const pastHr = Math.floor(pastMin / 60);
    return `${pastHr}h ago`;
  }

  if (diffMin < 1) return 'now';
  if (diffMin < 60) return `in ${diffMin}min`;

  const hours = Math.floor(diffMin / 60);
  const remainingMins = diffMin % 60;
  if (hours < 24) {
    if (remainingMins === 0) return `in ${hours}hr`;
    return `in ${hours}hr ${remainingMins}min`;
  }

  const days = Math.floor(hours / 24);
  const remainingHours = hours % 24;
  if (remainingHours === 0) return `in ${days}d`;
  return `in ${days}d ${remainingHours}hr`;
}

/** Formats and sanitizes calendar descriptions (HTML or plain text) for clean readability and responsive wrapping */
function renderCalendarDescription(description?: string | null) {
  if (!description || !description.trim()) {
    return (
      <p className="text-muted-foreground text-xs italic">
        No description or agenda provided for this meeting.
      </p>
    );
  }

  const hasHtml = /<[a-z][\s\S]*>/i.test(description);

  if (hasHtml) {
    try {
      const parser = new DOMParser();
      const doc = parser.parseFromString(description, 'text/html');

      // Strip dangerous or disruptive elements
      const dangerous = doc.querySelectorAll('script, style, iframe, object, embed, form, input, button');
      dangerous.forEach((el) => el.remove());

      // Ensure all anchor links have safe attributes, target="_blank", and wrap long URLs properly
      const links = doc.querySelectorAll('a');
      links.forEach((a) => {
        a.setAttribute('target', '_blank');
        a.setAttribute('rel', 'noopener noreferrer');
        a.classList.add(
          'text-primary',
          'underline',
          'hover:text-primary/80',
          'font-semibold',
          'break-all',
          '[overflow-wrap:anywhere]'
        );
      });

      // Prevent <pre> or <code> blocks from causing horizontal overflow
      const codeBlocks = doc.querySelectorAll('pre, code');
      codeBlocks.forEach((el) => {
        el.classList.add('whitespace-pre-wrap', 'break-all', '[overflow-wrap:anywhere]');
      });

      // Ensure tables fit nicely
      const tables = doc.querySelectorAll('table');
      tables.forEach((el) => {
        el.classList.add('max-w-full', 'table-auto', 'break-words');
      });

      return (
        <div
          className="prose dark:prose-invert max-w-none text-xs sm:text-sm text-foreground/90 leading-relaxed font-sans space-y-3 break-words [overflow-wrap:anywhere] [word-break:break-word]"
          dangerouslySetInnerHTML={{ __html: doc.body.innerHTML }}
        />
      );
    } catch {
      // fallback to plain text renderer below
    }
  }

  // Plain text formatting with clickable URL detection and wrap-anywhere for long tokens
  const urlRegex = /(https?:\/\/[^\s<>"')]+)/g;
  const parts = description.split(urlRegex);

  return (
    <div className="text-xs sm:text-sm text-foreground/90 leading-relaxed font-sans whitespace-pre-wrap break-words [overflow-wrap:anywhere] [word-break:break-word] space-y-1">
      {parts.map((part, i) =>
        urlRegex.test(part) ? (
          <a
            key={i}
            href={part}
            target="_blank"
            rel="noopener noreferrer"
            className="text-primary underline hover:text-primary/80 font-semibold break-all [overflow-wrap:anywhere]"
          >
            {part}
          </a>
        ) : (
          <React.Fragment key={i}>{part}</React.Fragment>
        )
      )}
    </div>
  );
}

interface UpcomingMeetingModalProps {
  event: CalendarEvent | null;
  isOpen: boolean;
  onClose: () => void;
  onStartRecording: (evt: CalendarEvent) => void;
  onJoinAndRecord: (evt: CalendarEvent) => void;
  isStarting?: boolean;
}

export const UpcomingMeetingModal: React.FC<UpcomingMeetingModalProps> = ({
  event,
  isOpen,
  onClose,
  onStartRecording,
  onJoinAndRecord,
  isStarting = false,
}) => {
  const [copied, setCopied] = useState<boolean>(false);

  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !isStarting) {
        onClose();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, isStarting, onClose]);

  if (!isOpen || !event) return null;

  const joinUrl = getMeetingJoinUrl(event);
  const relativeTime = formatRelativeTimeToMeeting(event.starts_at, event.ends_at);

  const startDate = new Date(event.starts_at);
  const endDate = new Date(event.ends_at);
  const dateStr = isNaN(startDate.getTime())
    ? ''
    : startDate.toLocaleDateString([], { weekday: 'short', month: 'short', day: 'numeric' });
  const startTimeStr = isNaN(startDate.getTime())
    ? ''
    : startDate.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  const endTimeStr = isNaN(endDate.getTime())
    ? ''
    : endDate.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });

  const handleCopyLink = () => {
    if (!joinUrl) return;
    navigator.clipboard.writeText(joinUrl);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 overflow-y-auto">
      {/* Viewport Backdrop */}
      <div
        className="fixed inset-0 bg-black/60 backdrop-blur-sm transition-opacity animate-in fade-in duration-150"
        onClick={() => !isStarting && onClose()}
      />

      {/* Centered Modal Card - 90% width and height aligned with Home page */}
      <div className="relative bg-card text-card-foreground border border-border rounded-lg shadow-2xl w-[90vw] max-w-[90vw] h-[90vh] max-h-[90vh] flex flex-col overflow-hidden z-10 animate-in fade-in zoom-in-95 duration-200">
        {/* Header */}
        <div className="p-4 px-6 border-b border-border flex items-start justify-between gap-4 shrink-0 bg-card">
          <div className="space-y-1 min-w-0 flex-1">
            <span className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground block">
              Meeting Details
            </span>
            <div className="flex items-center gap-2.5 flex-wrap">
              <h2 className="text-lg sm:text-xl font-extrabold text-foreground leading-snug break-words">
                {event.title}
              </h2>
              {relativeTime && (
                <span className="font-mono text-[9px] bg-primary/10 border border-primary/20 text-primary px-2 py-0.5 rounded-md font-semibold flex items-center gap-1">
                  <Clock className="w-3 h-3" />
                  <span>{relativeTime}</span>
                </span>
              )}
            </div>
          </div>

          <button
            type="button"
            onClick={() => !isStarting && onClose()}
            className="text-muted-foreground hover:text-foreground p-1.5 rounded-lg hover:bg-muted shrink-0 transition-colors"
            disabled={isStarting}
            aria-label="Close"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Content Body - Vertically fills the modal without empty dead space */}
        <div className="p-6 flex-1 flex flex-col gap-4 min-h-0 overflow-hidden">
          {/* Metadata Cards Grid - All cards have matching 2-line layout and equal height */}
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3.5 shrink-0">
            {/* Time & Duration - Exactly 2 lines */}
            <div className="p-3.5 rounded-lg bg-card border border-border flex items-center gap-3">
              <div className="p-2 rounded-md bg-muted/50 border border-border text-muted-foreground shrink-0">
                <Clock className="w-4 h-4" />
              </div>
              <div className="min-w-0 flex-1">
                <div className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground">
                  Date &amp; Time
                </div>
                <div className="text-xs font-bold text-foreground mt-0.5 truncate">
                  {dateStr ? `${dateStr} · ` : ''}{startTimeStr} {endTimeStr ? `– ${endTimeStr}` : ''}
                </div>
              </div>
            </div>

            {/* Location / Meeting link - Exactly 2 lines with clean Join Video Call and Copy Link button */}
            {(event.location || joinUrl) && (
              <div className="p-3.5 rounded-lg bg-card border border-border flex items-center gap-3">
                <div className="p-2 rounded-md bg-muted/50 border border-border text-primary shrink-0">
                  {joinUrl ? <Video className="w-4 h-4 text-blue-500" /> : <MapPin className="w-4 h-4" />}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground">
                    {joinUrl ? 'Conferencing Link' : 'Location'}
                  </div>
                  {joinUrl ? (
                    <div className="flex items-center gap-2 mt-0.5">
                      <a
                        href={joinUrl}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="text-xs font-bold text-primary hover:underline flex items-center gap-1 truncate"
                      >
                        <span>Join Video Call</span>
                        <ExternalLink className="w-3 h-3 shrink-0" />
                      </a>
                      <button
                        type="button"
                        onClick={handleCopyLink}
                        className="text-[10px] font-mono text-muted-foreground hover:text-foreground flex items-center gap-1 px-1.5 py-0.5 rounded-md border border-border hover:bg-muted/50 transition-colors shrink-0"
                        title="Copy meeting link to clipboard"
                      >
                        {copied ? <Check className="w-3 h-3 text-emerald-500" /> : <Copy className="w-3 h-3" />}
                        <span>{copied ? 'Copied' : 'Copy'}</span>
                      </button>
                    </div>
                  ) : (
                    <div className="text-xs font-bold text-foreground truncate mt-0.5">
                      {event.location}
                    </div>
                  )}
                </div>
              </div>
            )}

            {/* Organizer - Exactly 2 lines */}
            {event.organizer && (
              <div className="p-3.5 rounded-lg bg-card border border-border flex items-center gap-3">
                <div className="p-2 rounded-md bg-muted/50 border border-border text-muted-foreground shrink-0">
                  <User className="w-4 h-4" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground">
                    Organizer
                  </div>
                  <div className="text-xs font-bold text-foreground mt-0.5 truncate">
                    {event.organizer}
                  </div>
                </div>
              </div>
            )}
          </div>

          {/* Main Dual-column Body: Narrower Participants + Fully Expanded Description */}
          <div className="flex-1 min-h-0 flex flex-col lg:flex-row gap-5 overflow-hidden">
            {/* Participants Panel - Narrower width (w-72), full vertical height */}
            <div className="w-full lg:w-72 shrink-0 flex flex-col min-h-0 space-y-2">
              <div className="flex items-center justify-between text-xs font-semibold text-foreground shrink-0">
                <span className="flex items-center gap-2">
                  <Users className="w-4 h-4 text-primary" />
                  <span className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground">
                    Participants ({event.attendees?.length ?? 0})
                  </span>
                </span>
              </div>

              <div className="flex-1 min-h-0 overflow-y-auto rounded-lg border border-border bg-card divide-y divide-border">
                {event.attendees && event.attendees.length > 0 ? (
                  event.attendees.map((attendee, idx) => (
                    <div key={idx} className="p-2.5 px-3 flex items-center justify-between gap-2.5 text-xs">
                      <div className="flex items-center gap-2.5 min-w-0 flex-1">
                        <div className="w-6 h-6 rounded-full bg-primary/10 text-primary flex items-center justify-center font-bold text-[10px] shrink-0 border border-primary/20">
                          {attendee.name.charAt(0).toUpperCase()}
                        </div>
                        <div className="min-w-0 flex-1">
                          <span className="font-semibold text-foreground truncate block">
                            {attendee.name}
                          </span>
                          {attendee.is_organizer && (
                            <span className="text-[9px] font-mono text-muted-foreground block">
                              organizer
                            </span>
                          )}
                        </div>
                      </div>
                      {attendee.response && (
                        <Badge variant="outline" className="text-[9px] font-mono shrink-0 px-1.5 py-0 border-border text-muted-foreground">
                          {attendee.response.toLowerCase()}
                        </Badge>
                      )}
                    </div>
                  ))
                ) : (
                  <div className="p-4 text-center text-muted-foreground text-xs italic">
                    No other participants listed.
                  </div>
                )}
              </div>
            </div>

            {/* Description & Agenda - Full height & width, with clean sanitized formatting */}
            <div className="flex-1 min-w-0 flex flex-col min-h-0 space-y-2">
              <div className="flex items-center justify-between text-xs font-semibold text-foreground shrink-0">
                <span className="flex items-center gap-2">
                  <FileText className="w-4 h-4 text-primary" />
                  <span className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground">
                    Description &amp; Agenda
                  </span>
                </span>
              </div>

              <div className="flex-1 min-h-0 overflow-y-auto overflow-x-hidden p-5 rounded-lg border border-border bg-card select-text break-words [overflow-wrap:anywhere]">
                {renderCalendarDescription(event.description)}
              </div>
            </div>
          </div>
        </div>

        {/* Footer Actions */}
        <div className="p-4 px-6 border-t border-border bg-card/60 flex items-center justify-end gap-3 shrink-0">
          <Button
            type="button"
            variant="ghost"
            size="default"
            onClick={onClose}
            disabled={isStarting}
            className="text-xs h-9 px-4 text-muted-foreground hover:text-foreground"
          >
            Close
          </Button>

          <Button
            type="button"
            variant="outline"
            size="default"
            onClick={() => onStartRecording(event)}
            disabled={isStarting}
            className="text-xs h-9 px-4 gap-2 border-border hover:bg-muted font-medium"
          >
            <Play className="w-3.5 h-3.5 fill-current" />
            <span>Start Recording</span>
          </Button>

          <Button
            type="button"
            variant="default"
            size="default"
            onClick={() => onJoinAndRecord(event)}
            disabled={isStarting || !joinUrl}
            title={!joinUrl ? 'No video conference link found in this event' : undefined}
            className="text-xs h-9 px-4 gap-2 bg-primary text-primary-foreground hover:bg-primary/90 font-semibold disabled:opacity-50"
          >
            <Video className="w-4 h-4" />
            <span>Join and Record</span>
          </Button>
        </div>
      </div>
    </div>
  );
};
