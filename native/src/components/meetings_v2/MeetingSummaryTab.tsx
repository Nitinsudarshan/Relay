import React, { useState } from 'react';
import {
  Check,
  ChevronDown,
  Copy,
  Info,
  RefreshCw,
  Sparkles,
} from 'lucide-react';
import { MarkdownView } from '../common/MarkdownView';
import type {
  ActionItem,
  MeetingExtension,
  MeetingProcessing,
  RelatedMeeting,
  SummaryMode,
} from '../../types';
import { MeetingActionItems } from './MeetingActionItems';
import { MeetingRelatedList } from './MeetingRelatedList';
import { SUMMARY_MODES, canSummarize } from './meetingProcessing';

interface MeetingSummaryTabProps {
  processing?: MeetingProcessing | null;
  extensions: MeetingExtension[];
  related: RelatedMeeting[];
  isGenerating: boolean;
  /** False while the meeting is still recording, when there is nothing to summarize yet. */
  canGenerate: boolean;
  onGenerate: (mode?: SummaryMode, extensionId?: string) => void;
  onToggleActionItem: (item: ActionItem) => void;
  /** Adds one to-do to the Kanban board. */
  onAddTask: (item: ActionItem) => void;
  /** Adds every to-do not already on the board. */
  onAddAllTasks: () => void;
  busyActionItemId?: string | null;
  isAddingAllTasks?: boolean;
  onSelectRelated: (meetingId: string) => void;
}

/**
 * The default meeting view: what mattered, what was decided, what needs doing.
 *
 * Everything here is a projection of the same extracted facts — the prose, the
 * action items, the topics, and the related meetings all come from one
 * `MeetingProcessing`, so they cannot disagree with each other.
 */
export const MeetingSummaryTab: React.FC<MeetingSummaryTabProps> = ({
  processing,
  extensions,
  related,
  isGenerating,
  canGenerate,
  onGenerate,
  onToggleActionItem,
  onAddTask,
  onAddAllTasks,
  busyActionItemId,
  isAddingAllTasks,
  onSelectRelated,
}) => {
  const [copied, setCopied] = useState(false);
  const [modeMenuOpen, setModeMenuOpen] = useState(false);
  const [extensionMenuOpen, setExtensionMenuOpen] = useState(false);

  const summary = processing?.summary;
  const facts = processing?.facts;
  const activeMode: SummaryMode = summary?.mode ?? 'STANDARD';
  const activeExtension =
    extensions.find((e) => e.id === (summary?.extension_id ?? 'default')) ??
    extensions[0];

  const handleCopy = async () => {
    if (!summary) return;
    try {
      await navigator.clipboard.writeText(summary.markdown);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy summary:', err);
    }
  };

  if (!summary) {
    const hasTranscript = canSummarize(processing);
    return (
      <div className="flex-1 overflow-y-auto py-16 px-4 text-center flex flex-col items-center justify-center gap-3">
        <div className="w-12 h-12 rounded-lg bg-white/5 border border-white/10 flex items-center justify-center text-zinc-400 mb-1">
          <Sparkles className="w-6 h-6" />
        </div>
        <h4 className="text-sm font-semibold text-zinc-200">No summary yet</h4>
        <p className="text-xs text-zinc-400 max-w-sm">
          {hasTranscript
            ? 'Relay will read the transcript, work out what was decided and who owns what, and write it up.'
            : 'This meeting has no transcribed speech yet. The recording and raw transcript are unaffected.'}
        </p>
        {canGenerate && (
          <button
            onClick={() => onGenerate()}
            disabled={isGenerating || !hasTranscript}
            className="mt-2 flex items-center gap-1.5 px-4 py-2 rounded-md bg-zinc-100 hover:bg-white text-zinc-900 text-xs font-medium transition-colors disabled:opacity-40 disabled:cursor-not-allowed focus-visible:outline focus-visible:outline-2 focus-visible:outline-lime-400"
          >
            {isGenerating ? (
              <>
                <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                <span>Generating…</span>
              </>
            ) : (
              <>
                <Sparkles className="w-3.5 h-3.5" />
                <span>Generate Summary</span>
              </>
            )}
          </button>
        )}
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto p-6 space-y-5">
      {/* Mode and extension pickers. Two controls, not a configuration screen:
          both change presentation only and reuse the facts already extracted. */}
      <div className="flex items-center gap-2 flex-wrap">
        <div className="relative">
          <button
            onClick={() => {
              setModeMenuOpen((v) => !v);
              setExtensionMenuOpen(false);
            }}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-white/5 hover:bg-white/10 border border-white/10 text-xs font-medium text-zinc-200 transition-colors cursor-pointer"
          >
            <span className="text-zinc-500">Summary</span>
            <span>{SUMMARY_MODES.find((m) => m.value === activeMode)?.label}</span>
            <ChevronDown className="w-3 h-3 text-zinc-500" />
          </button>
          {modeMenuOpen && (
            <div className="absolute z-20 mt-1 w-48 rounded-md bg-zinc-900 border border-white/10 shadow-lg overflow-hidden">
              {SUMMARY_MODES.map((mode) => (
                <button
                  key={mode.value}
                  onClick={() => {
                    setModeMenuOpen(false);
                    onGenerate(mode.value, activeExtension?.id);
                  }}
                  className={`w-full flex items-center justify-between px-3 py-2 text-xs text-left hover:bg-white/5 transition-colors cursor-pointer ${
                    mode.value === activeMode ? 'text-lime-400' : 'text-zinc-300'
                  }`}
                >
                  <span>{mode.label}</span>
                  <span className="text-[10px] text-zinc-500">{mode.hint}</span>
                </button>
              ))}
            </div>
          )}
        </div>

        {extensions.length > 1 && (
          <div className="relative">
            <button
              onClick={() => {
                setExtensionMenuOpen((v) => !v);
                setModeMenuOpen(false);
              }}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-white/5 hover:bg-white/10 border border-white/10 text-xs font-medium text-zinc-200 transition-colors cursor-pointer"
            >
              <span className="text-zinc-500">Extension</span>
              <span>{activeExtension?.name ?? 'Default'}</span>
              <ChevronDown className="w-3 h-3 text-zinc-500" />
            </button>
            {extensionMenuOpen && (
              <div className="absolute z-20 mt-1 w-52 rounded-md bg-zinc-900 border border-white/10 shadow-lg overflow-hidden">
                {extensions.map((extension) => (
                  <button
                    key={extension.id}
                    onClick={() => {
                      setExtensionMenuOpen(false);
                      onGenerate(activeMode, extension.id);
                    }}
                    className={`w-full px-3 py-2 text-xs text-left hover:bg-white/5 transition-colors cursor-pointer ${
                      extension.id === activeExtension?.id
                        ? 'text-lime-400'
                        : 'text-zinc-300'
                    }`}
                  >
                    {extension.name}
                  </button>
                ))}
              </div>
            )}
          </div>
        )}

        <button
          onClick={() => onGenerate(activeMode, activeExtension?.id)}
          disabled={isGenerating}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-white/5 hover:bg-white/10 border border-white/10 text-xs font-medium text-zinc-300 transition-colors disabled:opacity-50 cursor-pointer"
          title="Regenerate from the same transcript"
        >
          <RefreshCw className={`w-3 h-3 ${isGenerating ? 'animate-spin' : ''}`} />
          <span>Regenerate</span>
        </button>

        <button
          onClick={handleCopy}
          className="ml-auto flex items-center gap-1 px-2.5 py-1.5 rounded-md text-[11px] font-mono text-zinc-400 hover:text-zinc-100 bg-white/5 hover:bg-white/10 border border-white/5 transition-colors cursor-pointer"
        >
          {copied ? (
            <>
              <Check className="w-3 h-3 text-lime-400" />
              <span className="text-lime-400">Copied</span>
            </>
          ) : (
            <>
              <Copy className="w-3 h-3" />
              <span>Copy</span>
            </>
          )}
        </button>
      </div>

      {summary.speaker_names_stale && (
        <p className="flex items-start gap-2 text-[11px] text-amber-300/90 px-3 py-2 rounded-md bg-amber-500/5 border border-amber-500/20">
          <Info className="w-3.5 h-3.5 shrink-0 mt-0.5" />
          <span>
            A speaker was renamed after this was written, so the text below still uses
            the old label. Action items and the conversation are already up to date.
            Regenerate to update the prose.
          </span>
        </p>
      )}

      {/* The summary reads as a document, not as a card. It is the thing the
          user came here for, so nothing frames it or competes with it. */}
      <div className="text-[13px] text-zinc-200 leading-relaxed font-sans select-text">
        <MarkdownView content={summary.markdown} />
      </div>

      {facts && (
        <MeetingActionItems
          items={facts.action_items}
          speakers={processing?.speakers ?? []}
          onToggle={onToggleActionItem}
          onAddTask={onAddTask}
          onAddAllTasks={onAddAllTasks}
          busyItemId={busyActionItemId}
          isAddingAll={isAddingAllTasks}
        />
      )}

      {facts && (facts.topics.length > 0 || facts.entities.length > 0) && (
        <div className="flex flex-col gap-3 pt-1">
          {facts.topics.length > 0 && (
            <div className="flex flex-col gap-1.5">
              <span className="text-xs font-medium uppercase tracking-wider text-zinc-400">
                Topics
              </span>
              <div className="flex flex-wrap gap-1.5">
                {facts.topics.map((topic) => (
                  <span
                    key={topic.id}
                    className="text-[11px] px-2 py-0.5 rounded-md bg-white/5 border border-white/10 text-zinc-300"
                  >
                    {topic.label}
                  </span>
                ))}
              </div>
            </div>
          )}

          {facts.entities.length > 0 && (
            <div className="flex flex-col gap-1.5">
              <span className="text-xs font-medium uppercase tracking-wider text-zinc-400">
                Mentioned
              </span>
              <div className="flex flex-wrap gap-1.5">
                {facts.entities.map((entity) => (
                  <span
                    key={entity.id}
                    className="text-[11px] px-2 py-0.5 rounded-md bg-white/5 border border-white/10 text-zinc-300"
                    title={entity.kind.toLowerCase()}
                  >
                    {entity.name}
                  </span>
                ))}
              </div>
            </div>
          )}
        </div>
      )}

      <MeetingRelatedList related={related} onSelect={onSelectRelated} />
    </div>
  );
};
