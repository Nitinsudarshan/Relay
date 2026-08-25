import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  Mic,
  MicOff,
  Volume2,
  VolumeX,
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
} from 'lucide-react';
import { ConfirmationModal } from '../common/ConfirmationModal';
import { MeetingSession, TranscriptSegment, LiveTranscriptUpdate } from '../../types';

/** States in which a session still owns the recorder. */
const ACTIVE_STATES = ['STARTING', 'RECORDING', 'PAUSED', 'STOPPING', 'FINALIZING'];

/** See the recording pill: events alone cannot keep a long-lived view honest. */
const RECONCILE_INTERVAL_MS = 1000;
const TIMER_TICK_MS = 250;

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
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);
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

  /** Honest source labels: "captured" must not be shown for a source that never was. */
  const sourceLabel = (active: boolean, heard: boolean, live: boolean) => {
    if (heard) return live ? 'Live' : 'Captured';
    if (active) return live ? 'Silent' : 'No audio';
    return 'Unavailable';
  };

  const pendingDeleteSession = sessions.find((s) => s.id === pendingDeleteId);
  const finalisedUpdates = liveUpdates.filter((u) => u.is_final);
  const pendingUpdate = liveUpdates.find((u) => !u.is_final);
  const latestLatency = liveUpdates.length
    ? liveUpdates[liveUpdates.length - 1].latency_ms
    : undefined;
  const backlog = activeSession?.pending_transcription_chunks ?? 0;

  return (
    <div className="flex-1 flex flex-col h-full bg-[#0a0a0c] text-zinc-100 overflow-hidden select-none">
      {/* Top Header */}
      <div className="border-b border-white/5 px-6 py-4 flex items-center justify-between bg-zinc-950/40 backdrop-blur-md">
        <div className="flex items-center gap-3">
          <div className="w-9 h-9 rounded-xl bg-gradient-to-tr from-indigo-500/20 to-violet-500/20 border border-indigo-500/30 flex items-center justify-center shadow-inner">
            <Mic className="w-5 h-5 text-indigo-400" />
          </div>
          <div>
            <h1 className="text-base font-semibold tracking-tight text-white flex items-center gap-2">
              Meetings
              <span className="text-[10px] uppercase font-bold tracking-wider px-2 py-0.5 rounded-full bg-indigo-500/10 text-indigo-400 border border-indigo-500/20">
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
                className={`flex items-center gap-2 px-3 py-2 rounded-xl font-medium text-xs shadow-lg active:scale-95 transition-all border disabled:opacity-50 disabled:cursor-not-allowed ${
                  isPaused
                    ? 'bg-emerald-600 hover:bg-emerald-500 text-white border-emerald-400/30'
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
                className="flex items-center gap-2 px-4 py-2 rounded-xl bg-red-500/90 hover:bg-red-500 text-white font-medium text-xs shadow-lg shadow-red-500/20 active:scale-95 transition-all border border-red-400/30 disabled:opacity-70"
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
                className="px-3 py-1.5 rounded-lg bg-zinc-900 border border-white/10 text-xs text-zinc-200 placeholder:text-zinc-500 focus:outline-none focus:border-indigo-500/50 w-56"
              />
              <button
                onClick={handleStartRecording}
                disabled={isStarting}
                className="flex items-center gap-2 px-4 py-2 rounded-xl bg-indigo-600 hover:bg-indigo-500 text-white font-medium text-xs shadow-lg shadow-indigo-600/20 active:scale-95 transition-all border border-indigo-400/30"
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

                return (
                  <div
                    key={item.id}
                    onClick={() => setSelectedSessionId(item.id)}
                    className={`group relative w-full text-left p-3 rounded-xl border transition-all flex flex-col gap-1.5 cursor-pointer ${
                      isSelected
                        ? 'bg-indigo-600/10 border-indigo-500/30 text-white'
                        : 'bg-zinc-900/40 hover:bg-zinc-900/80 border-white/5 text-zinc-300'
                    }`}
                  >
                    <div className="flex items-center justify-between">
                      <span className="font-medium text-xs truncate max-w-[150px] text-zinc-100">
                        {item.title}
                      </span>
                      <div className="flex items-center gap-1.5">
                        <span
                          className={`text-[9px] font-bold uppercase tracking-wider px-1.5 py-0.5 rounded ${
                            isItemActive && itemState === 'PAUSED'
                              ? 'bg-amber-500/20 text-amber-400 border border-amber-500/30'
                              : isItemActive
                              ? 'bg-red-500/20 text-red-400 border border-red-500/30'
                              : itemState === 'RECOVERED'
                              ? 'bg-amber-500/20 text-amber-400 border border-amber-500/30'
                              : itemState === 'INTERRUPTED' || itemState === 'ERROR'
                              ? 'bg-zinc-800 text-zinc-400'
                              : 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20'
                          }`}
                        >
                          {isItemActive && itemState === 'RECORDING' ? 'Recording' : itemState}
                        </span>

                        {!isItemActive && (
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              setPendingDeleteId(item.id);
                            }}
                            disabled={deletingId === item.id}
                            className="opacity-0 group-hover:opacity-100 p-1 hover:bg-red-500/20 hover:text-red-400 text-zinc-500 rounded transition-all"
                            title="Delete meeting"
                          >
                            <Trash2 className="w-3 h-3" />
                          </button>
                        )}
                      </div>
                    </div>

                    <div className="flex items-center gap-3 text-[11px] text-zinc-400">
                      <span className="flex items-center gap-1">
                        <Clock className="w-3 h-3 text-zinc-500" />
                        {formatDuration(isItemActive ? activeElapsedSec : item.duration_seconds)}
                      </span>
                      <span className="flex items-center gap-1">
                        <Layers className="w-3 h-3 text-zinc-500" />
                        {item.chunk_count} chunk{item.chunk_count === 1 ? '' : 's'}
                      </span>
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
                  <h2 className="text-lg font-semibold text-white">{selectedSession.title}</h2>
                  <p className="text-xs text-zinc-400 mt-1">
                    Recorded on {new Date(selectedSession.created_at).toLocaleString()} • Duration:{' '}
                    {formatDuration(
                      isSelectedActive ? activeElapsedSec : selectedSession.duration_seconds
                    )}{' '}
                    • Chunks: {selectedSession.chunk_count}
                    {selectedSession.paused_seconds > 1
                      ? ` • Paused: ${formatDuration(selectedSession.paused_seconds)}`
                      : ''}
                  </p>
                </div>

                <div className="flex items-center gap-2">
                  <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-white/5 border border-white/5 text-xs text-zinc-300">
                    {selectedSession.mic_heard || selectedSession.mic_active ? (
                      <Mic className="w-3 h-3 text-emerald-400" />
                    ) : (
                      <MicOff className="w-3 h-3 text-zinc-500" />
                    )}
                    <span>
                      Mic:{' '}
                      {sourceLabel(
                        selectedSession.mic_active,
                        selectedSession.mic_heard,
                        isSelectedActive
                      )}
                    </span>
                  </div>
                  <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-white/5 border border-white/5 text-xs text-zinc-300">
                    {selectedSession.sys_audio_heard || selectedSession.sys_audio_active ? (
                      <Volume2 className="w-3 h-3 text-indigo-400" />
                    ) : (
                      <VolumeX className="w-3 h-3 text-zinc-500" />
                    )}
                    <span>
                      Sys:{' '}
                      {sourceLabel(
                        selectedSession.sys_audio_active,
                        selectedSession.sys_audio_heard,
                        isSelectedActive
                      )}
                    </span>
                  </div>

                  {!isSelectedActive && (
                    <button
                      onClick={() => setPendingDeleteId(selectedSession.id)}
                      disabled={deletingId === selectedSession.id}
                      className="flex items-center gap-1.5 px-3 py-1 rounded-lg bg-red-500/10 hover:bg-red-500/20 text-red-400 border border-red-500/20 hover:border-red-500/30 text-xs font-medium transition-all"
                      title="Delete this meeting and all audio chunks"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                      <span>Delete Meeting</span>
                    </button>
                  )}
                </div>
              </div>

              {/* Transcript Scroll Area */}
              <div className="flex-1 overflow-y-auto p-6 space-y-4">
                <div className="flex items-center justify-between">
                  <h3 className="text-xs font-semibold uppercase tracking-wider text-zinc-400 flex items-center gap-2">
                    <FileText className="w-3.5 h-3.5" />
                    {isSelectedActive
                      ? `Live Stream • 30s Chunks (${transcriptSegments.length})`
                      : `Final Transcript (${transcriptSegments.length} Segments)`}
                  </h3>
                  {isSelectedActive && (
                    <div className="flex items-center gap-2">
                      {backlog > 0 && (
                        <span
                          className="text-[10px] text-amber-400 font-mono px-2 py-0.5 rounded-md bg-amber-500/10 border border-amber-500/20"
                          title="Recorded chunks still waiting to be transcribed. Audio is already saved."
                        >
                          {backlog} chunk{backlog === 1 ? '' : 's'} queued
                        </span>
                      )}
                      <span
                        className={`text-[11px] font-medium flex items-center gap-1.5 px-2 py-0.5 rounded-md border ${
                          isPaused
                            ? 'text-amber-400 bg-amber-500/10 border-amber-500/20'
                            : 'text-emerald-400 bg-emerald-500/10 border-emerald-500/20 animate-pulse'
                        }`}
                      >
                        <Sparkles className="w-3 h-3" />
                        {isPaused ? 'Paused' : 'Live STT Stream'}
                        {latestLatency !== undefined && !isPaused
                          ? ` • ${latestLatency}ms`
                          : ''}
                      </span>
                    </div>
                  )}
                </div>

                {/* Live feed: committed utterances plus the one still forming */}
                {isSelectedActive && (finalisedUpdates.length > 0 || pendingUpdate) && (
                  <div className="p-4 rounded-xl bg-emerald-950/20 border border-emerald-500/20 flex flex-col gap-2">
                    <div className="flex items-center justify-between text-[11px] text-emerald-400 font-medium">
                      <span className="flex items-center gap-1.5">
                        <span className="w-2 h-2 rounded-full bg-emerald-400 animate-ping" />
                        Live Continuous Speech
                      </span>
                    </div>
                    <div className="text-sm text-zinc-100 leading-relaxed font-sans select-text">
                      {finalisedUpdates.map((u) => (
                        <span key={u.segment_id} className="mr-1.5">
                          {u.text}
                        </span>
                      ))}
                      {pendingUpdate && (
                        <span
                          key={pendingUpdate.segment_id}
                          className="mr-1.5 text-emerald-200/80 italic"
                          title="Still being transcribed"
                        >
                          {pendingUpdate.text}
                        </span>
                      )}
                    </div>
                  </div>
                )}

                {transcriptSegments.length === 0 && liveUpdates.length === 0 ? (
                  <div className="py-12 text-center text-zinc-500 text-xs">
                    {isSelectedActive
                      ? 'Listening for speech on Microphone and System Audio...'
                      : 'No speech was recognized in this meeting.'}
                  </div>
                ) : (
                  <div className="space-y-3">
                    {transcriptSegments.map((seg) => (
                      <div
                        key={seg.chunk_index}
                        className="p-4 rounded-xl bg-zinc-950/60 border border-white/5 flex flex-col gap-1.5"
                      >
                        <div className="flex items-center justify-between text-[11px] text-zinc-500 font-mono">
                          <span>
                            Durable Chunk #{seg.chunk_index + 1} ({Math.floor(seg.start_time_s)}s -{' '}
                            {Math.floor(seg.end_time_s)}s)
                          </span>
                          <span
                            className={`px-1.5 py-0.2 rounded text-[9px] font-bold uppercase ${
                              seg.status === 'SUCCESS'
                                ? 'text-emerald-400 bg-emerald-500/10'
                                : 'text-zinc-500 bg-zinc-800'
                            }`}
                          >
                            {seg.status}
                          </span>
                        </div>
                        <p className="text-sm text-zinc-200 leading-relaxed font-sans select-text">
                          {seg.text || (
                            <span className="italic text-zinc-600">(Silence / No Speech)</span>
                          )}
                        </p>
                      </div>
                    ))}
                  </div>
                )}
              </div>
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
        title="Delete this meeting?"
        description={`"${
          pendingDeleteSession?.title ?? 'This meeting'
        }" and all of its recorded audio and transcripts will be permanently deleted. This cannot be undone.`}
        confirmLabel="Delete Meeting"
        variant="destructive"
        isBusy={!!deletingId}
        onConfirm={() => pendingDeleteId && handleDeleteSession(pendingDeleteId)}
        onCancel={() => setPendingDeleteId(null)}
      />
    </div>
  );
};
