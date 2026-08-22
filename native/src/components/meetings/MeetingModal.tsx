import React, { useState } from 'react';
import { Meeting, MeetingSeries, MeetingProvider } from '../../types';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  Calendar,
  Clock,
  Video,
  Users,
  Repeat,
  X,
  CheckCircle2,
} from 'lucide-react';

interface MeetingModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSaveMeeting: (meetingData: {
    title: string;
    provider: MeetingProvider;
    series_id?: string | null;
    scheduled_start?: string;
    participants: string[];
  }) => Promise<void>;
  onSaveSeries: (seriesData: {
    title: string;
    provider?: string;
    recurrence_rule?: string;
  }) => Promise<void>;
  existingSeries: MeetingSeries[];
}

export const MeetingModal: React.FC<MeetingModalProps> = ({
  isOpen,
  onClose,
  onSaveMeeting,
  onSaveSeries,
  existingSeries,
}) => {
  const [mode, setMode] = useState<'standalone' | 'series'>('standalone');
  const [title, setTitle] = useState('');
  const [provider, setProvider] = useState<MeetingProvider>('google_meet');
  const [selectedSeriesId, setSelectedSeriesId] = useState<string>('');
  const [scheduledDate, setScheduledDate] = useState(() => {
    const now = new Date();
    return now.toISOString().slice(0, 16); // YYYY-MM-DDTHH:mm
  });
  const [participantsStr, setParticipantsStr] = useState('');
  const [recurrenceRule, setRecurrenceRule] = useState('Weekly on Monday');
  const [saving, setSaving] = useState(false);

  if (!isOpen) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!title.trim()) return;

    setSaving(true);
    try {
      if (mode === 'standalone') {
        const parts = participantsStr
          .split(',')
          .map((p) => p.trim())
          .filter(Boolean);
        await onSaveMeeting({
          title: title.trim(),
          provider,
          series_id: selectedSeriesId || null,
          scheduled_start: new Date(scheduledDate).toISOString(),
          participants: parts,
        });
      } else {
        await onSaveSeries({
          title: title.trim(),
          provider,
          recurrence_rule: recurrenceRule.trim(),
        });
      }
      onClose();
    } catch (err) {
      console.error('Failed to create meeting/series:', err);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4 animate-in fade-in duration-200">
      <div className="bg-card border border-border rounded-xl shadow-xl w-full max-w-lg overflow-hidden flex flex-col max-h-[90vh]">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-border/80 bg-muted/20">
          <div className="flex items-center gap-2">
            <Calendar className="w-5 h-5 text-primary" />
            <h2 className="text-base font-bold text-foreground">
              {mode === 'standalone' ? 'Schedule / Add Meeting' : 'Create Recurring Meeting Series'}
            </h2>
          </div>
          <Button
            variant="ghost"
            size="icon"
            onClick={onClose}
            className="h-8 w-8 text-muted-foreground hover:text-foreground"
          >
            <X className="w-4 h-4" />
          </Button>
        </div>

        {/* Mode Selector Tabs */}
        <div className="flex border-b border-border/80 bg-muted/10 p-1.5 gap-1.5">
          <button
            type="button"
            onClick={() => setMode('standalone')}
            className={`flex-1 py-1.5 text-xs font-medium rounded-md transition-all flex items-center justify-center gap-2 ${
              mode === 'standalone'
                ? 'bg-card text-foreground font-semibold shadow-xs border border-border/60'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            <Calendar className="w-3.5 h-3.5" />
            <span>Standalone Meeting</span>
          </button>
          <button
            type="button"
            onClick={() => setMode('series')}
            className={`flex-1 py-1.5 text-xs font-medium rounded-md transition-all flex items-center justify-center gap-2 ${
              mode === 'series'
                ? 'bg-card text-foreground font-semibold shadow-xs border border-border/60'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            <Repeat className="w-3.5 h-3.5" />
            <span>Recurring Meeting Series</span>
          </button>
        </div>

        {/* Form Body */}
        <form onSubmit={handleSubmit} className="p-6 space-y-4 overflow-y-auto flex-1">
          {/* Title */}
          <div className="space-y-1.5">
            <label className="text-xs font-semibold text-foreground">
              {mode === 'standalone' ? 'Meeting Title' : 'Series Title'} *
            </label>
            <Input
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder={
                mode === 'standalone'
                  ? 'e.g. Candidate Interview — John Doe'
                  : 'e.g. Weekly Product Strategy Sync'
              }
              required
              className="text-xs"
            />
          </div>

          {/* Provider */}
          <div className="space-y-1.5">
            <label className="text-xs font-semibold text-foreground flex items-center gap-1.5">
              <Video className="w-3.5 h-3.5 text-muted-foreground" />
              <span>Conferencing Provider</span>
            </label>
            <select
              value={provider}
              onChange={(e) => setProvider(e.target.value as MeetingProvider)}
              className="w-full h-9 rounded-md border border-input bg-background px-3 py-1 text-xs shadow-xs focus:outline-hidden focus:ring-1 focus:ring-ring text-foreground"
            >
              <option value="google_meet">Google Meet</option>
              <option value="zoom">Zoom</option>
              <option value="teams">Microsoft Teams</option>
              <option value="webex">Cisco Webex</option>
              <option value="in_person">In-Person</option>
              <option value="other">Other / Custom</option>
            </select>
          </div>

          {mode === 'standalone' ? (
            <>
              {/* Optional Series Assignment */}
              {existingSeries.length > 0 && (
                <div className="space-y-1.5">
                  <label className="text-xs font-semibold text-foreground flex items-center gap-1.5">
                    <Repeat className="w-3.5 h-3.5 text-muted-foreground" />
                    <span>Part of Series (Optional)</span>
                  </label>
                  <select
                    value={selectedSeriesId}
                    onChange={(e) => setSelectedSeriesId(e.target.value)}
                    className="w-full h-9 rounded-md border border-input bg-background px-3 py-1 text-xs shadow-xs focus:outline-hidden focus:ring-1 focus:ring-ring text-foreground"
                  >
                    <option value="">-- None (Standalone) --</option>
                    {existingSeries.map((s) => (
                      <option key={s.id} value={s.id}>
                        {s.title} {s.recurrence_rule ? `(${s.recurrence_rule})` : ''}
                      </option>
                    ))}
                  </select>
                </div>
              )}

              {/* Scheduled Date & Time */}
              <div className="space-y-1.5">
                <label className="text-xs font-semibold text-foreground flex items-center gap-1.5">
                  <Clock className="w-3.5 h-3.5 text-muted-foreground" />
                  <span>Scheduled Date & Time</span>
                </label>
                <Input
                  type="datetime-local"
                  value={scheduledDate}
                  onChange={(e) => setScheduledDate(e.target.value)}
                  className="text-xs"
                />
              </div>

              {/* Participants */}
              <div className="space-y-1.5">
                <label className="text-xs font-semibold text-foreground flex items-center gap-1.5">
                  <Users className="w-3.5 h-3.5 text-muted-foreground" />
                  <span>Participants (comma-separated)</span>
                </label>
                <Input
                  value={participantsStr}
                  onChange={(e) => setParticipantsStr(e.target.value)}
                  placeholder="e.g. Sarah Jenkins, Alex Rivera, Nitin"
                  className="text-xs"
                />
              </div>
            </>
          ) : (
            <>
              {/* Recurrence Rule */}
              <div className="space-y-1.5">
                <label className="text-xs font-semibold text-foreground flex items-center gap-1.5">
                  <Repeat className="w-3.5 h-3.5 text-muted-foreground" />
                  <span>Recurrence Cadence</span>
                </label>
                <Input
                  value={recurrenceRule}
                  onChange={(e) => setRecurrenceRule(e.target.value)}
                  placeholder="e.g. Weekly on Monday & Thursday, or Bi-weekly"
                  className="text-xs"
                />
                <p className="text-[11px] text-muted-foreground">
                  Individual occurrences under this series will be grouped automatically, with the latest meeting shown first.
                </p>
              </div>
            </>
          )}

          {/* Footer Actions */}
          <div className="flex items-center justify-end gap-2 pt-4 border-t border-border/80">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={onClose}
              className="text-xs"
            >
              Cancel
            </Button>
            <Button
              type="submit"
              size="sm"
              disabled={saving || !title.trim()}
              className="text-xs gap-1.5"
            >
              <CheckCircle2 className="w-3.5 h-3.5" />
              <span>{saving ? 'Creating…' : mode === 'standalone' ? 'Add Meeting' : 'Create Series'}</span>
            </Button>
          </div>
        </form>
      </div>
    </div>
  );
};
