import React from 'react';
import { Check, Loader2, Plus, SquareCheckBig } from 'lucide-react';
import type { ActionItem, Speaker } from '../../types';
import { ownerLabel } from './meetingProcessing';

interface MeetingActionItemsProps {
  items: ActionItem[];
  speakers: Speaker[];
  /** Persists the checked state; action items are durable objects, not view state. */
  onToggle: (item: ActionItem) => void;
  /** Adds one to-do to the Kanban board. */
  onAddTask: (item: ActionItem) => void;
  /** Adds every to-do that is not already on the board. */
  onAddAllTasks: () => void;
  busyItemId?: string | null;
  isAddingAll?: boolean;
}

/**
 * Structured to-dos: what, who, when — each rendered from its own field rather
 * than parsed out of a formatted string.
 *
 * Ownership is shown exactly as extracted. "Unassigned" is a real answer and is
 * displayed as such; a missing deadline simply has no date, never a guessed one.
 *
 * Deliberately flat. A meeting's to-dos are a list to read and act on, so the
 * only colour here marks the one thing that is actionable — whether a to-do has
 * reached the board yet.
 */
export const MeetingActionItems: React.FC<MeetingActionItemsProps> = ({
  items,
  speakers,
  onToggle,
  onAddTask,
  onAddAllTasks,
  busyItemId,
  isAddingAll,
}) => {
  if (items.length === 0) return null;

  const openCount = items.filter((i) => i.status === 'OPEN').length;
  const pendingTaskCount = items.filter((i) => !i.kanban_card_id).length;

  return (
    <section className="flex flex-col gap-3">
      <header className="flex items-center justify-between gap-3">
        <h3 className="text-xs font-medium uppercase tracking-wider text-zinc-400">
          To-dos
          <span className="ml-2 font-mono normal-case tracking-normal text-zinc-600">
            {openCount} open · {items.length} total
          </span>
        </h3>

        {pendingTaskCount > 0 && (
          <button
            onClick={onAddAllTasks}
            disabled={isAddingAll}
            title="Add every to-do that is not already on the board"
            className="flex items-center gap-1.5 px-2.5 py-1 rounded-md text-[11px] font-medium text-zinc-300 border border-white/10 hover:bg-white/5 disabled:opacity-40 transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-lime-400"
          >
            {isAddingAll ? (
              <Loader2 className="w-3 h-3 animate-spin" />
            ) : (
              <Plus className="w-3 h-3" />
            )}
            Add {pendingTaskCount} to tasks
          </button>
        )}
      </header>

      <ul className="flex flex-col divide-y divide-white/5 border-y border-white/5">
        {items.map((item) => {
          const isDone = item.status === 'DONE';
          const owner = ownerLabel(item, speakers);
          const isUnassigned =
            item.owner_type === 'UNASSIGNED' || owner === 'Unassigned';
          const isBusy = busyItemId === item.id;
          const onBoard = Boolean(item.kanban_card_id);

          return (
            <li
              key={item.id}
              className={`flex items-start gap-3 py-2.5 ${isBusy ? 'opacity-50' : ''}`}
            >
              <button
                onClick={() => !isBusy && onToggle(item)}
                aria-label={isDone ? 'Mark as not done' : 'Mark as done'}
                className="mt-[3px] shrink-0 grid place-items-center w-4 h-4 rounded border border-zinc-600 hover:border-zinc-400 transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-lime-400"
              >
                {isDone && <Check className="w-3 h-3 text-lime-400" />}
              </button>

              <div className="flex-1 min-w-0 flex flex-col gap-1">
                <p
                  className={`text-[13px] leading-snug select-text ${
                    isDone ? 'line-through text-zinc-500' : 'text-zinc-200'
                  }`}
                >
                  {item.description}
                </p>

                <div className="flex items-center gap-3 flex-wrap text-[10px] font-mono text-zinc-500">
                  <span
                    title={
                      isUnassigned
                        ? 'Ownership was not established in the meeting'
                        : `Owner: ${owner}`
                    }
                    className={isUnassigned ? '' : 'text-zinc-400'}
                  >
                    {owner}
                  </span>

                  {item.deadline && (
                    <span title="A date was spoken in the meeting" className="text-zinc-400">
                      {item.deadline}
                    </span>
                  )}

                  {item.source_segment_ids.length > 0 && (
                    <span
                      title={`Read out of ${item.source_segment_ids.join(', ')} in the raw transcript`}
                    >
                      {item.source_segment_ids[0]}
                    </span>
                  )}
                </div>
              </div>

              {onBoard ? (
                <span
                  title="Already on the Kanban board"
                  className="shrink-0 mt-[2px] flex items-center gap-1 text-[10px] font-mono text-lime-500/80"
                >
                  <SquareCheckBig className="w-3 h-3" />
                  Task
                </span>
              ) : (
                <button
                  onClick={() => !isBusy && onAddTask(item)}
                  disabled={isBusy}
                  title="Add this to-do to the Kanban board"
                  className="shrink-0 mt-[1px] grid place-items-center w-6 h-6 rounded-md text-zinc-500 hover:text-zinc-200 hover:bg-white/5 disabled:opacity-40 transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-lime-400"
                >
                  <Plus className="w-3.5 h-3.5" />
                </button>
              )}
            </li>
          );
        })}
      </ul>
    </section>
  );
};
