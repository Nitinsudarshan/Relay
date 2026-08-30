import React from 'react';
import { Link2 } from 'lucide-react';
import type { RelatedMeeting } from '../../types';

interface MeetingRelatedListProps {
  related: RelatedMeeting[];
  onSelect: (meetingId: string) => void;
}

/**
 * Meetings related to this one, with the reason shown.
 */
export const MeetingRelatedList: React.FC<MeetingRelatedListProps> = ({
  related,
  onSelect,
}) => {
  if (related.length === 0) return null;

  return (
    <div className="p-5 rounded-lg bg-card border border-border flex flex-col gap-3">
      <span className="text-xs font-bold text-foreground flex items-center gap-1.5">
        <Link2 className="w-3.5 h-3.5 text-primary" />
        Related Meetings
      </span>

      <div className="flex flex-col gap-1.5">
        {related.map((meeting) => {
          const reasons = [
            ...meeting.signals.shared_topics,
            ...meeting.signals.shared_entities,
          ].slice(0, 3);

          return (
            <button
              key={meeting.meeting_id}
              onClick={() => onSelect(meeting.meeting_id)}
              className="flex flex-col gap-1 p-2.5 rounded-md bg-muted/40 hover:bg-accent border border-border text-left transition-colors cursor-pointer"
            >
              <div className="flex items-center justify-between gap-2">
                <span className="text-xs font-medium text-foreground truncate">
                  {meeting.title}
                </span>
                <span className="text-[10px] font-mono text-muted-foreground shrink-0">
                  {meeting.created_at.split('T')[0]}
                </span>
              </div>
              <span className="text-[10px] text-muted-foreground truncate">
                {reasons.length > 0
                  ? `Shares ${reasons.join(', ')}`
                  : 'Related by participants and meeting type'}
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
};

