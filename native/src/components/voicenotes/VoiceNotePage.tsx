import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { HardDrive, Mic, ShieldCheck, Edit3, Trash2, GitMerge, Copy, Check, X, Save, Sparkles } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { AppSettings, VaultLocationInfo, VaultNote } from '../../types';


type VaultViewState =
  | { status: 'loading' }
  | { status: 'setup' }
  | { status: 'recovery' }
  | { status: 'ready' };

function formatNoteTimestamp(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;

  const now = new Date();
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  const time = date.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });

  if (date.toDateString() === now.toDateString()) return `Today · ${time}`;
  if (date.toDateString() === yesterday.toDateString()) return `Yesterday · ${time}`;
  return `${date.toLocaleDateString([], { month: 'short', day: 'numeric' })} · ${time}`;
}

function countWords(text: string): number {
  const trimmed = text.trim();
  return trimmed ? trimmed.split(/\s+/).length : 0;
}

const VaultSetupPrompt: React.FC<{
  recovery: boolean;
  defaultPath: string;
  busy: boolean;
  error: string;
  onChooseFolder: () => void;
  onUseDefault: () => void;
}> = ({ recovery, busy, error, onChooseFolder, onUseDefault }) => (
  <div className="flex-1 flex flex-col items-center justify-center text-center py-16 px-6 rounded-lg border border-dashed border-border bg-card">
    <HardDrive className="w-9 h-9 mb-3 text-muted-foreground opacity-60" />
    <h2 className="text-lg font-bold text-foreground mb-1.5 max-w-sm">
      {recovery ? "We can't access your Voice Note folder" : 'Where should Relay save your Voice Notes?'}
    </h2>
    <p className="text-xs text-muted-foreground max-w-sm mb-3">
      {recovery
        ? 'Choose another location to continue.'
        : 'Choose a folder where Relay should store your Voice Notes.'}
    </p>
    <p className="text-[11px] text-muted-foreground flex items-center gap-1.5 mb-5">
      <ShieldCheck className="w-3.5 h-3.5 text-emerald-500 shrink-0" />
      Your Voice Notes are stored locally on your computer.
    </p>
    {error && <p className="text-xs text-destructive mb-4 max-w-sm">{error}</p>}
    <div className="flex items-center gap-2">
      <Button onClick={onChooseFolder} disabled={busy} size="sm">
        Choose Folder
      </Button>
      {!recovery && (
        <Button onClick={onUseDefault} disabled={busy} size="sm" variant="outline">
          Use Default Relay Vault
        </Button>
      )}
    </div>
  </div>
);

export const VoiceNotePage: React.FC = () => {
  const [vaultState, setVaultState] = useState<VaultViewState>({ status: 'loading' });
  const [defaultPath, setDefaultPath] = useState('');
  const [notes, setNotes] = useState<VaultNote[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');

  // Interactive Action States
  const [editingNoteId, setEditingNoteId] = useState<string | null>(null);
  const [editingContent, setEditingContent] = useState('');
  const [deletingNoteId, setDeletingNoteId] = useState<string | null>(null);
  const [mergingNoteId, setMergingNoteId] = useState<string | null>(null);
  const [copiedNoteId, setCopiedNoteId] = useState<string | null>(null);
  const [promotedNoteIds, setPromotedNoteIds] = useState<Set<string>>(new Set());
  const [actionBusy, setActionBusy] = useState(false);
  const [settings, setSettings] = useState<AppSettings | null>(null);


  const handlePromoteToScribble = async (note: VaultNote) => {
    setActionBusy(true);
    try {
      await invoke('promote_voice_note_to_scribble', {
        voiceNoteId: note.id,
      });
      setPromotedNoteIds((prev) => new Set(prev).add(note.id));
    } catch (err) {
      console.error('Failed to promote Voice Note to Scribble:', err);
    } finally {
      setActionBusy(false);
    }
  };

  const refreshLocation = async () => {
    try {
      const [info, appSetts] = await Promise.all([
        invoke<VaultLocationInfo>('get_vault_location'),
        invoke<AppSettings>('get_settings').catch(() => null),
      ]);
      if (appSetts) setSettings(appSetts);
      setDefaultPath(info.default_path);
      if (!info.configured) {
        setVaultState({ status: 'setup' });
      } else if (!info.accessible) {
        setVaultState({ status: 'recovery' });
      } else {
        setVaultState({ status: 'ready' });
      }
    } catch (err) {
      console.error('Failed to read Vault Directory Location', err);
      setError('Could not determine where Voice Notes are stored.');
      setVaultState({ status: 'recovery' });
    }
  };

  useEffect(() => {
    refreshLocation();

    const unlistenSettings = listen<AppSettings>('settings-changed', ({ payload }) => {
      if (payload) setSettings(payload);
    });

    return () => {
      unlistenSettings.then((unlisten) => unlisten());
    };
  }, []);

  const refreshPromotedScribbles = useCallback(() => {
    invoke<{ source_metadata?: { source_voice_note_id?: string; source_voice_note_ids?: string[] } }[]>('get_scribbles')
      .then((scribbles) => {
        const ids = new Set<string>();
        for (const s of scribbles) {
          if (s.source_metadata?.source_voice_note_id) {
            ids.add(s.source_metadata.source_voice_note_id);
          }
          if (Array.isArray(s.source_metadata?.source_voice_note_ids)) {
            for (const id of s.source_metadata.source_voice_note_ids) {
              ids.add(id);
            }
          }
        }
        setPromotedNoteIds(ids);
      })
      .catch((err) => console.error('Failed to load Scribble promotion mapping', err));
  }, []);

  useEffect(() => {
    if (vaultState.status !== 'ready') return;
    invoke<VaultNote[]>('get_voice_notes')
      .then(setNotes)
      .catch((err) => console.error('Failed to load Voice Notes', err));

    refreshPromotedScribbles();
  }, [vaultState.status, refreshPromotedScribbles]);

  useEffect(() => {
    const unlistenVoice = listen<VaultNote>('voice-note-saved', ({ payload }) => {
      setNotes((prev) => [payload, ...prev.filter((n) => n.id !== payload.id)]);
      refreshPromotedScribbles();
    });
    const unlistenScribble = listen('scribble-saved', () => {
      refreshPromotedScribbles();
    });
    return () => {
      unlistenVoice.then((unlisten) => unlisten());
      unlistenScribble.then((unlisten) => unlisten());
    };
  }, [refreshPromotedScribbles]);

  const handleChooseFolder = async () => {
    setBusy(true);
    setError('');
    try {
      const picked = await invoke<string | null>('choose_vault_folder');
      if (!picked) return;
      await invoke('set_vault_location', { path: picked });
      await refreshLocation();
    } catch (err: any) {
      console.error('Failed to set Vault Directory Location', err);
      setError(err?.message || "Couldn't use that folder — choose another.");
    } finally {
      setBusy(false);
    }
  };

  const handleUseDefault = async () => {
    setBusy(true);
    setError('');
    try {
      await invoke('set_vault_location', { path: defaultPath });
      await refreshLocation();
    } catch (err: any) {
      console.error('Failed to set default Vault Directory Location', err);
      setError(err?.message || 'Could not use the default Relay Vault.');
    } finally {
      setBusy(false);
    }
  };

  // Note Action Handlers
  const handleStartEdit = (note: VaultNote) => {
    setEditingNoteId(note.id);
    setEditingContent(note.content);
    setDeletingNoteId(null);
    setMergingNoteId(null);
  };

  const handleCancelEdit = () => {
    setEditingNoteId(null);
    setEditingContent('');
  };

  const handleSaveEdit = async (id: string) => {
    if (!editingContent.trim()) return;
    setActionBusy(true);
    try {
      const updated = await invoke<VaultNote>('update_voice_note', {
        id,
        content: editingContent.trim(),
      });
      setNotes((prev) => prev.map((n) => (n.id === id ? updated : n)));
      setEditingNoteId(null);
    } catch (err) {
      console.error('Failed to update voice note', err);
    } finally {
      setActionBusy(false);
    }
  };

  const handleDelete = async (id: string) => {
    setActionBusy(true);
    try {
      await invoke('delete_voice_note', { id });
      setNotes((prev) => prev.filter((n) => n.id !== id));
      setDeletingNoteId(null);
      if (editingNoteId === id) setEditingNoteId(null);
      if (mergingNoteId === id) setMergingNoteId(null);
    } catch (err) {
      console.error('Failed to delete voice note', err);
    } finally {
      setActionBusy(false);
    }
  };

  const handleMerge = async (primaryId: string, secondaryId: string) => {
    setActionBusy(true);
    try {
      const merged = await invoke<VaultNote>('merge_voice_notes', {
        primaryId,
        secondaryId,
      });
      setNotes((prev) =>
        prev
          .filter((n) => n.id !== secondaryId)
          .map((n) => (n.id === primaryId ? merged : n))
      );
      setMergingNoteId(null);
    } catch (err) {
      console.error('Failed to merge voice notes', err);
    } finally {
      setActionBusy(false);
    }
  };

  const handleCopy = (note: VaultNote) => {
    navigator.clipboard.writeText(note.content);
    setCopiedNoteId(note.id);
    setTimeout(() => setCopiedNoteId(null), 1800);
  };

  const stats = useMemo(() => {
    const total = notes.length;
    const totalWords = notes.reduce((sum, n) => sum + countWords(n.content), 0);
    const todayKey = new Date().toDateString();
    const notesToday = notes.filter((n) => new Date(n.created_at).toDateString() === todayKey).length;
    return { total, totalWords, notesToday };
  }, [notes]);

  if (vaultState.status === 'loading') {
    return (
      <div className="flex-1 flex items-center justify-center text-xs text-muted-foreground">
        Loading Voice Notes…
      </div>
    );
  }

  if (vaultState.status === 'setup' || vaultState.status === 'recovery') {
    return (
      <VaultSetupPrompt
        recovery={vaultState.status === 'recovery'}
        defaultPath={defaultPath}
        busy={busy}
        error={error}
        onChooseFolder={handleChooseFolder}
        onUseDefault={handleUseDefault}
      />
    );
  }

  return (
    <div className="flex-1 flex flex-col gap-4 min-h-0 overflow-hidden">
      {/* Top Stats Overview */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4 shrink-0">
        <div className="rounded-lg border border-border bg-card p-4">
          <p className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground mb-1.5">
            Total Voice Notes
          </p>
          <p className="text-2xl font-extrabold text-foreground">{stats.total}</p>
        </div>
        <div className="rounded-lg border border-border bg-card p-4">
          <p className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground mb-1.5">
            Total Words
          </p>
          <p className="text-2xl font-extrabold text-foreground">{stats.totalWords.toLocaleString()}</p>
        </div>
        <div className="rounded-lg border border-border bg-card p-4">
          <p className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground mb-1.5">
            Notes Today
          </p>
          <p className="text-2xl font-extrabold text-foreground">{stats.notesToday}</p>
        </div>
      </div>

      {/* Main Transcript History Container */}
      <div className="flex-1 flex flex-col min-h-0 rounded-lg border border-border bg-card p-5">
        <div className="flex items-center justify-between mb-4 shrink-0">
          <h2 className="text-sm font-bold text-foreground">Transcript History</h2>
          <Badge variant="outline" className="text-[10px] font-mono">
            {notes.length} voice note{notes.length === 1 ? '' : 's'}
          </Badge>
        </div>

        {notes.length === 0 ? (
          <div className="flex-1 flex flex-col items-center justify-center text-center py-10 border border-dashed border-border rounded-lg text-muted-foreground">
            <Mic className="w-8 h-8 mb-2 opacity-40" />
            <p className="text-sm font-semibold">No Voice Notes yet</p>
            <p className="text-xs mt-1">Everything you dictate with Relay will show up here.</p>
          </div>
        ) : (
          <div className="flex-1 overflow-y-auto space-y-3 pr-1">
            {notes.map((note, index) => {
              const isEditing = editingNoteId === note.id;
              const isDeleting = deletingNoteId === note.id;
              const isMerging = mergingNoteId === note.id;
              const canMergeWithNext = index < notes.length - 1;
              const nextNote = canMergeWithNext ? notes[index + 1] : null;

              return (
                <div
                  key={note.id}
                  className="p-4 rounded-lg border border-border bg-muted/20 hover:border-border/80 transition-all space-y-2 group"
                >
                  {/* Card Header without redundant 'Voice Note' label */}
                  <div className="flex items-center justify-between gap-2">
                    <div className="flex items-center gap-2">
                      <span className="text-xs font-semibold text-foreground font-mono">
                        {formatNoteTimestamp(note.created_at)}
                      </span>
                      <Badge variant="outline" className="text-[10px] font-mono px-1.5 py-0">
                        {countWords(note.content)} words
                      </Badge>
                      {promotedNoteIds.has(note.id) && (
                        <Badge variant="outline" className="text-[9px] font-mono px-1.5 py-0 bg-primary/10 text-primary border-primary/25 gap-1">
                          <Sparkles className="w-2.5 h-2.5" />
                          <span>SCRIBBLE</span>
                        </Badge>
                      )}
                    </div>

                    {/* Action Buttons Toolbar */}
                    {!isEditing && (
                      <div className="flex items-center gap-1">
                        {/* Merge with adjacent earlier note */}
                        {canMergeWithNext && (
                          <Button
                            size="icon"
                            variant="ghost"
                            onClick={() => {
                              setMergingNoteId(isMerging ? null : note.id);
                              setDeletingNoteId(null);
                            }}
                            className={`h-7 w-7 rounded-lg transition-colors ${
                              isMerging
                                ? 'bg-primary/10 text-primary'
                                : 'text-muted-foreground hover:text-primary hover:bg-primary/10'
                            }`}
                            title="Merge with adjacent earlier note"
                            aria-label="Merge with adjacent earlier note"
                          >
                            <GitMerge className="w-3.5 h-3.5" />
                          </Button>
                        )}

                        {/* Save / Promote as Scribble */}
                        <Button
                          size="icon"
                          variant="ghost"
                          onClick={() => handlePromoteToScribble(note)}
                          disabled={actionBusy || promotedNoteIds.has(note.id)}
                          className={`h-7 w-7 rounded-lg transition-colors ${
                            promotedNoteIds.has(note.id)
                              ? 'bg-primary/10 text-primary cursor-default'
                              : 'text-muted-foreground hover:text-primary hover:bg-primary/10'
                          }`}
                          title={promotedNoteIds.has(note.id) ? 'Promoted to Scribble' : 'Save as Scribble (Promote into Knowledge Layer)'}
                          aria-label="Save as Scribble"
                        >
                          {promotedNoteIds.has(note.id) ? (
                            <Check className="w-3.5 h-3.5 text-primary" />
                          ) : (
                            <Sparkles className="w-3.5 h-3.5" />
                          )}
                        </Button>

                        {/* Edit Note */}
                        <Button
                          size="icon"
                          variant="ghost"
                          onClick={() => handleStartEdit(note)}
                          className="h-7 w-7 rounded-lg text-muted-foreground hover:text-foreground hover:bg-muted"
                          title="Edit transcript"
                          aria-label="Edit transcript"
                        >
                          <Edit3 className="w-3.5 h-3.5" />
                        </Button>

                        {/* Copy Content */}
                        <Button
                          size="icon"
                          variant="ghost"
                          onClick={() => handleCopy(note)}
                          className="h-7 w-7 rounded-lg text-muted-foreground hover:text-foreground hover:bg-muted"
                          title="Copy transcript"
                          aria-label="Copy transcript"
                        >
                          {copiedNoteId === note.id ? (
                            <Check className="w-3.5 h-3.5 text-emerald-500" />
                          ) : (
                            <Copy className="w-3.5 h-3.5" />
                          )}
                        </Button>

                        {/* Delete Note */}
                        <Button
                          size="icon"
                          variant="ghost"
                          onClick={() => {
                            setDeletingNoteId(isDeleting ? null : note.id);
                            setMergingNoteId(null);
                          }}
                          className={`h-7 w-7 rounded-lg transition-colors ${
                            isDeleting
                              ? 'bg-red-500/15 text-red-600 dark:text-red-400'
                              : 'text-muted-foreground hover:text-red-600 dark:hover:text-red-400 hover:bg-red-500/10'
                          }`}
                          title="Delete note"
                          aria-label="Delete note"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </Button>
                      </div>
                    )}
                  </div>

                  {/* Body: Editing Mode vs Normal Display */}
                  {isEditing ? (
                    <div className="space-y-2 pt-1">
                      <textarea
                        value={editingContent}
                        onChange={(e) => setEditingContent(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
                            handleSaveEdit(note.id);
                          }
                          if (e.key === 'Escape') {
                            handleCancelEdit();
                          }
                        }}
                        disabled={actionBusy}
                        className="w-full min-h-[90px] p-3 text-sm bg-background border border-border rounded-lg text-foreground focus:outline-none focus:ring-1 focus:ring-ring font-sans leading-relaxed resize-y"
                        autoFocus
                      />
                      <div className="flex items-center justify-between">
                        <span className="text-[11px] text-muted-foreground">
                          Press <kbd className="font-mono text-[10px] bg-muted px-1 py-0.5 rounded">Ctrl+Enter</kbd> to save, <kbd className="font-mono text-[10px] bg-muted px-1 py-0.5 rounded">Esc</kbd> to cancel
                        </span>
                        <div className="flex items-center gap-2">
                          <Button
                            size="sm"
                            variant="outline"
                            onClick={handleCancelEdit}
                            disabled={actionBusy}
                            className="h-7 text-xs gap-1"
                          >
                            <X className="w-3.5 h-3.5" />
                            <span>Cancel</span>
                          </Button>
                          <Button
                            size="sm"
                            variant="default"
                            onClick={() => handleSaveEdit(note.id)}
                            disabled={actionBusy || !editingContent.trim()}
                            className="h-7 text-xs gap-1"
                          >
                            <Save className="w-3.5 h-3.5" />
                            <span>Save Changes</span>
                          </Button>
                        </div>
                      </div>
                    </div>
                  ) : (
                    <p className="text-sm text-foreground whitespace-pre-wrap break-words leading-relaxed">
                      {note.content}
                    </p>
                  )}

                  {/* Delete Confirmation Inline Banner */}
                  {isDeleting && (
                    <div className="flex flex-wrap items-center justify-between gap-2 p-3 bg-red-500/10 border border-red-500/30 rounded-lg text-xs text-red-600 dark:text-red-400 animate-in fade-in duration-150">
                      <span className="font-medium">Move this Voice Note to Trash? (Kept for 30 days before permanent deletion)</span>
                      <div className="flex items-center gap-1.5">
                        <Button
                          size="sm"
                          variant="ghost"
                          disabled={actionBusy}
                          onClick={() => setDeletingNoteId(null)}
                          className="h-7 text-xs"
                        >
                          Cancel
                        </Button>
                        <Button
                          size="sm"
                          variant="destructive"
                          disabled={actionBusy}
                          onClick={() => handleDelete(note.id)}
                          className="h-7 text-xs gap-1 font-semibold"
                        >
                          <Trash2 className="w-3 h-3" />
                          <span>Move to Trash</span>
                        </Button>
                      </div>
                    </div>
                  )}

                  {/* Merge Confirmation Inline Banner */}
                  {isMerging && nextNote && (
                    <div className="p-3 bg-accent/40 border border-border rounded-lg text-xs space-y-2 animate-in fade-in duration-150">
                      <div className="flex items-center gap-1.5 font-semibold text-foreground">
                        <GitMerge className="w-4 h-4 text-primary shrink-0" />
                        <span>Merge with adjacent note ({formatNoteTimestamp(nextNote.created_at)})?</span>
                      </div>
                      <p className="text-[11px] text-muted-foreground line-clamp-2 italic bg-background/60 p-2 rounded border border-border/50">
                        "{nextNote.content}"
                      </p>
                      <div className="flex items-center justify-end gap-2 pt-1">
                        <Button
                          size="sm"
                          variant="ghost"
                          disabled={actionBusy}
                          onClick={() => setMergingNoteId(null)}
                          className="h-7 text-xs"
                        >
                          Cancel
                        </Button>
                        <Button
                          size="sm"
                          variant="default"
                          disabled={actionBusy}
                          onClick={() => handleMerge(note.id, nextNote.id)}
                          className="h-7 text-xs gap-1.5"
                        >
                          <GitMerge className="w-3.5 h-3.5" />
                          <span>Combine Notes</span>
                        </Button>
                      </div>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

    </div>
  );
};
