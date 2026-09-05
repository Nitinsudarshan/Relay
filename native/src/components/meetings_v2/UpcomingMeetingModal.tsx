import React, { useEffect } from 'react';
import {
  Calendar,
  Clock,
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
        <div className="p-5 px-6 border-b border-border flex items-start justify-between gap-4 shrink-0 bg-card">
          <div className="space-y-1.5 min-w-0 flex-1">
            <span className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground block">
              Meeting Details
            </span>
            <div className="flex items-center gap-2.5 flex-wrap">
              <h2 className="text-xl font-extrabold text-foreground leading-snug break-words">
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
            className="text-muted-foreground hover:text-foreground p-2 rounded-lg hover:bg-muted shrink-0 transition-colors"
            disabled={isStarting}
            aria-label="Close"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Scrollable Content */}
        <div className="p-6 overflow-y-auto space-y-6 text-sm flex-1">
          {/* Metadata Cards Grid */}
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3.5">
            {/* Time & Duration */}
            <div className="p-3.5 rounded-lg bg-muted/30 border border-border/60 flex items-start gap-3">
              <div className="p-2 rounded-md bg-background border border-border/40 text-muted-foreground shrink-0 mt-0.5">
                <Clock className="w-4 h-4" />
              </div>
              <div className="min-w-0">
                <div className="text-[11px] font-medium text-muted-foreground">Date & Time</div>
                <div className="text-xs font-semibold text-foreground mt-0.5">
                  {dateStr ? `${dateStr}` : 'Scheduled'}
                </div>
                <div className="text-xs text-muted-foreground mt-0.5">
                  {startTimeStr} {endTimeStr ? `– ${endTimeStr}` : ''}
                </div>
              </div>
            </div>

            {/* Location / Meeting link */}
            {(event.location || joinUrl) && (
              <div className="p-3.5 rounded-lg bg-muted/30 border border-border/60 flex items-start gap-3">
                <div className="p-2 rounded-md bg-background border border-border/40 text-primary shrink-0 mt-0.5">
                  {joinUrl ? <Video className="w-4 h-4 text-blue-500" /> : <MapPin className="w-4 h-4" />}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="text-[11px] font-medium text-muted-foreground">
                    {joinUrl ? 'Conferencing Link' : 'Location'}
                  </div>
                  {joinUrl ? (
                    <a
                      href={joinUrl}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-xs text-primary font-mono hover:underline truncate block mt-0.5"
                    >
                      {joinUrl}
                    </a>
                  ) : (
                    <div className="text-xs text-foreground truncate mt-0.5">{event.location}</div>
                  )}
                </div>
              </div>
            )}

            {/* Organizer */}
            {event.organizer && (
              <div className="p-3.5 rounded-lg bg-muted/30 border border-border/60 flex items-start gap-3">
                <div className="p-2 rounded-md bg-background border border-border/40 text-muted-foreground shrink-0 mt-0.5">
                  <User className="w-4 h-4" />
                </div>
                <div className="min-w-0">
                  <div className="text-[11px] font-medium text-muted-foreground">Organizer</div>
                  <div className="text-xs font-semibold text-foreground mt-0.5">
                    {event.organizer}
                  </div>
                </div>
              </div>
            )}
          </div>

          {/* Dual-column section for Participants and Description/Agenda */}
          <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
            {/* Participants */}
            <div className={event.description ? 'lg:col-span-5 space-y-2.5' : 'col-span-12 space-y-2.5'}>
              <div className="flex items-center justify-between text-xs font-semibold text-foreground">
                <span className="flex items-center gap-2">
                  <Users className="w-4 h-4 text-primary" />
                  Participants ({event.attendees?.length ?? 0})
                </span>
              </div>

              {event.attendees && event.attendees.length > 0 ? (
                <div className="max-h-64 overflow-y-auto rounded-lg border border-border/60 divide-y divide-border/40 bg-muted/20">
                  {event.attendees.map((attendee, idx) => (
                    <div key={idx} className="p-2.5 px-3 flex items-center justify-between gap-3 text-xs">
                      <div className="flex items-center gap-2.5 min-w-0">
                        <div className="w-6 h-6 rounded-full bg-primary/10 text-primary flex items-center justify-center font-bold text-[10px] shrink-0">
                          {attendee.name.charAt(0).toUpperCase()}
                        </div>
                        <span className="font-medium text-foreground truncate">
                          {attendee.name}
                        </span>
                        {attendee.is_organizer && (
                          <Badge variant="outline" className="text-[9px] px-1.5 py-0 font-mono text-muted-foreground">
                            organizer
                          </Badge>
                        )}
                      </div>
                      {attendee.response && (
                        <Badge variant="outline" className="text-[10px] font-mono shrink-0">
                          {attendee.response.toLowerCase()}
                        </Badge>
                      )}
                    </div>
                  ))}
                </div>
              ) : (
                <p className="text-muted-foreground text-xs italic">No other participants listed.</p>
              )}
            </div>

            {/* Description / Agenda */}
            {event.description && (
              <div className="lg:col-span-7 space-y-2.5">
                <div className="flex items-center gap-2 text-xs font-semibold text-foreground">
                  <FileText className="w-4 h-4 text-primary" />
                  <span>Description & Agenda</span>
                </div>
                <div className="p-4 rounded-lg bg-muted/20 border border-border/50 text-xs text-foreground/90 whitespace-pre-wrap max-h-64 overflow-y-auto leading-relaxed font-sans">
                  {event.description}
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Footer Actions */}
        <div className="p-5 px-6 border-t border-border/60 bg-muted/10 flex items-center justify-end gap-3 shrink-0">
          <Button
            type="button"
            variant="ghost"
            size="default"
            onClick={onClose}
            disabled={isStarting}
            className="text-xs h-9 px-4"
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
