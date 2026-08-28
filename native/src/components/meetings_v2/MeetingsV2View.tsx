import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  Mic,
  Square,
  Play,
  Pause,
  Clock,
  FileText,
  RefreshCw,
  Layers,
  Sparkles,
  Trash2,
  AlertTriangle,
  MessageSquare,
  Terminal,
  NotebookPen,
  ListTodo,
} from 'lucide-react';
import { ConfirmationModal } from '../common/ConfirmationModal';
import {
  ActionItem,
  AppSettings,
  LiveTranscriptUpdate,
  MeetingExtension,
  MeetingNotes,
  MeetingProcessing,
  MeetingProcessingIndexEntry,
  MeetingSession,
  MeetingTaskPushResult,
  RelatedMeeting,
  SummaryMode,
  TranscriptSegment,
} from '../../types';
import { MeetingConversationTab } from './MeetingConversationTab';
import { MeetingProcessingStatus } from './MeetingProcessingStatus';
import {
  MeetingRawTranscriptTab,
  RawTranscriptHidden,
} from './MeetingRawTranscriptTab';
import { MeetingNotesTab } from './MeetingNotesTab';
import { MeetingSummaryTab } from './MeetingSummaryTab';
import { meetingTitle, meetingTypeLabel } from './meetingProcessing';

/** States in which a session still owns the recorder. */
const ACTIVE_STATES = ['STARTING', 'RECORDING', 'PAUSED', 'STOPPING', 'FINALIZING'];

/** See the recording pill: events alone cannot keep a long-lived view honest. */
const RECONCILE_INTERVAL_MS = 1000;
const TIMER_TICK_MS = 250;

type MeetingTab = 'summary' | 'notes' | 'conversation' | 'raw';

/** The subset of meeting settings this view needs to render. */
interface MeetingsUiSettings {
  showRawTranscript: boolean;
  generateConversationTranscript: boolean;
}

const DEFAULT_MEETING_UI_SETTINGS: MeetingsUiSettings = {
  showRawTranscript: true,
  generateConversationTranscript: true,
};

const TABS: { key: MeetingTab; label: string; icon: typeof Sparkles }[] = [
  { key: 'summary', label: 'Summary', icon: Sparkles },
  // Second, not last: notes are written *during* a meeting, and a tab buried
  // behind the transcript is a tab nobody reaches while one is running.
  { key: 'notes', label: 'Notes', icon: NotebookPen },
  { key: 'conversation', label: 'Conversation', icon: MessageSquare },
  { key: 'raw', label: 'Raw Transcript', icon: Terminal },
];

export const MeetingsV2View: React.FC = () => {
  const [sessions, setSessions] = useState<MeetingSession[]>([]);
  const [activeSession, setActiveSession] = useState<MeetingSession | null>(null);
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [transcriptSegments, setTranscriptSegments] = useState<TranscriptSegment[]>([]);
  const [liveUpdates, setLiveUpdates] = useState<LiveTranscriptUpdate[]>([]);
  const [isLoading, setIsLoading] = useState<boolean>(true);
  const [isStarting, setIsStarting] = useState<boolean>(false);
  const [isStopping, setIsStopping] = useState<boolean>(false);
  const [isTogglingPause, setIsTogglingPause] = useState<boolean>(false);
  const [isSummarizing, setIsSummarizing] = useState<boolean>(false);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);
  // Summary is the default view. The raw transcript is the debug tab, not the
  // meeting experience.
  const [activeMeetingTab, setActiveMeetingTab] = useState<MeetingTab>('summary');
  const [processing, setProcessing] = useState<MeetingProcessing | null>(null);
  /** The user's own notes for the selected meeting. Source data, never derived. */
  const [notes, setNotes] = useState<MeetingNotes | null>(null);
  const [extensions, setExtensions] = useState<MeetingExtension[]>([]);
  /** Per-meeting derived info for the list, keyed by meeting id. */
  const [processingIndex, setProcessingIndex] = useState<
    Record<string, MeetingProcessingIndexEntry>
  >({});
  const [related, setRelated] = useState<RelatedMeeting[]>([]);
  const [meetingSettings, setMeetingSettings] = useState<MeetingsUiSettings>(
    DEFAULT_MEETING_UI_SETTINGS,
  );
  const [isRenamingSpeaker, setIsRenamingSpeaker] = useState<boolean>(false);
  const [busyActionItemId, setBusyActionItemId] = useState<string | null>(null);
  const [isAddingAllTasks, setIsAddingAllTasks] = useState<boolean>(false);
  const [isPromoting, setIsPromoting] = useState<boolean>(false);
  const [promotedScribbleTitle, setPromotedScribbleTitle] = useState<string | null>(
    null,
  );
  const [meetingTitleInput, setMeetingTitleInput] = useState<string>('');
  const [activeElapsedSec, setActiveElapsedSec] = useState<number>(0);

  const selectedSessionIdRef = useRef<string | null>(null);
  selectedSessionIdRef.current = selectedSessionId;

  /** Recorded seconds as last reported by the backend, plus when that arrived. */
  const durationAnchor = useRef<{ seconds: number; at: number } | null>(null);

  const applyActiveSession = useCallback((next: MeetingSession | null) => {
    if (!next || !ACTIVE_STATES.includes(next.state)) {
      durationAnchor.current = null;
      setActiveSession(null);
      setActiveElapsedSec(0);
      return;
    }
    durationAnchor.current = { seconds: next.duration_seconds || 0, at: performance.now() };
    setActiveSession(next);
  }, []);

  const loadSessions = useCallback(async () => {
    try {
      const listRes = await invoke<MeetingSession[]>('list_meetings_v2');
      setSessions(listRes);
      if (!selectedSessionIdRef.current && listRes.length > 0) {
        setSelectedSessionId(listRes[0].id);
      }
    } catch (err) {
      console.error('Failed to load meetings list:', err);
    } finally {
      setIsLoading(false);
    }

    // The derived index in one call, so the list can show extracted titles and
    // outstanding tasks without a request per row.
    try {
      const index = await invoke<MeetingProcessingIndexEntry[]>(
        'list_meeting_v2_processing',
      );
      setProcessingIndex(
        Object.fromEntries(index.map((entry) => [entry.meeting_id, entry])),
      );
    } catch (err) {
      console.error('Failed to load the meeting processing index:', err);
    }
  }, []);

  // Reconcile the active session against the backend rather than trusting an
  // unbroken stream of events: the recorder, not this view, owns the state.
  useEffect(() => {
    let cancelled = false;

    const reconcile = async () => {
      try {
        const active = await invoke<MeetingSession | null>('get_active_meeting_v2');
        if (!cancelled) {
          applyActiveSession(active);
        }
      } catch (err) {
        console.error('Failed to reconcile active meeting:', err);
      }
    };

    reconcile();
    loadSessions();
    const poll = setInterval(reconcile, RECONCILE_INTERVAL_MS);

    const unlistenState = listen<MeetingSession>('meeting-session-state-changed', (event) => {
      const updated = event.payload;
      applyActiveSession(updated);
      if (ACTIVE_STATES.includes(updated.state)) {
        setSelectedSessionId(updated.id);
      } else {
        setLiveUpdates([]);
      }
      loadSessions();
    });

    const unlistenSegment = listen<TranscriptSegment>('meeting-transcript-segment', (event) => {
      setTranscriptSegments((prev) => {
        const idx = prev.findIndex((s) => s.chunk_index === event.payload.chunk_index);
        if (idx === -1) {
          return [...prev, event.payload];
        }
        const next = [...prev];
        next[idx] = event.payload;
        return next;
      });
    });

    // Live updates are keyed by utterance: a new update for an existing
    // segment_id replaces it as the utterance grows.
    const unlistenLive = listen<LiveTranscriptUpdate>('meeting-live-transcript', (event) => {
      setLiveUpdates((prev) => {
        const idx = prev.findIndex((u) => u.segment_id === event.payload.segment_id);
        if (idx === -1) {
          return [...prev, event.payload];
        }
        const next = [...prev];
        next[idx] = event.payload;
        return next;
      });
    });

    return () => {
      cancelled = true;
      clearInterval(poll);
      unlistenState.then((f) => f());
      unlistenSegment.then((f) => f());
      unlistenLive.then((f) => f());
    };
  }, [applyActiveSession, loadSessions]);

  // Interpolate the timer between reconciliations, and only while recording, so
  // it neither counts paused time nor keeps running after a meeting ends.
  const isRecording = activeSession?.state === 'RECORDING';
  useEffect(() => {
    const update = () => {
      const anchor = durationAnchor.current;
      if (!anchor) {
        setActiveElapsedSec(0);
        return;
      }
      const drift = isRecording ? (performance.now() - anchor.at) / 1000 : 0;
      setActiveElapsedSec(Math.max(0, Math.floor(anchor.seconds + drift)));
    };

    update();
    if (!isRecording) {
      return;
    }
    const timer = setInterval(update, TIMER_TICK_MS);
    return () => clearInterval(timer);
  }, [isRecording, activeSession?.id, activeSession?.duration_seconds]);

  // Fetch the durable transcript when the selection changes.
  useEffect(() => {
    if (!selectedSessionId) {
      setTranscriptSegments([]);
      return;
    }
    invoke<TranscriptSegment[]>('get_meeting_v2_transcript', { sessionId: selectedSessionId })
      .then((segs) => setTranscriptSegments(segs))
      .catch((err) => console.error('Failed to get transcript segments:', err));
  }, [selectedSessionId]);

  // Meeting settings decide which tabs exist, so they are read once on mount.
  useEffect(() => {
    invoke<AppSettings>('get_settings')
      .then((settings) => {
        if (!settings.meetings) return;
        setMeetingSettings({
          showRawTranscript: settings.meetings.show_raw_transcript,
          generateConversationTranscript:
            settings.meetings.generate_conversation_transcript,
        });
      })
      .catch((err) => console.error('Failed to load meeting settings:', err));

    invoke<MeetingExtension[]>('get_meeting_v2_extensions')
      .then(setExtensions)
      .catch((err) => console.error('Failed to load summary extensions:', err));
  }, []);

  const loadRelated = useCallback(async (sessionId: string) => {
    try {
      setRelated(await invoke<RelatedMeeting[]>('get_meeting_v2_related', { sessionId }));
    } catch {
      // A meeting with no extracted metadata has no relations to show. Not an
      // error worth surfacing.
      setRelated([]);
    }
  }, []);

  /**
   * Loads a meeting's derived data, running the deterministic stages first if it
   * has never been processed.
   *
   * This is what makes an older meeting — or one recovered after a crash — gain
   * a conversation view simply by being opened, without the recorder or a
   * migration being involved.
   */
  const loadProcessing = useCallback(
    async (sessionId: string, isActive: boolean) => {
      try {
        let next = await invoke<MeetingProcessing | null>('get_meeting_v2_processing', {
          sessionId,
        });

        if (!next && !isActive) {
          next = await invoke<MeetingProcessing>('prepare_meeting_v2', { sessionId });
        }

        setProcessing(next ?? null);
        if (next?.facts) {
          loadRelated(sessionId);
        } else {
          setRelated([]);
        }
      } catch (err) {
        console.error('Failed to load meeting processing:', err);
        setProcessing(null);
        setRelated([]);
      }
    },
    [loadRelated],
  );

  useEffect(() => {
    if (!selectedSessionId) {
      setProcessing(null);
      setRelated([]);
      setNotes(null);
      return;
    }
    const isActive = activeSession?.id === selectedSessionId;
    loadProcessing(selectedSessionId, isActive);
  }, [selectedSessionId, activeSession?.id, loadProcessing]);

  // Notes load separately from the processing pipeline, because they are source
  // data: they exist for a meeting that has never been processed, and they must
  // stay readable when processing has failed.
  useEffect(() => {
    if (!selectedSessionId) return;
    let cancelled = false;
    setNotes(null);
    invoke<MeetingNotes>('get_meeting_v2_notes', { sessionId: selectedSessionId })
      .then((loaded) => {
        if (!cancelled) setNotes(loaded);
      })
      .catch((err) => {
        console.error('Failed to load meeting notes:', err);
        if (!cancelled) setNotes({ during: '', before: '' });
      });
    return () => {
      cancelled = true;
    };
  }, [selectedSessionId]);

  // The backend emits this whenever derived data changes — including from the
  // automatic run after a recording stops — so the view stays current without
  // polling the pipeline.
  useEffect(() => {
    const unlisten = listen<MeetingProcessing>('meeting-processing-updated', (event) => {
      const updated = event.payload;

      // Keep the list's badges in step with the detail pane, so ticking off an
      // action item or generating a summary is reflected in both without a
      // round trip.
      setProcessingIndex((prev) => ({
        ...prev,
        [updated.meeting_id]: {
          meeting_id: updated.meeting_id,
          title: updated.facts?.title ?? null,
          status: updated.status,
          meeting_type: updated.facts
            ? meetingTypeLabel(updated.facts.meeting_type)
            : null,
          has_summary: !!updated.summary,
          open_action_item_count:
            updated.facts?.action_items.filter((a) => a.status === 'OPEN').length ?? 0,
          action_item_count: updated.facts?.action_items.length ?? 0,
        },
      }));

      if (updated.meeting_id !== selectedSessionIdRef.current) return;
      setProcessing(updated);
      if (updated.facts) {
        loadRelated(updated.meeting_id);
      }
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [loadRelated]);

  const handleStartRecording = async () => {
    if (isStarting) return;
    setIsStarting(true);
    try {
      const title = meetingTitleInput.trim() ? meetingTitleInput.trim() : undefined;
      const newSession = await invoke<MeetingSession>('start_meeting_v2', { title });
      applyActiveSession(newSession);
      setSelectedSessionId(newSession.id);
      setLiveUpdates([]);
      setMeetingTitleInput('');
      loadSessions();
    } catch (err) {
      console.error('Failed to start meeting recording:', err);
    } finally {
      setIsStarting(false);
    }
  };

  const handleStopRecording = async () => {
    if (isStopping || !activeSession) return;
    setIsStopping(true);
    try {
      // Name the session so this cannot stop a later recording.
      const completed = await invoke<MeetingSession>('stop_meeting_v2', {
        sessionId: activeSession.id,
      });
      applyActiveSession(null);
      setSelectedSessionId(completed.id);
      loadSessions();
    } catch (err) {
      console.error('Failed to stop meeting recording:', err);
    } finally {
      setIsStopping(false);
    }
  };

  const handleTogglePause = async () => {
    if (isTogglingPause || !activeSession) return;
    setIsTogglingPause(true);
    try {
      const command = activeSession.state === 'PAUSED' ? 'resume_meeting_v2' : 'pause_meeting_v2';
      const updated = await invoke<MeetingSession>(command, { sessionId: activeSession.id });
      applyActiveSession(updated);
    } catch (err) {
      console.error('Failed to toggle meeting pause:', err);
    } finally {
      setIsTogglingPause(false);
    }
  };

  const handleDeleteSession = async (sessionId: string) => {
    if (deletingId) return;
    setDeletingId(sessionId);
    try {
      await invoke('delete_meeting_v2', { sessionId });
      setSessions((prev) => prev.filter((s) => s.id !== sessionId));
      if (selectedSessionId === sessionId) {
        const remaining = sessions.filter((s) => s.id !== sessionId);
        setSelectedSessionId(remaining.length > 0 ? remaining[0].id : null);
      }
    } catch (err) {
      console.error('Failed to delete meeting session:', err);
    } finally {
      setDeletingId(null);
      setPendingDeleteId(null);
    }
  };

  /**
   * Runs the canonical pipeline. A mode or extension change re-renders from the
   * facts already extracted; only an explicit retry re-runs extraction.
   */
  const handleGenerateSummary = async (
    sessionId: string,
    mode?: SummaryMode,
    extensionId?: string,
    force?: boolean,
  ) => {
    if (isSummarizing) return;
    setIsSummarizing(true);
    try {
      const updated = await invoke<MeetingProcessing>('generate_meeting_v2_summary', {
        sessionId,
        mode: mode ? mode.toLowerCase() : undefined,
        extensionId,
        force: force ?? false,
      });
      setProcessing(updated);
      setActiveMeetingTab('summary');
      loadRelated(sessionId);
    } catch (err) {
      console.error('Failed to generate meeting summary:', err);
      // The pipeline records the failure on the meeting itself, so re-reading it
      // shows the user what actually went wrong instead of nothing.
      invoke<MeetingProcessing | null>('get_meeting_v2_processing', { sessionId })
        .then((p) => setProcessing(p ?? null))
        .catch(() => undefined);
    } finally {
      setIsSummarizing(false);
    }
  };

  const handleRenameSpeaker = async (
    sessionId: string,
    speakerId: string,
    displayName: string | null,
  ) => {
    if (isRenamingSpeaker) return;
    setIsRenamingSpeaker(true);
    try {
      setProcessing(
        await invoke<MeetingProcessing>('rename_meeting_v2_speaker', {
          sessionId,
          speakerId,
          displayName,
        }),
      );
    } catch (err) {
      console.error('Failed to rename speaker:', err);
    } finally {
      setIsRenamingSpeaker(false);
    }
  };

  const handleToggleActionItem = async (sessionId: string, item: ActionItem) => {
    setBusyActionItemId(item.id);
    try {
      setProcessing(
        await invoke<MeetingProcessing>('set_meeting_v2_action_item_status', {
          sessionId,
          actionItemId: item.id,
          done: item.status !== 'DONE',
        }),
      );
    } catch (err) {
      console.error('Failed to update action item:', err);
    } finally {
      setBusyActionItemId(null);
    }
  };

  /**
   * Adds a meeting's to-dos to the Kanban board.
   *
   * `item` adds exactly one; omitting it adds everything not already on the
   * board, which is what makes the button safe to press twice. The backend
   * returns the refreshed processing record, so an added to-do shows as a task
   * without a reload.
   */
  const handleAddTasks = async (sessionId: string, item?: ActionItem) => {
    if (item) {
      setBusyActionItemId(item.id);
    } else {
      setIsAddingAllTasks(true);
    }
    try {
      const results = await invoke<MeetingTaskPushResult[]>(
        'push_meeting_v2_action_items_to_kanban',
        { sessionId, actionItemId: item?.id ?? null },
      );
      const failed = results.filter((r) => r.error);
      if (failed.length > 0) {
        console.error('Some to-dos could not be added as tasks:', failed);
      }
      const refreshed = await invoke<MeetingProcessing | null>('get_meeting_v2_processing', {
        sessionId,
      });
      if (refreshed) setProcessing(refreshed);
    } catch (err) {
      console.error('Failed to add to-dos as tasks:', err);
    } finally {
      setBusyActionItemId(null);
      setIsAddingAllTasks(false);
    }
  };

  const handlePromoteToScribble = async (sessionId: string) => {
    if (isPromoting) return;
    setIsPromoting(true);
    setPromotedScribbleTitle(null);
    try {
      const scribble = await invoke<{ title: string }>('promote_meeting_v2_to_scribble', {
        sessionId,
        includeConversation: false,
      });
      setPromotedScribbleTitle(scribble.title);
    } catch (err) {
      console.error('Failed to turn this meeting into a Scribble:', err);
    } finally {
      setIsPromoting(false);
    }
  };

  /**
   * Saves the user's notes.
   *
   * Writes `notes.json` and nothing else: no summary is regenerated and no facts
   * are invalidated, which is what makes it safe to type into this while a
   * meeting is being recorded.
   */
  const handleSaveNotes = async (
    sessionId: string,
    next: { during?: string; before?: string },
  ) => {
    try {
      const saved = await invoke<MeetingNotes>('save_meeting_v2_notes', {
        sessionId,
        during: next.during,
        before: next.before,
      });
      setNotes(saved);
    } catch (err) {
      console.error('Failed to save meeting notes:', err);
    }
  };

  const handleSelectSession = (sessionId: string) => {
    setSelectedSessionId(sessionId);
    setPromotedScribbleTitle(null);
    // Always land on Summary. The raw transcript is a tab you choose, not the
    // default meeting experience.
    setActiveMeetingTab('summary');
  };

  const selectedSession = useMemo(
    () =>
      (activeSession?.id === selectedSessionId ? activeSession : undefined) ||
      sessions.find((s) => s.id === selectedSessionId) ||
      activeSession,
    [sessions, selectedSessionId, activeSession]
  );

  const isSelectedActive = !!activeSession && activeSession.id === selectedSession?.id;
  const isPaused = activeSession?.state === 'PAUSED';
  const isFinalizing =
    activeSession?.state === 'STOPPING' || activeSession?.state === 'FINALIZING';

  const formatDuration = (secs: number) => {
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return `${m}m ${s}s`;
  };

  // Helper for computing word count from durable segments
  const countWords = useCallback((segments: TranscriptSegment[]) => {
    return segments.reduce((acc, seg) => {
      if (seg.status === 'SUCCESS' && seg.text) {
        return acc + seg.text.trim().split(/\s+/).filter(Boolean).length;
      }
      return acc;
    }, 0);
  }, []);

  // Word count for the active recording session
  const activeSessionWords = useMemo(() => {
    const durableWords = countWords(transcriptSegments);
    const liveWords = liveUpdates.reduce((acc, update) => {
      if (update.text) {
        return acc + update.text.trim().split(/\s+/).filter(Boolean).length;
      }
      return acc;
    }, 0);
    return Math.max(durableWords, liveWords, activeSession?.word_count ?? 0);
  }, [countWords, transcriptSegments, liveUpdates, activeSession?.word_count]);

  // Word count for the currently selected session in the details view
  const selectedSessionWords = useMemo(() => {
    if (isSelectedActive) {
      return activeSessionWords;
    }
    const fromSegments = countWords(transcriptSegments);
    return Math.max(fromSegments, selectedSession?.word_count ?? 0);
  }, [isSelectedActive, activeSessionWords, countWords, transcriptSegments, selectedSession?.word_count]);

  const pendingDeleteSession = sessions.find((s) => s.id === pendingDeleteId);
  const latestLatency = liveUpdates.length
    ? liveUpdates[liveUpdates.length - 1].latency_ms
    : undefined;
  const backlog = activeSession?.pending_transcription_chunks ?? 0;

  return (
    <div className="flex-1 flex flex-col h-full bg-[#0a0a0c] text-zinc-100 overflow-hidden select-none">
      {/* Top Header */}
      <div className="border-b border-white/5 px-6 py-4 flex items-center justify-between bg-zinc-950/40 backdrop-blur-md">
        <div className="flex items-center gap-3">
          <div className="w-9 h-9 rounded-md bg-white/5 border border-white/10 flex items-center justify-center">
            <Mic className="w-4.5 h-4.5 text-zinc-400" />
          </div>
          <div>
            <h1 className="text-base font-semibold tracking-tight text-white flex items-center gap-2">
              Meetings
              <span className="text-[10px] uppercase font-bold tracking-wider px-2 py-0.5 rounded-full bg-white/5 text-zinc-400 border border-white/10">
                V2 Crash-Resilient
              </span>
            </h1>
            <p className="text-xs text-zinc-400">
              30-second incremental persistence &amp; dual microphone + system audio capture.
            </p>
          </div>
        </div>

        {/* Action Buttons */}
        <div className="flex items-center gap-3">
          {activeSession ? (
            <div className="flex items-center gap-2">
              <span className="font-mono text-xs text-zinc-300 tabular-nums px-2">
                {formatDuration(activeElapsedSec)}
              </span>
              <button
                onClick={handleTogglePause}
                disabled={isTogglingPause || isFinalizing}
                className={`flex items-center gap-2 px-3 py-2 rounded-md font-medium text-xs transition-colors border disabled:opacity-50 disabled:cursor-not-allowed ${
                  isPaused
                    ? 'bg-zinc-100 hover:bg-white text-zinc-900 border-transparent'
                    : 'bg-zinc-800 hover:bg-zinc-700 text-zinc-100 border-white/10'
                }`}
              >
                {isPaused ? (
                  <>
                    <Play className="w-3.5 h-3.5 fill-current" />
                    <span>Resume</span>
                  </>
                ) : (
                  <>
                    <Pause className="w-3.5 h-3.5 fill-current" />
                    <span>Pause</span>
                  </>
                )}
              </button>
              <button
                onClick={handleStopRecording}
                disabled={isStopping || isFinalizing}
                className="flex items-center gap-2 px-4 py-2 rounded-md bg-red-500/90 hover:bg-red-500 text-white font-medium text-xs transition-colors border border-red-400/30 disabled:opacity-50"
              >
                {isStopping || isFinalizing ? (
                  <>
                    <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                    <span>Finalizing Audio...</span>
                  </>
                ) : (
                  <>
                    <Square className="w-3.5 h-3.5 fill-current" />
                    <span>Stop Meeting Recording</span>
                  </>
                )}
              </button>
            </div>
          ) : (
            <div className="flex items-center gap-2">
              <input
                type="text"
                value={meetingTitleInput}
                onChange={(e) => setMeetingTitleInput(e.target.value)}
                placeholder="Meeting Title (optional)..."
                className="px-3 py-1.5 rounded-lg bg-zinc-900 border border-white/10 text-xs text-zinc-200 placeholder:text-zinc-500 focus:outline-none focus:border-white/25 w-56"
              />
              <button
                onClick={handleStartRecording}
                disabled={isStarting}
                className="flex items-center gap-2 px-4 py-2 rounded-md bg-zinc-100 hover:bg-white text-zinc-900 font-medium text-xs transition-colors border border-transparent"
              >
                {isStarting ? (
                  <>
                    <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                    <span>Starting...</span>
                  </>
                ) : (
                  <>
                    <Play className="w-3.5 h-3.5 fill-current" />
                    <span>Start Recording</span>
                  </>
                )}
              </button>
            </div>
          )}
        </div>
      </div>

      {activeSession?.capture_warning && (
        <div className="px-6 py-2 bg-amber-500/10 border-b border-amber-500/20 text-[11px] text-amber-300 flex items-center gap-2">
          <AlertTriangle className="w-3.5 h-3.5 shrink-0" />
          <span>{activeSession.capture_warning}</span>
        </div>
      )}

      {/* Main Content Split */}
      <div className="flex-1 flex overflow-hidden">
        {/* Left: Meeting Sessions List */}
        <div className="w-80 border-r border-white/5 flex flex-col bg-zinc-950/20">
          <div className="p-3 border-b border-white/5 flex items-center justify-between text-xs text-zinc-400">
            <span className="font-semibold text-zinc-300">
              Recorded Sessions ({sessions.length})
            </span>
            <button
              onClick={loadSessions}
              className="p-1 hover:bg-white/5 rounded transition-colors text-zinc-400 hover:text-zinc-200"
              title="Refresh list"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${isLoading ? 'animate-spin' : ''}`} />
            </button>
          </div>

          <div className="flex-1 overflow-y-auto p-2 space-y-1">
            {sessions.length === 0 && !activeSession ? (
              <div className="h-full flex flex-col items-center justify-center p-6 text-center text-zinc-500">
                <FileText className="w-8 h-8 mb-2 opacity-30" />
                <p className="text-xs">No meetings recorded yet.</p>
                <p className="text-[11px] text-zinc-600 mt-1">
                  Start a recording above to capture live audio &amp; incremental transcripts.
                </p>
              </div>
            ) : (
              sessions.map((item) => {
                const isSelected = selectedSessionId === item.id;
                const isItemActive = activeSession?.id === item.id;
                const itemState = isItemActive ? activeSession!.state : item.state;
                const derived = processingIndex[item.id];
                // Prefer the extracted title for display; the recorder's own
                // title is never overwritten.
                const displayTitle = derived?.title?.trim() || item.title;

                return (
                  <div
                    key={item.id}
                    onClick={() => handleSelectSession(item.id)}
                    title={displayTitle}
                    className={`group relative w-full text-left p-3 rounded-md border transition-colors flex flex-col gap-1.5 cursor-pointer ${
                      isSelected
                        ? 'bg-white/[0.07] border-white/20 text-white'
                        : 'bg-zinc-900/40 hover:bg-zinc-900/80 border-white/5 text-zinc-300'
                    }`}
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span
                        className="font-medium text-xs truncate text-zinc-100 flex-1 min-w-0"
                        title={displayTitle}
                      >
                        {displayTitle}
                      </span>
                      <span
                        className={`text-[9px] font-bold uppercase tracking-wider px-1.5 py-0.5 rounded shrink-0 ${
                          isItemActive && itemState === 'PAUSED'
                            ? 'bg-amber-500/20 text-amber-400 border border-amber-500/30'
                            : isItemActive
                            ? 'bg-red-500/20 text-red-400 border border-red-500/30'
                            : itemState === 'RECOVERED'
                            ? 'bg-amber-500/20 text-amber-400 border border-amber-500/30'
                            : itemState === 'INTERRUPTED' || itemState === 'ERROR'
                            ? 'bg-zinc-800 text-zinc-400'
                            : 'bg-lime-500/10 text-lime-400 border border-lime-500/20'
                        }`}
                      >
                        {isItemActive && itemState === 'RECORDING' ? 'Recording' : itemState}
                      </span>
                    </div>

                    <div className="flex items-center justify-between text-[11px] text-zinc-400 gap-2">
                      <div className="flex items-center gap-2.5 flex-wrap min-w-0">
                        <span className="flex items-center gap-1">
                          <Clock className="w-3 h-3 text-zinc-500" />
                          {formatDuration(isItemActive ? activeElapsedSec : item.duration_seconds)}
                        </span>
                        <span className="flex items-center gap-1">
                          <Layers className="w-3 h-3 text-zinc-500" />
                          {item.chunk_count} chunk{item.chunk_count === 1 ? '' : 's'}
                        </span>
                        <span className="flex items-center gap-1">
                          <FileText className="w-3 h-3 text-zinc-500" />
                          {isItemActive ? activeSessionWords : (item.word_count ?? 0)} word{(isItemActive ? activeSessionWords : (item.word_count ?? 0)) === 1 ? '' : 's'}
                        </span>
                        {derived?.meeting_type && (
                          <span className="text-[10px] px-1.5 rounded bg-white/5 text-zinc-400 border border-white/10">
                            {derived.meeting_type}
                          </span>
                        )}
                        {derived?.open_action_item_count ? (
                          <span
                            title={`${derived.open_action_item_count} open of ${derived.action_item_count} action items`}
                            className="flex items-center gap-1 text-amber-400"
                          >
                            <ListTodo className="w-3 h-3" />
                            {derived.open_action_item_count}
                          </span>
                        ) : null}
                        {(derived?.has_summary || item.summary) && (
                          <span
                            title={
                              derived?.has_summary
                                ? 'Summary available'
                                : 'Summarized before the processing pipeline existed'
                            }
                            className="flex items-center text-zinc-400"
                          >
                            <Sparkles className="w-3 h-3" />
                          </span>
                        )}
                      </div>

                      {!isItemActive && (
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            setPendingDeleteId(item.id);
                          }}
                          disabled={deletingId === item.id}
                          className="opacity-0 group-hover:opacity-100 p-1 hover:bg-red-500/20 hover:text-red-400 text-zinc-500 rounded transition-all shrink-0 cursor-pointer"
                          title="Move to trash"
                        >
                          <Trash2 className="w-3 h-3" />
                        </button>
                      )}
                    </div>
                  </div>
                );
              })
            )}
          </div>
        </div>

        {/* Right: Selected Session Transcript & Details */}
        <div className="flex-1 flex flex-col bg-zinc-900/20 overflow-hidden">
          {selectedSession ? (
            <div className="flex-1 flex flex-col h-full overflow-hidden">
              {/* Session Overview Header */}
              <div className="p-6 border-b border-white/5 bg-zinc-950/30 flex items-start justify-between">
                <div>
                  <h2 className="text-lg font-semibold text-white">
                    {meetingTitle(selectedSession, processing)}
                  </h2>
                  <p className="text-xs text-zinc-400 mt-1">
                    Recorded on {new Date(selectedSession.created_at).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' })} • Duration:{' '}
                    {formatDuration(
                      isSelectedActive ? activeElapsedSec : selectedSession.duration_seconds
                    )}{' '}
                    • Chunks: {selectedSession.chunk_count} • Words: {selectedSessionWords}
                    {selectedSession.paused_seconds > 1
                      ? ` • Paused: ${formatDuration(selectedSession.paused_seconds)}`
                      : ''}
                  </p>
                </div>

                <div className="flex items-center gap-2">
                  {!isSelectedActive && (
                    <>
                      <button
                        onClick={() => handleGenerateSummary(selectedSession.id)}
                        disabled={isSummarizing || (selectedSessionWords === 0 && transcriptSegments.length === 0)}
                        className="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-white/5 hover:bg-white/10 text-zinc-200 border border-white/10 text-xs font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer"
                        title="Read the transcript, extract what was decided and who owns what, then write it up"
                      >
                        {isSummarizing ? (
                          <>
                            <RefreshCw className="w-3.5 h-3.5 animate-spin text-zinc-300" />
                            <span>Generating…</span>
                          </>
                        ) : (
                          <>
                            <Sparkles className="w-3.5 h-3.5 text-zinc-300" />
                            <span>{processing?.summary ? 'Regenerate' : 'Generate Summary'}</span>
                          </>
                        )}
                      </button>

                      <button
                        onClick={() => handlePromoteToScribble(selectedSession.id)}
                        disabled={isPromoting || !processing?.facts}
                        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-white/5 hover:bg-white/10 text-zinc-300 border border-white/10 text-xs font-medium transition-all disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer"
                        title="Create a Scribble from this meeting, keeping the meeting as its source"
                      >
                        {isPromoting ? (
                          <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                        ) : (
                          <NotebookPen className="w-3.5 h-3.5" />
                        )}
                        <span>
                          {processing?.scribble_ref ? 'Scribble again' : 'To Scribble'}
                        </span>
                      </button>

                      <button
                        onClick={() => setPendingDeleteId(selectedSession.id)}
                        disabled={deletingId === selectedSession.id || isSummarizing}
                        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-red-500/10 hover:bg-red-500/20 text-red-400 border border-red-500/20 hover:border-red-500/30 text-xs font-medium transition-all cursor-pointer"
                        title="Move this meeting to Trash"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                        <span>Delete Meeting</span>
                      </button>
                    </>
                  )}
                </div>
              </div>

              {promotedScribbleTitle && (
                <div className="mx-6 mt-3 px-3 py-2 rounded-lg bg-emerald-500/10 border border-emerald-500/25 text-[11px] text-emerald-300 flex items-center gap-2">
                  <NotebookPen className="w-3.5 h-3.5 shrink-0" />
                  <span className="flex-1 min-w-0 truncate">
                    Saved “{promotedScribbleTitle}” to Scribbles, with this meeting as
                    its source.
                  </span>
                  <button
                    onClick={() => setPromotedScribbleTitle(null)}
                    className="text-lime-500/80 hover:text-lime-300 font-medium cursor-pointer shrink-0"
                  >
                    Dismiss
                  </button>
                </div>
              )}

              <MeetingProcessingStatus
                processing={processing}
                isBusy={isSummarizing}
                onRetry={() =>
                  handleGenerateSummary(selectedSession.id, undefined, undefined, true)
                }
              />

              {/* Tab Navigation — Summary first, raw transcript last. */}
              <div className="flex items-center gap-1 px-6 border-b border-white/5 bg-zinc-950/20">
                {TABS.filter(
                  (tab) => tab.key !== 'raw' || meetingSettings.showRawTranscript,
                ).map((tab) => {
                  const Icon = tab.icon;
                  const isActive = activeMeetingTab === tab.key;
                  const count =
                    tab.key === 'raw'
                      ? transcriptSegments.length
                      : tab.key === 'conversation'
                        ? processing?.conversation?.turns.length ?? 0
                        : 0;

                  return (
                    <button
                      key={tab.key}
                      onClick={() => setActiveMeetingTab(tab.key)}
                      className={`flex items-center gap-2 px-4 py-2.5 text-xs font-semibold border-b-2 transition-all cursor-pointer ${
                        isActive
                          ? 'border-zinc-200 text-zinc-100'
                          : 'border-transparent text-zinc-400 hover:text-zinc-200 hover:bg-white/5'
                      }`}
                    >
                      <Icon className="w-3.5 h-3.5" />
                      <span>{tab.label}</span>
                      {tab.key === 'summary' && processing?.summary && (
                        <span className="w-1.5 h-1.5 rounded-full bg-lime-400" />
                      )}
                      {tab.key !== 'summary' && count > 0 && (
                        <span className="text-[10px] font-mono px-1.5 rounded bg-white/5 text-zinc-400">
                          {count}
                        </span>
                      )}
                    </button>
                  );
                })}
              </div>

              {activeMeetingTab === 'summary' && (
                <MeetingSummaryTab
                  processing={processing}
                  extensions={extensions}
                  related={related}
                  isGenerating={isSummarizing}
                  canGenerate={!isSelectedActive}
                  onGenerate={(mode, extensionId) =>
                    handleGenerateSummary(selectedSession.id, mode, extensionId)
                  }
                  onToggleActionItem={(item) =>
                    handleToggleActionItem(selectedSession.id, item)
                  }
                  onAddTask={(item) => handleAddTasks(selectedSession.id, item)}
                  onAddAllTasks={() => handleAddTasks(selectedSession.id)}
                  busyActionItemId={busyActionItemId}
                  isAddingAllTasks={isAddingAllTasks}
                  onSelectRelated={handleSelectSession}
                />
              )}

              {activeMeetingTab === 'notes' && (
                <MeetingNotesTab
                  notes={notes}
                  isLoaded={notes !== null}
                  onSave={(next) => handleSaveNotes(selectedSession.id, next)}
                />
              )}

              {activeMeetingTab === 'conversation' && (
                <MeetingConversationTab
                  conversation={processing?.conversation}
                  speakers={processing?.speakers ?? []}
                  isRenaming={isRenamingSpeaker}
                  isDisabled={!meetingSettings.generateConversationTranscript}
                  onRenameSpeaker={(speakerId, displayName) =>
                    handleRenameSpeaker(selectedSession.id, speakerId, displayName)
                  }
                />
              )}

              {activeMeetingTab === 'raw' &&
                (meetingSettings.showRawTranscript ? (
                  <MeetingRawTranscriptTab
                    segments={transcriptSegments}
                    liveUpdates={liveUpdates}
                    isRecording={isSelectedActive}
                    isPaused={isPaused}
                    backlog={backlog}
                    latestLatency={latestLatency}
                  />
                ) : (
                  <RawTranscriptHidden />
                ))}
            </div>
          ) : (
            <div className="flex-1 flex flex-col items-center justify-center p-8 text-center text-zinc-500">
              <FileText className="w-10 h-10 mb-3 opacity-20" />
              <p className="text-sm font-medium text-zinc-400">No Meeting Selected</p>
              <p className="text-xs text-zinc-500 mt-1 max-w-sm">
                Select a meeting from the list or start a new recording to capture audio &amp;
                incremental transcripts.
              </p>
            </div>
          )}
        </div>
      </div>

      <ConfirmationModal
        isOpen={!!pendingDeleteId}
        title="Move meeting to Trash?"
        description={`"${
          pendingDeleteSession?.title ?? 'This meeting'
        }" will be moved to Trash. You can restore it from Settings within 30 days.`}
        confirmLabel="Move to Trash"
        variant="destructive"
        isBusy={!!deletingId}
        onConfirm={() => pendingDeleteId && handleDeleteSession(pendingDeleteId)}
        onCancel={() => setPendingDeleteId(null)}
      />
    </div>
  );
};
