import React, { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { HardDrive, Mic, ShieldCheck } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { VaultLocationInfo, VaultNote } from '../../types';

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
  <div className="flex-1 flex flex-col items-center justify-center text-center py-16 px-6 rounded-2xl border border-dashed border-border bg-card">
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

  const refreshLocation = async () => {
    try {
      const info = await invoke<VaultLocationInfo>('get_vault_location');
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
      // Must not leave the page stuck on "Loading" forever — treat an
      // unreadable location the same as an inaccessible one, so the user
      // always lands on an actionable screen.
      setVaultState({ status: 'recovery' });
    }
  };

  useEffect(() => {
    refreshLocation();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (vaultState.status !== 'ready') return;
    invoke<VaultNote[]>('get_voice_notes')
      .then(setNotes)
      .catch((err) => console.error('Failed to load Voice Notes', err));
  }, [vaultState.status]);

  // Keeps Transcript History live while this page is open — the backend
  // emits this exactly once per successful, non-empty transcript, from
  // both the global dictation hotkey and click-to-talk.
  useEffect(() => {
    const unlistenPromise = listen<VaultNote>('voice-note-saved', ({ payload }) => {
      setNotes((prev) => [payload, ...prev.filter((n) => n.id !== payload.id)]);
    });
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

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
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4 shrink-0">
        <div className="rounded-xl border border-border bg-card p-4">
          <p className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground mb-1.5">
            Total Voice Notes
          </p>
          <p className="text-2xl font-extrabold text-foreground">{stats.total}</p>
        </div>
        <div className="rounded-xl border border-border bg-card p-4">
          <p className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground mb-1.5">
            Total Words
          </p>
          <p className="text-2xl font-extrabold text-foreground">{stats.totalWords.toLocaleString()}</p>
        </div>
        <div className="rounded-xl border border-border bg-card p-4">
          <p className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground mb-1.5">
            Notes Today
          </p>
          <p className="text-2xl font-extrabold text-foreground">{stats.notesToday}</p>
        </div>
      </div>

      <div className="flex-1 flex flex-col min-h-0 rounded-2xl border border-border bg-card p-5">
        <div className="flex items-center justify-between mb-4 shrink-0">
          <h2 className="text-sm font-bold text-foreground">Transcript History</h2>
          <Badge variant="outline" className="text-[10px] font-mono">
            {notes.length} voice note{notes.length === 1 ? '' : 's'}
          </Badge>
        </div>

        {notes.length === 0 ? (
          <div className="flex-1 flex flex-col items-center justify-center text-center py-10 border border-dashed border-border rounded-xl text-muted-foreground">
            <Mic className="w-8 h-8 mb-2 opacity-40" />
            <p className="text-sm font-semibold">No Voice Notes yet</p>
            <p className="text-xs mt-1">Everything you dictate with Relay will show up here.</p>
          </div>
        ) : (
          <div className="flex-1 overflow-y-auto space-y-2">
            {notes.map((note) => (
              <div key={note.id} className="p-4 rounded-xl border border-border bg-muted/20">
                <div className="flex items-center justify-between gap-2 mb-1.5">
                  <span className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground">
                    Voice Note
                  </span>
                  <span className="text-[11px] text-muted-foreground font-mono shrink-0">
                    {formatNoteTimestamp(note.created_at)}
                  </span>
                </div>
                <p className="text-sm text-foreground whitespace-pre-wrap break-words leading-relaxed">
                  {note.content}
                </p>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};
