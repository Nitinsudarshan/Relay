import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  Mic,
  Volume2,
  Square,
  Play,
  Clock,
  FileText,
  RefreshCw,
  Layers,
  Sparkles,
  Trash2,
} from 'lucide-react';
import { MeetingSession, TranscriptSegment, LiveTranscriptUpdate } from '../../types';

export const MeetingsV2View: React.FC = () => {
  const [sessions, setSessions] = useState<MeetingSession[]>([]);
  const [activeSession, setActiveSession] = useState<MeetingSession | null>(null);
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [transcriptSegments, setTranscriptSegments] = useState<TranscriptSegment[]>([]);
  const [liveUpdates, setLiveUpdates] = useState<LiveTranscriptUpdate[]>([]);
  const [isLoading, setIsLoading] = useState<boolean>(true);
  const [isStarting, setIsStarting] = useState<boolean>(false);
  const [isStopping, setIsStopping] = useState<boolean>(false);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [meetingTitleInput, setMeetingTitleInput] = useState<string>('');
  const [activeElapsedSec, setActiveElapsedSec] = useState<number>(0);

  useEffect(() => {
    if (!activeSession) return;
    const startMs = activeSession.started_at
      ? new Date(activeSession.started_at).getTime()
      : Date.now() - (activeSession.duration_seconds || 0) * 1000;

    const update = () => {
      const diffSec = Math.max(0, Math.floor((Date.now() - startMs) / 1000));
      setActiveElapsedSec(diffSec);
    };
    update();
    const timer = setInterval(update, 500);
    return () => clearInterval(timer);
  }, [activeSession?.id, activeSession?.started_at, activeSession?.state]);

  const loadSessions = async () => {
    try {
      setIsLoading(true);
      const [listRes, activeRes] = await Promise.all([
        invoke<MeetingSession[]>('list_meetings_v2'),
        invoke<MeetingSession | null>('get_active_meeting_v2'),
      ]);
      setSessions(listRes);
      setActiveSession(activeRes);
      if (activeRes) {
        setSelectedSessionId(activeRes.id);
      } else if (listRes.length > 0 && !selectedSessionId) {
        setSelectedSessionId(listRes[0].id);
      }
    } catch (err) {
      console.error('Failed to load meetings list:', err);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadSessions();

    const unlistenState = listen<MeetingSession>('meeting-session-state-changed', (event) => {
      const updated = event.payload;
      if (
        updated.state === 'RECORDING' ||
        updated.state === 'STARTING' ||
        updated.state === 'STOPPING' ||
        updated.state === 'FINALIZING'
      ) {
        setActiveSession(updated);
        setSelectedSessionId(updated.id);
      } else {
        setActiveSession(null);
        setLiveUpdates([]);
      }
      loadSessions();
    });

    const unlistenSegment = listen<TranscriptSegment>('meeting-transcript-segment', (event) => {
      setTranscriptSegments((prev) => {
        if (prev.some((s) => s.chunk_index === event.payload.chunk_index)) {
          return prev.map((s) =>
            s.chunk_index === event.payload.chunk_index ? event.payload : s
          );
        }
        return [...prev, event.payload];
      });
    });

    const unlistenLive = listen<LiveTranscriptUpdate>('meeting-live-transcript', (event) => {
      setLiveUpdates((prev) => [...prev, event.payload]);
    });

    return () => {
      unlistenState.then((f) => f());
      unlistenSegment.then((f) => f());
      unlistenLive.then((f) => f());
    };
  }, []);

  // Fetch transcript when selected session changes
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
      setActiveSession(newSession);
      setSelectedSessionId(newSession.id);
      setMeetingTitleInput('');
      loadSessions();
    } catch (err) {
      console.error('Failed to start meeting recording:', err);
    } finally {
      setIsStarting(false);
    }
  };

  const handleStopRecording = async () => {
    if (isStopping) return;
    setIsStopping(true);
    try {
      const completed = await invoke<MeetingSession>('stop_meeting_v2');
      setActiveSession(null);
      setSelectedSessionId(completed.id);
      loadSessions();
    } catch (err) {
      console.error('Failed to stop meeting recording:', err);
    } finally {
      setIsStopping(false);
    }
  };

  const handleDeleteSession = async (sessionId: string, e?: React.MouseEvent) => {
    if (e) {
      e.stopPropagation();
    }
    if (deletingId) return;
    if (activeSession?.id === sessionId) {
      alert('Cannot delete an active recording. Please stop recording first.');
      return;
    }
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
    }
  };

  const selectedSession = sessions.find((s) => s.id === selectedSessionId) || activeSession;

  const formatDuration = (secs: number) => {
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return `${m}m ${s}s`;
  };

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

        {/* Action Button & Diagnostics */}
        <div className="flex items-center gap-3">
          {activeSession ? (
            <button
              onClick={handleStopRecording}
              disabled={isStopping}
              className="flex items-center gap-2 px-4 py-2 rounded-xl bg-red-500/90 hover:bg-red-500 text-white font-medium text-xs shadow-lg shadow-red-500/20 active:scale-95 transition-all border border-red-400/30"
            >
              {isStopping ? (
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

      {/* Main Content Split */}
      <div className="flex-1 flex overflow-hidden">
        {/* Left: Meeting Sessions List */}
        <div className="w-80 border-r border-white/5 flex flex-col bg-zinc-950/20">
          <div className="p-3 border-b border-white/5 flex items-center justify-between text-xs text-zinc-400">
            <span className="font-semibold text-zinc-300">Recorded Sessions ({sessions.length})</span>
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
                            isItemActive
                              ? 'bg-red-500/20 text-red-400 border border-red-500/30'
                              : item.state === 'RECOVERED'
                              ? 'bg-amber-500/20 text-amber-400 border border-amber-500/30'
                              : item.state === 'INTERRUPTED'
                              ? 'bg-zinc-800 text-zinc-400'
                              : 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20'
                          }`}
                        >
                          {isItemActive ? 'Recording' : item.state}
                        </span>

                        {!isItemActive && (
                          <button
                            onClick={(e) => handleDeleteSession(item.id, e)}
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
                      activeSession?.id === selectedSession.id
                        ? activeElapsedSec
                        : selectedSession.duration_seconds
                    )}{' '}
                    • Chunks: {selectedSession.chunk_count}
                  </p>
                </div>

                <div className="flex items-center gap-2">
                  <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-white/5 border border-white/5 text-xs text-zinc-300">
                    <Mic className="w-3 h-3 text-emerald-400" />
                    <span>Mic: {selectedSession.mic_active ? 'Active' : 'Captured'}</span>
                  </div>
                  <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-white/5 border border-white/5 text-xs text-zinc-300">
                    <Volume2 className="w-3 h-3 text-indigo-400" />
                    <span>Sys: {selectedSession.sys_audio_active ? 'Active' : 'Captured'}</span>
                  </div>

                  {activeSession?.id !== selectedSession.id && (
                    <button
                      onClick={() => handleDeleteSession(selectedSession.id)}
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
                    {activeSession?.id === selectedSession.id
                      ? `Live Stream (${liveUpdates.length} updates) • 30s Chunks (${transcriptSegments.length})`
                      : `Final Transcript (${transcriptSegments.length} Segments)`}
                  </h3>
                  {activeSession?.id === selectedSession.id && (
                    <span className="text-[11px] text-emerald-400 font-medium animate-pulse flex items-center gap-1.5 px-2 py-0.5 rounded-md bg-emerald-500/10 border border-emerald-500/20">
                      <Sparkles className="w-3 h-3 text-emerald-400" /> Live STT Stream (~1.5s latency)
                    </span>
                  )}
                </div>

                {/* Real-time Live STT Feed during active recording */}
                {activeSession?.id === selectedSession.id && liveUpdates.length > 0 && (
                  <div className="p-4 rounded-xl bg-emerald-950/20 border border-emerald-500/20 flex flex-col gap-2">
                    <div className="flex items-center justify-between text-[11px] text-emerald-400 font-medium">
                      <span className="flex items-center gap-1.5">
                        <span className="w-2 h-2 rounded-full bg-emerald-400 animate-ping" />
                        Live Continuous Speech
                      </span>
                      {liveUpdates.length > 0 && (
                        <span className="text-[10px] text-zinc-400 font-mono">
                          Latency: {liveUpdates[liveUpdates.length - 1].latency_ms}ms
                        </span>
                      )}
                    </div>
                    <div className="text-sm text-zinc-100 leading-relaxed font-sans select-text space-y-1.5">
                      {liveUpdates.map((u) => (
                        <span key={u.segment_id} className="inline mr-1.5">
                          {u.text}{' '}
                        </span>
                      ))}
                    </div>
                  </div>
                )}

                {transcriptSegments.length === 0 && (!activeSession || liveUpdates.length === 0) ? (
                  <div className="py-12 text-center text-zinc-500 text-xs">
                    {activeSession?.id === selectedSession.id
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
                          {seg.text || <span className="italic text-zinc-600">(Silence / No Speech)</span>}
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
                Select a meeting from the list or start a new recording to capture audio &amp; incremental transcripts.
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
