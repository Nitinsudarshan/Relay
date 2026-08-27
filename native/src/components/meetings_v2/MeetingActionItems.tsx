import React from 'react';
import { Calendar, Check, ListTodo, User } from 'lucide-react';
import type { ActionItem, Speaker } from '../../types';
import { ownerLabel } from './meetingProcessing';

interface MeetingActionItemsProps {
  items: ActionItem[];
  speakers: Speaker[];
  /** Persists the checked state; action items are durable objects, not view state. */
  onToggle: (item: ActionItem) => void;
  busyItemId?: string | null;
}

/**
 * Structured action items: what, who, and when — each rendered from its own
 * field rather than parsed out of a formatted string.
 *
 * Ownership is shown exactly as extracted. "Unassigned" is a real answer and is
 * displayed as such; a missing deadline simply has no date, never a guessed one.
 */
export const MeetingActionItems: React.FC<MeetingActionItemsProps> = ({
  items,
  speakers,
  onToggle,
  busyItemId,
}) => {
  if (items.length === 0) return null;

  const openCount = items.filter((i) => i.status === 'OPEN').length;

  return (
    <div className="p-5 rounded-xl bg-gradient-to-b from-amber-950/25 to-zinc-950/50 border border-amber-500/30 flex flex-col gap-3 shadow-sm">
      <div className="flex items-center justify-between">
        <span className="text-xs font-bold text-amber-300 flex items-center gap-1.5 tracking-wide">
          <ListTodo className="w-3.5 h-3.5 text-amber-400" />
          Action Items
        </span>
        <span className="text-[10px] font-mono font-medium px-2 py-0.5 rounded-full bg-amber-500/15 text-amber-300 border border-amber-500/30">
          {openCount} open · {items.length} total
        </span>
      </div>

      <div className="space-y-1.5 pt-1">
        {items.map((item) => {
          const isDone = item.status === 'DONE';
          const owner = ownerLabel(item, speakers);
          const isUnassigned =
            item.owner_type === 'UNASSIGNED' || owner === 'Unassigned';

          return (
            <div
              key={item.id}
              onClick={() => busyItemId !== item.id && onToggle(item)}
              className={`flex items-start gap-2.5 p-2.5 rounded-lg cursor-pointer transition-all border ${
                isDone
                  ? 'bg-white/5 border-transparent text-zinc-500'
                  : 'bg-zinc-900/60 hover:bg-zinc-900 border-white/5 hover:border-amber-500/30 text-zinc-200'
              } ${busyItemId === item.id ? 'opacity-60' : ''}`}
            >
              <div className="mt-0.5 shrink-0">
                {isDone ? (
                  <Check className="w-3.5 h-3.5 text-emerald-400" />
                ) : (
                  <div className="w-3.5 h-3.5 rounded border border-zinc-600 hover:border-amber-400 transition-colors" />
                )}
              </div>

              <div className="flex-1 min-w-0 flex flex-col gap-1">
                <p
                  className={`text-xs leading-relaxed select-text ${
                    isDone ? 'line-through' : ''
                  }`}
                >
                  {item.description}
                </p>

                <div className="flex items-center gap-3 flex-wrap text-[10px] font-mono">
                  <span
                    className={`flex items-center gap-1 ${
                      isUnassigned ? 'text-zinc-500' : 'text-amber-300/90'
                    }`}
                    title={
                      isUnassigned
                        ? 'Ownership was not established in the meeting'
                        : `Owner: ${owner}`
                    }
                  >
                    <User className="w-3 h-3" />
                    {owner}
                  </span>

                  {item.deadline && (
                    <span
                      className="flex items-center gap-1 text-sky-300/90"
                      title="A date was spoken in the meeting"
                    >
                      <Calendar className="w-3 h-3" />
                      {item.deadline}
                    </span>
                  )}

                  {item.source_segment_ids.length > 0 && (
                    <span
                      className="text-zinc-600"
                      title={`Extracted from ${item.source_segment_ids.join(', ')} in the raw transcript`}
                    >
                      {item.source_segment_ids[0]}
                    </span>
                  )}
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};
