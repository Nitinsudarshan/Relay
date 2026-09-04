import React, { useState } from 'react';
import {
  Check,
  ChevronDown,
  Copy,
  Info,
  RefreshCw,
  Share2,
  Sparkles,
} from 'lucide-react';
import { MarkdownView } from '../common/MarkdownView';
import type {
  ActionItem,
  MeetingExtension,
  MeetingProcessing,
  RelatedMeeting,
  SharedDocument,
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
  /**
   * Renders the meeting as one shareable document. Returns the text, which the
   * tab copies — the backend composes it so the header stays counted rather
   * than reassembled in the UI.
   */
  onShare: (options: ShareSelection) => Promise<SharedDocument>;
}

/** Which parts of a meeting go into a shared document. */
export interface ShareSelection {
  summary: boolean;
  actionItems: boolean;
  decisions: boolean;
  conversation: boolean;
  notes: boolean;
}

const DEFAULT_SHARE: ShareSelection = {
  summary: true,
  actionItems: true,
  decisions: true,
  conversation: false,
  notes: false,
};

/**
 * The share menu's checkboxes.
 *
 * The conversation and the notes are off by default for different reasons: the
 * conversation turns a one-page summary into forty, and notes are often private
 * working material. Both are deliberate choices the user makes each time rather
 * than a setting they configure once and forget.
 */
const SHARE_PARTS: {
  key: keyof ShareSelection;
  label: string;
  hint?: string;
}[] = [
  { key: 'summary', label: 'Summary' },
  { key: 'actionItems', label: 'To-dos' },
  { key: 'decisions', label: 'Decisions' },
  { key: 'conversation', label: 'Full conversation', hint: 'long' },
  { key: 'notes', label: 'Your notes', hint: 'private by default' },
];

/**
 * The default meeting view: what mattered, what was decided, what needs doing.
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
  onShare,
}) => {
  const [copied, setCopied] = useState(false);
  const [modeMenuOpen, setModeMenuOpen] = useState(false);
  const [extensionMenuOpen, setExtensionMenuOpen] = useState(false);
  const [shareMenuOpen, setShareMenuOpen] = useState(false);
  const [shareSelection, setShareSelection] = useState<ShareSelection>(DEFAULT_SHARE);
  const [shareState, setShareState] = useState<'idle' | 'working' | 'done' | 'error'>(
    'idle',
  );
  const [shareNote, setShareNote] = useState<string | null>(null);

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

  /**
   * Builds the shareable document and puts it on the clipboard.
   *
   * Copying rather than writing a file: the destination is almost always a
   * message or a document somebody else already has open, and a file in
   * Downloads is a second step for no benefit. The suggested filename comes
   * back anyway, for the case where the user does want to save it.
   */
  const handleShare = async () => {
    setShareState('working');
    setShareNote(null);
    try {
      const document = await onShare(shareSelection);
      await navigator.clipboard.writeText(document.contents);
      setShareState('done');
      setShareNote(`Copied — ${document.includes}`);
      setTimeout(() => {
        setShareState('idle');
        setShareNote(null);
      }, 4000);
    } catch (err) {
      console.error('Failed to share meeting:', err);
      setShareState('error');
      setShareNote(
        typeof err === 'object' && err !== null && 'message' in err
          ? String((err as { message: unknown }).message)
          : 'Could not build the document.',
      );
    }
  };

  if (!summary) {
    const hasTranscript = canSummarize(processing);
    return (
      <div className="flex-1 overflow-y-auto py-16 px-4 text-center flex flex-col items-center justify-center gap-3">
        <div className="w-12 h-12 rounded-lg bg-muted/40 border border-border flex items-center justify-center text-muted-foreground mb-1">
          <Sparkles className="w-6 h-6 text-primary" />
        </div>
        <h4 className="text-sm font-semibold text-foreground">No summary yet</h4>
        <p className="text-xs text-muted-foreground max-w-sm">
          {hasTranscript
            ? 'Relay will read the transcript, work out what was decided and who owns what, and write it up.'
            : 'This meeting has no transcribed speech yet. The recording and raw transcript are unaffected.'}
        </p>
        {canGenerate && (
          <button
            onClick={() => onGenerate()}
            disabled={isGenerating || !hasTranscript}
            className="mt-2 flex items-center gap-1.5 px-4 py-2 rounded-lg bg-primary hover:bg-primary/90 text-primary-foreground text-xs font-semibold transition-colors disabled:opacity-40 disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
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
      {/* Mode and extension pickers */}
      <div className="flex items-center gap-2 flex-wrap">
        <div className="relative">
          <button
            onClick={() => {
              setModeMenuOpen((v) => !v);
              setExtensionMenuOpen(false);
            }}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-card hover:bg-accent border border-border text-xs font-medium text-foreground transition-colors cursor-pointer"
          >
            <span className="text-muted-foreground">Summary</span>
            <span>{SUMMARY_MODES.find((m) => m.value === activeMode)?.label}</span>
            <ChevronDown className="w-3 h-3 text-muted-foreground" />
          </button>
          {modeMenuOpen && (
            <div className="absolute z-20 mt-1 w-48 rounded-lg bg-popover border border-border shadow-lg overflow-hidden">
              {SUMMARY_MODES.map((mode) => (
                <button
                  key={mode.value}
                  onClick={() => {
                    setModeMenuOpen(false);
                    onGenerate(mode.value, activeExtension?.id);
                  }}
                  className={`w-full flex items-center justify-between px-3 py-2 text-xs text-left hover:bg-accent transition-colors cursor-pointer ${
                    mode.value === activeMode ? 'text-primary font-semibold' : 'text-foreground'
                  }`}
                >
                  <span>{mode.label}</span>
                  <span className="text-[10px] text-muted-foreground">{mode.hint}</span>
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
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-card hover:bg-accent border border-border text-xs font-medium text-foreground transition-colors cursor-pointer"
            >
              <span className="text-muted-foreground">Extension</span>
              <span>{activeExtension?.name ?? 'Default'}</span>
              <ChevronDown className="w-3 h-3 text-muted-foreground" />
            </button>
            {extensionMenuOpen && (
              <div className="absolute z-20 mt-1 w-52 rounded-lg bg-popover border border-border shadow-lg overflow-hidden">
                {extensions.map((extension) => (
                  <button
                    key={extension.id}
                    onClick={() => {
                      setExtensionMenuOpen(false);
                      onGenerate(activeMode, extension.id);
                    }}
                    className={`w-full px-3 py-2 text-xs text-left hover:bg-accent transition-colors cursor-pointer ${
                      extension.id === activeExtension?.id
                        ? 'text-primary font-semibold'
                        : 'text-foreground'
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
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-card hover:bg-accent border border-border text-xs font-medium text-foreground transition-colors disabled:opacity-50 cursor-pointer"
          title="Regenerate from the same transcript"
        >
          <RefreshCw className={`w-3 h-3 ${isGenerating ? 'animate-spin' : ''}`} />
          <span>Regenerate</span>
        </button>

        <div className="ml-auto flex items-center gap-2">
          <div className="relative">
            <button
              onClick={() => {
                setShareMenuOpen((v) => !v);
                setModeMenuOpen(false);
                setExtensionMenuOpen(false);
              }}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-card hover:bg-accent border border-border text-xs font-medium text-foreground transition-colors cursor-pointer"
              title="Build one document with the meeting's date, participants and duration in front of the summary"
            >
              <Share2 className="w-3 h-3 text-primary" />
              <span>Share</span>
              <ChevronDown className="w-3 h-3 text-muted-foreground" />
            </button>

            {shareMenuOpen && (
              <div className="absolute right-0 z-20 mt-1 w-64 rounded-lg bg-popover border border-border shadow-lg overflow-hidden p-2 space-y-1">
                <p className="text-[11px] text-muted-foreground px-1 pb-1">
                  The date, duration and participants are always included, so the
                  summary carries its own provenance.
                </p>
                {SHARE_PARTS.map((part) => (
                  <label
                    key={part.key}
                    className="flex items-center gap-2 px-1 py-1 rounded hover:bg-accent cursor-pointer text-xs text-foreground"
                  >
                    <input
                      type="checkbox"
                      checked={shareSelection[part.key]}
                      onChange={(e) =>
                        setShareSelection((prev) => ({
                          ...prev,
                          [part.key]: e.target.checked,
                        }))
                      }
                      className="accent-primary"
                    />
                    <span className="flex-1">{part.label}</span>
                    {part.hint && (
                      <span className="text-[10px] text-muted-foreground">
                        {part.hint}
                      </span>
                    )}
                  </label>
                ))}
                <button
                  onClick={() => {
                    setShareMenuOpen(false);
                    void handleShare();
                  }}
                  disabled={shareState === 'working'}
                  className="w-full mt-1 flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-md bg-primary hover:bg-primary/90 text-primary-foreground text-xs font-semibold transition-colors disabled:opacity-50 cursor-pointer"
                >
                  {shareState === 'working' ? (
                    <RefreshCw className="w-3 h-3 animate-spin" />
                  ) : (
                    <Copy className="w-3 h-3" />
                  )}
                  Copy document
                </button>
              </div>
            )}
          </div>

          <button
            onClick={handleCopy}
            className="flex items-center gap-1 px-2.5 py-1.5 rounded-lg text-[11px] font-mono text-muted-foreground hover:text-foreground bg-card hover:bg-accent border border-border transition-colors cursor-pointer"
            title="Copy the summary prose alone"
          >
            {copied ? (
              <>
                <Check className="w-3 h-3 text-emerald-600 dark:text-emerald-400" />
                <span className="text-emerald-600 dark:text-emerald-400 font-sans">Copied</span>
              </>
            ) : (
              <>
                <Copy className="w-3 h-3" />
                <span>Copy</span>
              </>
            )}
          </button>
        </div>
      </div>

      {shareNote && (
        <p
          className={`flex items-center gap-1.5 text-[11px] ${
            shareState === 'error'
              ? 'text-amber-800 dark:text-amber-200'
              : 'text-emerald-700 dark:text-emerald-300'
          }`}
          aria-live="polite"
        >
          {shareState === 'error' ? (
            <Info className="w-3.5 h-3.5 shrink-0" />
          ) : (
            <Check className="w-3.5 h-3.5 shrink-0" />
          )}
          {shareNote}
        </p>
      )}

      {summary.speaker_names_stale && (
        <p className="flex items-start gap-2 text-[11px] text-amber-600 dark:text-amber-400 px-3 py-2 rounded-lg bg-amber-500/10 border border-amber-500/20">
          <Info className="w-3.5 h-3.5 shrink-0 mt-0.5" />
          <span>
            A speaker was renamed after this was written, so the text below still uses
            the old label. Action items and the conversation are already up to date.
            Regenerate to update the prose.
          </span>
        </p>
      )}

      <div className="text-[13px] text-foreground/90 leading-relaxed font-sans select-text">
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
              <span className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
                Topics
              </span>
              <div className="flex flex-wrap gap-1.5">
                {facts.topics.map((topic) => (
                  <span
                    key={topic.id}
                    className="text-[11px] px-2 py-0.5 rounded-md bg-muted border border-border text-foreground"
                  >
                    {topic.label}
                  </span>
                ))}
              </div>
            </div>
          )}

          {facts.entities.length > 0 && (
            <div className="flex flex-col gap-1.5">
              <span className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
                Mentioned
              </span>
              <div className="flex flex-wrap gap-1.5">
                {facts.entities.map((entity) => (
                  <span
                    key={entity.id}
                    className="text-[11px] px-2 py-0.5 rounded-md bg-muted border border-border text-foreground"
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

