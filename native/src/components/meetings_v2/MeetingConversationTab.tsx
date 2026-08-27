import React, { useState } from 'react';
import { Check, Info, Pencil, Users, X } from 'lucide-react';
import type { Conversation, Speaker } from '../../types';
import { formatTimestamp, speakerLabel } from './meetingProcessing';

interface MeetingConversationTabProps {
  conversation?: Conversation | null;
  speakers: Speaker[];
  onRenameSpeaker: (speakerId: string, displayName: string | null) => void;
  isRenaming: boolean;
  /** True when the conversation transcript is switched off in settings. */
  isDisabled: boolean;
}

const SpeakerRow: React.FC<{
  speaker: Speaker;
  onRename: (displayName: string | null) => void;
  isRenaming: boolean;
}> = ({ speaker, onRename, isRenaming }) => {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(speaker.display_name ?? '');

  const commit = () => {
    setEditing(false);
    const trimmed = draft.trim();
    // An empty name clears the override and restores "Speaker N" rather than
    // storing a blank name.
    onRename(trimmed.length > 0 ? trimmed : null);
  };

  if (editing) {
    return (
      <div className="flex items-center gap-1.5">
        <input
          autoFocus
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') commit();
            if (e.key === 'Escape') setEditing(false);
          }}
          placeholder={speaker.fallback_label}
          className="w-32 px-2 py-1 rounded-md bg-zinc-900 border border-indigo-500/40 text-xs text-zinc-100 outline-none"
        />
        <button
          onClick={commit}
          disabled={isRenaming}
          className="p-1 rounded-md bg-emerald-500/10 hover:bg-emerald-500/20 text-emerald-400 cursor-pointer"
          aria-label={`Save name for ${speaker.fallback_label}`}
        >
          <Check className="w-3 h-3" />
        </button>
        <button
          onClick={() => setEditing(false)}
          className="p-1 rounded-md bg-white/5 hover:bg-white/10 text-zinc-400 cursor-pointer"
          aria-label="Cancel rename"
        >
          <X className="w-3 h-3" />
        </button>
      </div>
    );
  }

  return (
    <button
      onClick={() => {
        setDraft(speaker.display_name ?? '');
        setEditing(true);
      }}
      className="group flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-white/5 hover:bg-white/10 border border-white/10 text-xs text-zinc-200 transition-colors cursor-pointer"
      title={`Rename ${speaker.fallback_label} (id: ${speaker.id})`}
    >
      <span>
        {speaker.display_name?.trim() || speaker.fallback_label}
      </span>
      {speaker.is_local_user && (
        <span className="text-[9px] font-mono text-zinc-500">you</span>
      )}
      <Pencil className="w-3 h-3 text-zinc-500 opacity-0 group-hover:opacity-100 transition-opacity" />
    </button>
  );
};

/**
 * The readable transcript: chronological, speaker-labelled, sentence grouped.
 *
 * Turns store speaker *ids*; names are resolved here, which is why renaming a
 * speaker updates this view immediately without regenerating anything and
 * without touching the raw transcript.
 */
export const MeetingConversationTab: React.FC<MeetingConversationTabProps> = ({
  conversation,
  speakers,
  onRenameSpeaker,
  isRenaming,
  isDisabled,
}) => {
  if (isDisabled) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-2 p-8 text-center">
        <Users className="w-8 h-8 text-zinc-700" />
        <p className="text-sm font-medium text-zinc-400">Conversation transcript is off</p>
        <p className="text-xs text-zinc-500 max-w-sm">
          Turn it on in Settings › Meetings. The raw transcript is unaffected either
          way.
        </p>
      </div>
    );
  }

  const turns = conversation?.turns ?? [];

  if (turns.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-2 p-8 text-center">
        <Users className="w-8 h-8 text-zinc-700" />
        <p className="text-sm font-medium text-zinc-400">No conversation yet</p>
        <p className="text-xs text-zinc-500 max-w-sm">
          This appears once the recording is finished and the transcript has been
          processed.
        </p>
      </div>
    );
  }

  const unattributed = conversation?.unattributed_turn_count ?? 0;

  return (
    <div className="flex-1 overflow-y-auto p-6 space-y-4">
      {speakers.length > 0 && (
        <div className="flex items-center gap-2 flex-wrap pb-3 border-b border-white/5">
          <span className="text-xs font-semibold text-zinc-400 flex items-center gap-1.5">
            <Users className="w-3.5 h-3.5" />
            Speakers
          </span>
          {speakers.map((speaker) => (
            <SpeakerRow
              key={speaker.id}
              speaker={speaker}
              isRenaming={isRenaming}
              onRename={(name) => onRenameSpeaker(speaker.id, name)}
            />
          ))}
        </div>
      )}

      {unattributed > 0 && (
        <p className="flex items-start gap-2 text-[11px] text-zinc-400 px-3 py-2 rounded-lg bg-white/5 border border-white/10">
          <Info className="w-3.5 h-3.5 shrink-0 mt-0.5" />
          <span>
            {unattributed} {unattributed === 1 ? 'stretch' : 'stretches'} could not be
            attributed. Speakers are told apart by capture channel — your microphone
            versus everyone else — so when both were audible at once, Relay leaves it
            unattributed rather than guessing.
          </span>
        </p>
      )}

      <div className="space-y-3">
        {turns.map((turn) => {
          const label = speakerLabel(speakers, turn.speaker_id);
          const isMe = speakers.find((s) => s.id === turn.speaker_id)?.is_local_user;

          return (
            <div key={turn.id} className="flex flex-col gap-1">
              <div className="flex items-center gap-2">
                <span
                  className={`text-xs font-semibold ${
                    isMe ? 'text-indigo-300' : turn.speaker_id ? 'text-zinc-200' : 'text-zinc-500'
                  }`}
                >
                  {label}
                </span>
                <span className="text-[10px] font-mono text-zinc-600">
                  {formatTimestamp(turn.start_time_s)}
                </span>
              </div>
              <p className="text-sm text-zinc-300 leading-relaxed font-sans select-text pl-0.5">
                {turn.text}
              </p>
            </div>
          );
        })}
      </div>
    </div>
  );
};
