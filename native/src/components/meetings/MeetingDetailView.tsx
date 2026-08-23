import React, { useState } from 'react';
import { Meeting, Scribble, MeetingActionItem } from '../../types';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Input } from '@/components/ui/input';
import { MarkdownView } from '../common/MarkdownView';
import { ConfirmationModal } from '../common/ConfirmationModal';
import {
  Mic,
  Square,
  Sparkles,
  BookmarkPlus,
  Trash2,
  Share2,
  Calendar,
  Clock,
  Video,
  Users,
  CheckCircle2,
  Circle,
  HelpCircle,
  FileText,
  Copy,
  ExternalLink,
  Edit3,
  Check,
  ChevronDown,
  ChevronUp,
  Layers,
  ArrowRight,
  ListTodo,
} from 'lucide-react';

interface MeetingDetailViewProps {
  meeting: Meeting;
  linkedScribbles: Scribble[];
  isRecordingThisMeeting: boolean;
  onStartRecording: (meetingId: string) => Promise<void>;
  onStopRecording: (meetingId: string) => Promise<void>;
  onEnrichMeeting: (meetingId: string) => Promise<void>;
  onSaveScribbleFromMeeting: (
    content: string,
    title?: string,
    segment?: string
  ) => Promise<void>;
  onUpdateMeeting: (updated: Meeting) => Promise<void>;
  onDeleteMeeting: (meetingId: string) => Promise<void>;
  onNavigateToScribble?: (scribbleId?: string) => void;
  disableRecording?: boolean;
}

type DetailTab = 'notes' | 'decisions_actions' | 'questions' | 'transcript' | 'scribbles';

export const MeetingDetailView: React.FC<MeetingDetailViewProps> = ({
  meeting,
  linkedScribbles,
  isRecordingThisMeeting,
  onStartRecording,
  onStopRecording,
  onEnrichMeeting,
  onSaveScribbleFromMeeting,
  onUpdateMeeting,
  onDeleteMeeting,
  onNavigateToScribble,
  disableRecording,
}) => {
  const [activeTab, setActiveTab] = useState<DetailTab>('notes');
  const [isEditingTitle, setIsEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState(meeting.title);
  const [isEditingNotes, setIsEditingNotes] = useState(false);
  const [notesDraft, setNotesDraft] = useState(meeting.notes);
  const [deleteModalOpen, setDeleteModalOpen] = useState(false);
  const [selectedText, setSelectedText] = useState('');
  const [enriching, setEnriching] = useState(false);
  const [actionItemDraft, setActionItemDraft] = useState('');
  const [copiedSection, setCopiedSection] = useState<string | null>(null);

  // Sync draft when meeting changes
  React.useEffect(() => {
    setTitleDraft(meeting.title);
    setNotesDraft(meeting.notes);
  }, [meeting]);

  const handleSaveTitle = async () => {
    if (titleDraft.trim() && titleDraft !== meeting.title) {
      await onUpdateMeeting({ ...meeting, title: titleDraft.trim() });
    }
    setIsEditingTitle(false);
  };

  const handleSaveNotes = async () => {
    if (notesDraft !== meeting.notes) {
      await onUpdateMeeting({ ...meeting, notes: notesDraft });
    }
    setIsEditingNotes(false);
  };

  const handleEnrich = async () => {
    setEnriching(true);
    try {
      await onEnrichMeeting(meeting.id);
    } finally {
      setEnriching(false);
    }
  };

  const handleToggleActionItem = async (actionId: string) => {
    const updatedActions = meeting.action_items.map((item) => {
      if (item.id === actionId) {
        return {
          ...item,
          status: item.status === 'done' ? 'todo' : 'done',
        };
      }
      return item;
    });
    await onUpdateMeeting({ ...meeting, action_items: updatedActions });
  };

  const handleAddActionItem = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!actionItemDraft.trim()) return;

    const newItem: MeetingActionItem = {
      id: `act_${Date.now()}`,
      title: actionItemDraft.trim(),
      assignee: null,
      due_date: null,
      priority: 'medium',
      status: 'todo',
    };

    await onUpdateMeeting({
      ...meeting,
      action_items: [...meeting.action_items, newItem],
    });
    setActionItemDraft('');
  };

  const handleCopy = (text: string, sectionKey: string) => {
    navigator.clipboard.writeText(text);
    setCopiedSection(sectionKey);
    setTimeout(() => setCopiedSection(null), 2000);
  };

  const handleExportMarkdown = () => {
    const content = meeting.notes || meeting.transcript;
    const blob = new Blob([content], { type: 'text/markdown;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `${meeting.title.toLowerCase().replace(/\s+/g, '_')}_notes.md`;
    link.click();
    URL.revokeObjectURL(url);
  };

  const handleTranscriptMouseUp = () => {
    const sel = window.getSelection()?.toString().trim();
    if (sel && sel.length > 5) {
      setSelectedText(sel);
    } else {
      setSelectedText('');
    }
  };

  const startDate = meeting.scheduled_start ? new Date(meeting.scheduled_start) : new Date(meeting.created_at);
  const formattedDateTime = startDate.toLocaleDateString(undefined, {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });

  return (
    <div className="flex-1 flex flex-col h-full bg-card rounded-xl border border-border overflow-hidden shadow-xs">
      {/* Header Banner & Meta */}
      <div className="p-5 md:p-6 border-b border-border bg-gradient-to-r from-card via-card/95 to-muted/20 space-y-4">
        {/* Top meta row */}
        <div className="flex items-center justify-between gap-3 flex-wrap">
          <div className="flex items-center gap-2">
            <Badge
              variant="outline"
              className="text-[10px] uppercase font-mono tracking-wider px-2 py-0.5 border-primary/30 text-primary bg-primary/5 flex items-center gap-1.5"
            >
              <Video className="w-3 h-3" />
              <span>{meeting.provider.replace('_', ' ')}</span>
            </Badge>

            <Badge
              variant={
                meeting.status === 'recording'
                  ? 'destructive'
                  : meeting.status === 'completed'
                  ? 'outline'
                  : 'secondary'
              }
              className={`text-[10px] uppercase font-mono px-2 py-0.5 ${
                meeting.status === 'recording'
                  ? 'animate-pulse'
                  : meeting.status === 'completed'
                  ? 'border-emerald-500/40 text-emerald-500 bg-emerald-500/5'
                  : ''
              }`}
            >
              {meeting.status}
            </Badge>

            {meeting.series_id && (
              <Badge variant="outline" className="text-[10px] font-mono px-2 py-0.5 border-border text-muted-foreground">
                Series Occurrence
              </Badge>
            )}
          </div>

          <div className="flex items-center gap-1.5 text-xs text-muted-foreground font-mono">
            <Calendar className="w-3.5 h-3.5" />
            <span>{formattedDateTime}</span>
          </div>
        </div>

        {/* Title row */}
        <div className="flex items-start justify-between gap-4">
          {isEditingTitle ? (
            <div className="flex items-center gap-2 flex-1 max-w-2xl">
              <Input
                value={titleDraft}
                onChange={(e) => setTitleDraft(e.target.value)}
                className="text-lg font-bold h-9"
                autoFocus
                onKeyDown={(e) => {
                  if (e.key === 'Enter') handleSaveTitle();
                  if (e.key === 'Escape') setIsEditingTitle(false);
                }}
              />
              <Button size="sm" onClick={handleSaveTitle} className="h-9 text-xs">
                Save
              </Button>
              <Button size="sm" variant="ghost" onClick={() => setIsEditingTitle(false)} className="h-9 text-xs">
                Cancel
              </Button>
            </div>
          ) : (
            <div className="flex items-center gap-2 group flex-1">
              <h2 className="text-xl md:text-2xl font-extrabold text-foreground tracking-tight">
                {meeting.title}
              </h2>
              <button
                type="button"
                onClick={() => setIsEditingTitle(true)}
                className="opacity-0 group-hover:opacity-100 transition-opacity p-1 text-muted-foreground hover:text-foreground"
                title="Rename Meeting"
              >
                <Edit3 className="w-4 h-4" />
              </button>
            </div>
          )}
        </div>

        {/* Participants chips */}
        {meeting.participants.length > 0 && (
          <div className="flex items-center gap-2 flex-wrap pt-1">
            <Users className="w-3.5 h-3.5 text-muted-foreground shrink-0" />
            {meeting.participants.map((person, idx) => (
              <span
                key={idx}
                className="text-[11px] px-2 py-0.5 rounded-md bg-muted/40 border border-border text-foreground font-medium"
              >
                {person}
              </span>
            ))}
          </div>
        )}

        {/* Meeting Control Action Bar */}
        <div className="pt-2 flex items-center justify-between gap-2 flex-wrap border-t border-border/60">
          <div className="flex items-center gap-2">
            {/* Start / Stop Recording */}
            {isRecordingThisMeeting || meeting.status === 'recording' ? (
              <Button
                variant="destructive"
                size="sm"
                onClick={() => onStopRecording(meeting.id)}
                className="text-xs gap-1.5 animate-pulse"
              >
                <Square className="w-3.5 h-3.5" />
                <span>Stop Recording</span>
              </Button>
            ) : (
              <Button
                variant="default"
                size="sm"
                onClick={() => onStartRecording(meeting.id)}
                disabled={disableRecording}
                className="text-xs gap-1.5 bg-emerald-600 hover:bg-emerald-700 text-white"
              >
                <Mic className="w-3.5 h-3.5" />
                <span>Start Recording</span>
              </Button>
            )}

            {/* AI Re-enrich */}
            <Button
              variant="outline"
              size="sm"
              onClick={handleEnrich}
              disabled={enriching || isRecordingThisMeeting}
              className="text-xs gap-1.5"
            >
              <Sparkles className={`w-3.5 h-3.5 text-primary ${enriching ? 'animate-spin' : ''}`} />
              <span>{enriching ? 'Analyzing…' : 'AI Extract & Enrich'}</span>
            </Button>

            {/* Save Whole Meeting as Scribble */}
            <Button
              variant="outline"
              size="sm"
              onClick={() =>
                onSaveScribbleFromMeeting(
                  `# Meeting: ${meeting.title}\n\n${meeting.summary ? `## Summary\n${meeting.summary}\n\n` : ''}## Notes\n${meeting.notes || meeting.transcript}`,
                  meeting.title,
                  'full_meeting'
                )
              }
              className="text-xs gap-1.5 text-amber-500 border-amber-500/30 hover:bg-amber-500/10"
              title="Promote entire meeting notes into a persistent atomic Scribble"
            >
              <BookmarkPlus className="w-3.5 h-3.5" />
              <span>Save as Scribble</span>
            </Button>
          </div>

          <div className="flex items-center gap-1.5">
            <Button
              variant="ghost"
              size="sm"
              onClick={handleExportMarkdown}
              className="text-xs gap-1.5 text-muted-foreground hover:text-foreground"
              title="Export Meeting Markdown"
            >
              <Share2 className="w-3.5 h-3.5" />
              <span>Export</span>
            </Button>

            <Button
              variant="ghost"
              size="icon"
              onClick={() => setDeleteModalOpen(true)}
              className="h-8 w-8 text-muted-foreground hover:text-destructive"
              title="Move Meeting to 30-day Trash"
            >
              <Trash2 className="w-3.5 h-3.5" />
            </Button>
          </div>
        </div>
      </div>

      {/* Navigation Sub-Tabs */}
      <div className="flex items-center border-b border-border bg-muted/10 px-4 gap-2 overflow-x-auto shrink-0 select-none">
        <button
          type="button"
          onClick={() => setActiveTab('notes')}
          className={`py-3 px-3 text-xs font-semibold border-b-2 transition-all flex items-center gap-2 ${
            activeTab === 'notes'
              ? 'border-primary text-primary'
              : 'border-transparent text-muted-foreground hover:text-foreground'
          }`}
        >
          <FileText className="w-3.5 h-3.5" />
          <span>Notes & Summary</span>
        </button>

        <button
          type="button"
          onClick={() => setActiveTab('decisions_actions')}
          className={`py-3 px-3 text-xs font-semibold border-b-2 transition-all flex items-center gap-2 ${
            activeTab === 'decisions_actions'
              ? 'border-primary text-primary'
              : 'border-transparent text-muted-foreground hover:text-foreground'
          }`}
        >
          <ListTodo className="w-3.5 h-3.5" />
          <span>Decisions & Tasks</span>
          {(meeting.decisions.length > 0 || meeting.action_items.length > 0) && (
            <Badge variant="secondary" className="text-[10px] py-0 px-1.5 h-4">
              {meeting.decisions.length + meeting.action_items.length}
            </Badge>
          )}
        </button>

        <button
          type="button"
          onClick={() => setActiveTab('questions')}
          className={`py-3 px-3 text-xs font-semibold border-b-2 transition-all flex items-center gap-2 ${
            activeTab === 'questions'
              ? 'border-primary text-primary'
              : 'border-transparent text-muted-foreground hover:text-foreground'
          }`}
        >
          <HelpCircle className="w-3.5 h-3.5" />
          <span>Open Questions</span>
          {meeting.questions.length > 0 && (
            <Badge variant="secondary" className="text-[10px] py-0 px-1.5 h-4">
              {meeting.questions.length}
            </Badge>
          )}
        </button>

        <button
          type="button"
          onClick={() => setActiveTab('transcript')}
          className={`py-3 px-3 text-xs font-semibold border-b-2 transition-all flex items-center gap-2 ${
            activeTab === 'transcript'
              ? 'border-primary text-primary'
              : 'border-transparent text-muted-foreground hover:text-foreground'
          }`}
        >
          <Mic className="w-3.5 h-3.5" />
          <span>Transcript</span>
        </button>

        <button
          type="button"
          onClick={() => setActiveTab('scribbles')}
          className={`py-3 px-3 text-xs font-semibold border-b-2 transition-all flex items-center gap-2 ${
            activeTab === 'scribbles'
              ? 'border-primary text-primary'
              : 'border-transparent text-muted-foreground hover:text-foreground'
          }`}
        >
          <Layers className="w-3.5 h-3.5" />
          <span>Derived Scribbles</span>
          {linkedScribbles.length > 0 && (
            <Badge variant="outline" className="text-[10px] py-0 px-1.5 h-4 border-amber-500/40 text-amber-500 bg-amber-500/5">
              {linkedScribbles.length}
            </Badge>
          )}
        </button>
      </div>

      {/* Main Tab Content */}
      <div className="flex-1 p-5 md:p-6 overflow-y-auto space-y-6">
        {/* TAB 1: NOTES & SUMMARY */}
        {activeTab === 'notes' && (
          <div className="space-y-6">
            {/* AI Summary Banner (if present) */}
            {meeting.summary && (
              <div className="p-4 rounded-xl bg-primary/5 border border-primary/20 space-y-2">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-1.5 font-bold text-xs text-primary">
                    <Sparkles className="w-3.5 h-3.5" />
                    <span>Executive Summary</span>
                  </div>
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => onSaveScribbleFromMeeting(meeting.summary!, `Summary: ${meeting.title}`, 'summary')}
                    className="h-7 text-[11px] text-amber-500 hover:text-amber-600 gap-1"
                  >
                    <BookmarkPlus className="w-3 h-3" />
                    <span>Save as Scribble</span>
                  </Button>
                </div>
                <MarkdownView content={meeting.summary} />
              </div>
            )}

            {/* Candidate Scribbles Suggestions */}
            {meeting.candidate_scribbles && meeting.candidate_scribbles.length > 0 && (
              <div className="space-y-2.5">
                <h4 className="text-xs font-bold text-muted-foreground uppercase tracking-wider flex items-center gap-1.5">
                  <Sparkles className="w-3.5 h-3.5 text-amber-500" />
                  <span>AI Suggested Knowledge Candidates</span>
                </h4>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-2.5">
                  {meeting.candidate_scribbles.map((candidate, idx) => (
                    <div
                      key={idx}
                      className="p-3 rounded-lg bg-card border border-border/80 hover:border-amber-500/40 transition-all flex items-start justify-between gap-2 shadow-xs"
                    >
                      <p className="text-xs text-foreground leading-relaxed flex-1">{candidate}</p>
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => onSaveScribbleFromMeeting(candidate, undefined, `candidate_${idx}`)}
                        className="text-[11px] h-7 px-2 border-amber-500/30 text-amber-500 hover:bg-amber-500/10 shrink-0 gap-1"
                      >
                        <BookmarkPlus className="w-3 h-3" />
                        <span>Scribble</span>
                      </Button>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Meeting Notes */}
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <h3 className="text-sm font-bold text-foreground flex items-center gap-2">
                  <FileText className="w-4 h-4 text-primary" />
                  <span>Meeting Notes</span>
                </h3>
                <div className="flex items-center gap-2">
                  {isEditingNotes ? (
                    <div className="flex items-center gap-2">
                      <Button size="sm" onClick={handleSaveNotes} className="h-7 text-xs">
                        Done
                      </Button>
                      <Button size="sm" variant="ghost" onClick={() => setIsEditingNotes(false)} className="h-7 text-xs">
                        Cancel
                      </Button>
                    </div>
                  ) : (
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => setIsEditingNotes(true)}
                      className="h-7 text-xs gap-1"
                    >
                      <Edit3 className="w-3 h-3" />
                      <span>Edit Notes</span>
                    </Button>
                  )}
                </div>
              </div>

              {isEditingNotes ? (
                <textarea
                  value={notesDraft}
                  onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) => setNotesDraft(e.target.value)}
                  placeholder="Write or edit meeting notes, discussion points, takeaways..."
                  className="w-full min-h-[260px] rounded-md border border-input bg-background px-3 py-2 text-xs font-mono shadow-xs focus:outline-hidden focus:ring-1 focus:ring-ring text-foreground"
                />
              ) : meeting.notes.trim() ? (
                <div className="p-4 rounded-xl border border-border/80 bg-background/50">
                  <MarkdownView content={meeting.notes} />
                </div>
              ) : (
                <div className="p-8 text-center border border-dashed border-border rounded-xl space-y-2">
                  <FileText className="w-8 h-8 text-muted-foreground/40 mx-auto" />
                  <p className="text-xs text-muted-foreground">No notes added yet.</p>
                  <Button size="sm" variant="outline" onClick={() => setIsEditingNotes(true)} className="text-xs">
                    Write Notes
                  </Button>
                </div>
              )}
            </div>
          </div>
        )}

        {/* TAB 2: DECISIONS & ACTIONS */}
        {activeTab === 'decisions_actions' && (
          <div className="space-y-6">
            {/* Key Decisions */}
            <div className="space-y-3">
              <h3 className="text-sm font-bold text-foreground flex items-center gap-2">
                <CheckCircle2 className="w-4 h-4 text-emerald-500" />
                <span>Explicit Decisions Made</span>
              </h3>

              {meeting.decisions.length === 0 ? (
                <p className="text-xs text-muted-foreground italic">No explicit decisions recorded yet.</p>
              ) : (
                <div className="space-y-2">
                  {meeting.decisions.map((dec, idx) => (
                    <div
                      key={idx}
                      className="p-3 rounded-lg bg-emerald-500/5 border border-emerald-500/20 flex items-start justify-between gap-3 shadow-xs"
                    >
                      <div className="flex items-start gap-2 flex-1">
                        <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0 mt-0.5" />
                        <span className="text-xs text-foreground font-medium">{dec}</span>
                      </div>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => onSaveScribbleFromMeeting(dec, `Decision: ${meeting.title}`, `decision_${idx}`)}
                        className="text-[11px] h-6 px-2 text-amber-500 hover:text-amber-600 gap-1 shrink-0"
                      >
                        <BookmarkPlus className="w-3 h-3" />
                        <span>Save as Scribble</span>
                      </Button>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {/* Action Items */}
            <div className="space-y-3 pt-4 border-t border-border/60">
              <h3 className="text-sm font-bold text-foreground flex items-center gap-2">
                <ListTodo className="w-4 h-4 text-blue-500" />
                <span>Action Items & Tasks</span>
              </h3>

              {/* Add Action Item Form */}
              <form onSubmit={handleAddActionItem} className="flex items-center gap-2">
                <Input
                  value={actionItemDraft}
                  onChange={(e) => setActionItemDraft(e.target.value)}
                  placeholder="Add a new action item..."
                  className="text-xs h-8 flex-1"
                />
                <Button type="submit" size="sm" className="h-8 text-xs">
                  Add Task
                </Button>
              </form>

              {meeting.action_items.length === 0 ? (
                <p className="text-xs text-muted-foreground italic">No action items created yet.</p>
              ) : (
                <div className="space-y-2">
                  {meeting.action_items.map((item) => (
                    <div
                      key={item.id}
                      className={`p-3 rounded-lg border transition-all flex items-center justify-between gap-3 shadow-xs ${
                        item.status === 'done'
                          ? 'bg-muted/20 border-border opacity-70'
                          : 'bg-card border-border/80 hover:border-primary/40'
                      }`}
                    >
                      <div className="flex items-center gap-3 flex-1 min-w-0">
                        <button
                          type="button"
                          onClick={() => handleToggleActionItem(item.id)}
                          className="text-muted-foreground hover:text-primary transition-colors cursor-pointer shrink-0"
                        >
                          {item.status === 'done' ? (
                            <CheckCircle2 className="w-4 h-4 text-emerald-500" />
                          ) : (
                            <Circle className="w-4 h-4" />
                          )}
                        </button>
                        <span
                          className={`text-xs text-foreground truncate ${
                            item.status === 'done' ? 'line-through text-muted-foreground' : 'font-medium'
                          }`}
                        >
                          {item.title}
                        </span>
                        {item.assignee && (
                          <Badge variant="secondary" className="text-[10px] py-0 px-1.5">
                            {item.assignee}
                          </Badge>
                        )}
                        {item.due_date && (
                          <span className="text-[10px] text-muted-foreground font-mono">
                            Due {item.due_date}
                          </span>
                        )}
                      </div>

                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() =>
                          onSaveScribbleFromMeeting(
                            `Action Item: ${item.title}${item.assignee ? ` (Assignee: ${item.assignee})` : ''}`,
                            item.title,
                            `action_${item.id}`
                          )
                        }
                        className="text-[11px] h-6 px-2 text-amber-500 hover:text-amber-600 gap-1 shrink-0"
                        title="Promote task to Scribble"
                      >
                        <BookmarkPlus className="w-3 h-3" />
                        <span>Scribble</span>
                      </Button>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        )}

        {/* TAB 3: OPEN QUESTIONS */}
        {activeTab === 'questions' && (
          <div className="space-y-4">
            <h3 className="text-sm font-bold text-foreground flex items-center gap-2">
              <HelpCircle className="w-4 h-4 text-purple-500" />
              <span>Unresolved Exploration Questions</span>
            </h3>

            {meeting.questions.length === 0 ? (
              <p className="text-xs text-muted-foreground italic">No open questions identified.</p>
            ) : (
              <div className="space-y-2.5">
                {meeting.questions.map((q, idx) => (
                  <div
                    key={idx}
                    className="p-3.5 rounded-lg bg-purple-500/5 border border-purple-500/20 flex items-start justify-between gap-3 shadow-xs"
                  >
                    <div className="flex items-start gap-2.5 flex-1">
                      <HelpCircle className="w-4 h-4 text-purple-500 shrink-0 mt-0.5" />
                      <p className="text-xs text-foreground font-medium leading-relaxed">{q}</p>
                    </div>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => onSaveScribbleFromMeeting(q, `Question: ${q.slice(0, 40)}…`, `question_${idx}`)}
                      className="text-[11px] h-7 px-2 border-amber-500/30 text-amber-500 hover:bg-amber-500/10 shrink-0 gap-1"
                    >
                      <BookmarkPlus className="w-3 h-3" />
                      <span>Explore as Scribble</span>
                    </Button>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {/* TAB 4: TRANSCRIPT */}
        {activeTab === 'transcript' && (
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <h3 className="text-sm font-bold text-foreground flex items-center gap-2">
                <Mic className="w-4 h-4 text-primary" />
                <span>Full Audio Transcript</span>
              </h3>
              <div className="flex items-center gap-2">
                {selectedText && (
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => onSaveScribbleFromMeeting(selectedText, undefined, 'transcript_selection')}
                    className="h-7 text-xs border-amber-500/40 text-amber-500 hover:bg-amber-500/10 gap-1"
                  >
                    <BookmarkPlus className="w-3 h-3" />
                    <span>Save Selection as Scribble</span>
                  </Button>
                )}
                {meeting.transcript && (
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => handleCopy(meeting.transcript, 'transcript')}
                    className="h-7 text-xs gap-1"
                  >
                    {copiedSection === 'transcript' ? <Check className="w-3 h-3 text-emerald-500" /> : <Copy className="w-3 h-3" />}
                    <span>{copiedSection === 'transcript' ? 'Copied' : 'Copy Full'}</span>
                  </Button>
                )}
              </div>
            </div>

            {meeting.transcript.trim() ? (
              <div
                onMouseUp={handleTranscriptMouseUp}
                className="p-4 rounded-xl border border-border/80 bg-background/50 text-xs text-foreground leading-relaxed whitespace-pre-wrap font-sans select-text"
              >
                {meeting.transcript}
              </div>
            ) : (
              <div className="p-8 text-center border border-dashed border-border rounded-xl space-y-2">
                <Mic className="w-8 h-8 text-muted-foreground/40 mx-auto" />
                <p className="text-xs text-muted-foreground">No transcript recorded for this meeting yet.</p>
                <Button
                  size="sm"
                  onClick={() => onStartRecording(meeting.id)}
                  className="text-xs bg-emerald-600 hover:bg-emerald-700 text-white"
                >
                  Start Recording Meeting
                </Button>
              </div>
            )}
          </div>
        )}

        {/* TAB 5: DERIVED SCRIBBLES */}
        {activeTab === 'scribbles' && (
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <h3 className="text-sm font-bold text-foreground flex items-center gap-2">
                  <Layers className="w-4 h-4 text-amber-500" />
                  <span>Scribbles Created From This Meeting</span>
                </h3>
                <p className="text-xs text-muted-foreground">
                  Atomic knowledge pieces promoted from this meeting into Relay's living knowledge graph.
                </p>
              </div>
              <Button
                size="sm"
                variant="outline"
                onClick={() =>
                  onSaveScribbleFromMeeting(
                    meeting.notes || meeting.transcript,
                    `Insight from ${meeting.title}`,
                    'manual_promote'
                  )
                }
                className="text-xs border-amber-500/30 text-amber-500 hover:bg-amber-500/10 gap-1.5"
              >
                <BookmarkPlus className="w-3.5 h-3.5" />
                <span>New Scribble from Meeting</span>
              </Button>
            </div>

            {linkedScribbles.length === 0 ? (
              <div className="p-8 text-center border border-dashed border-border rounded-xl space-y-2">
                <Layers className="w-8 h-8 text-muted-foreground/40 mx-auto" />
                <p className="text-xs text-muted-foreground">No Scribbles have been extracted from this meeting yet.</p>
                <p className="text-[11px] text-muted-foreground">
                  You can promote notes, summaries, decisions, action items, or selected transcript text to independent Scribbles at any time.
                </p>
              </div>
            ) : (
              <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                {linkedScribbles.map((sc) => (
                  <div
                    key={sc.id}
                    className="p-4 rounded-xl bg-card border border-border/80 hover:border-amber-500/50 transition-all space-y-2 shadow-xs group"
                  >
                    <div className="flex items-center justify-between">
                      <Badge
                        variant="outline"
                        className="text-[10px] uppercase font-mono py-0 px-1.5 border-amber-500/30 text-amber-500 bg-amber-500/5"
                      >
                        Scribble
                      </Badge>
                      {onNavigateToScribble && (
                        <button
                          type="button"
                          onClick={() => onNavigateToScribble(sc.id)}
                          className="text-xs text-primary hover:underline flex items-center gap-1 font-medium cursor-pointer"
                        >
                          <span>Open</span>
                          <ArrowRight className="w-3 h-3" />
                        </button>
                      )}
                    </div>
                    <h4 className="text-sm font-bold text-foreground truncate">{sc.title}</h4>
                    <p className="text-xs text-muted-foreground line-clamp-3 leading-relaxed">
                      {sc.content}
                    </p>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}
      </div>

      {/* 30-Day Trash Confirmation Modal */}
      <ConfirmationModal
        isOpen={deleteModalOpen}
        onCancel={() => setDeleteModalOpen(false)}
        onConfirm={async () => {
          await onDeleteMeeting(meeting.id);
          setDeleteModalOpen(false);
        }}
        title={meeting.series_id ? "Delete Meeting Occurrence" : "Delete Meeting"}
        description={`Are you sure you want to move this occurrence ("${meeting.title}") to the 30-day Trash? Any Scribbles derived from this meeting will remain intact and will not be deleted.`}
        confirmLabel="Move to Trash"
        variant="destructive"
      />
    </div>
  );
};
