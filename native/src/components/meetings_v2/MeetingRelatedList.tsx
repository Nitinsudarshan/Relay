import React from 'react';
import { Link2 } from 'lucide-react';
import type { RelatedMeeting } from '../../types';

interface MeetingRelatedListProps {
  related: RelatedMeeting[];
  onSelect: (meetingId: string) => void;
}

/**
 * Meetings related to this one, with the reason shown.
 *
 * The reason matters: two meetings both called "Daily Standup" are not related
 * by that fact alone, so each row names the topics and entities they actually
 * share rather than presenting an unexplained score.
 */
export const MeetingRelatedList: React.FC<MeetingRelatedListProps> = ({
  related,
  onSelect,
}) => {
  if (related.length === 0) return null;

  return (
    <div className="p-5 rounded-xl bg-zinc-950/50 border border-white/5 flex flex-col gap-3">
      <span className="text-xs font-bold text-zinc-300 flex items-center gap-1.5">
        <Link2 className="w-3.5 h-3.5 text-zinc-500" />
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
              className="flex flex-col gap-1 p-2.5 rounded-md bg-white/[0.03] hover:bg-white/[0.06] border border-white/5 hover:border-white/15 text-left transition-colors cursor-pointer"
            >
              <div className="flex items-center justify-between gap-2">
                <span className="text-xs font-medium text-zinc-200 truncate">
                  {meeting.title}
                </span>
                <span className="text-[10px] font-mono text-zinc-500 shrink-0">
                  {meeting.created_at.split('T')[0]}
                </span>
              </div>
              <span className="text-[10px] text-zinc-500 truncate">
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
