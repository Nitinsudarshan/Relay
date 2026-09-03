import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  AlertCircle,
  Archive,
  ArrowRight,
  Check,
  CheckCircle2,
  FileArchive,
  FileText,
  Loader2,
  RefreshCw,
  Search,
  Sparkles,
  Upload,
  X,
} from 'lucide-react';
import type { ExportInspection, VaultFile } from '../../types';

interface ImportConversationModalProps {
  onClose: () => void;
  onSuccess: (imported: VaultFile) => void;
}

export const ImportConversationModal: React.FC<ImportConversationModalProps> = ({
  onClose,
  onSuccess,
}) => {
  const [filePath, setFilePath] = useState<string>('');
  const [inspecting, setInspecting] = useState(false);
  const [inspection, setInspection] = useState<ExportInspection | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [filter, setFilter] = useState('');
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handlePickFile = async () => {
    setError(null);
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: 'AI Conversation Export',
            extensions: ['zip', 'json'],
          },
        ],
      });

      if (selected && typeof selected === 'string') {
        await inspectPath(selected);
      }
    } catch (err) {
      console.warn('Native open dialog failed:', err);
      setError('Could not open file picker. Please ensure the file path is accessible.');
    }
  };

  const inspectPath = async (path: string) => {
    setFilePath(path);
    setInspecting(true);
    setError(null);
    try {
      const result = await invoke<ExportInspection>('inspect_ai_conversation_export', { path });
      setInspection(result);
      if (result.conversations.length > 0) {
        setSelectedId(result.conversations[0].id);
      }
    } catch (err) {
      console.error('Inspection failed:', err);
      setError(
        typeof err === 'string'
          ? err
          : (err as Error)?.message || 'Failed to inspect export package. Is it a valid ChatGPT or Claude archive?',
      );
      setInspection(null);
    } finally {
      setInspecting(false);
    }
  };

  const handleImport = async () => {
    if (!filePath || !selectedId) return;

    setImporting(true);
    setError(null);
    try {
      const file = await invoke<VaultFile>('import_ai_conversation_export', {
        path: filePath,
        conversationId: selectedId,
      });
      onSuccess(file);
      onClose();
    } catch (err) {
      console.error('Import failed:', err);
      setError(
        typeof err === 'string'
          ? err
          : (err as Error)?.message || 'Failed to import conversation into Relay vault.',
      );
    } finally {
      setImporting(false);
    }
  };

  const filteredConversations = (inspection?.conversations ?? []).filter((c) =>
    c.title.toLowerCase().includes(filter.toLowerCase()),
  );

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="import-modal-title"
      className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm p-4"
    >
      <div className="relative flex max-h-[85vh] w-full max-w-2xl flex-col rounded-xl border border-border bg-card shadow-2xl overflow-hidden">
        {/* Header */}
        <header className="flex items-center justify-between border-b border-border px-5 py-4">
          <div className="flex items-center gap-2.5">
            <div className="rounded-lg bg-primary/10 p-2 text-primary">
              <Upload className="h-4 w-4" />
            </div>
            <div>
              <h2 id="import-modal-title" className="text-sm font-semibold text-foreground">
                Import AI Conversation
              </h2>
              <p className="text-xs text-muted-foreground">
                Import official exports from ChatGPT or Claude with assets and turn provenance
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close"
            className="rounded-md p-1.5 text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
          >
            <X className="h-4 w-4" />
          </button>
        </header>

        {/* Content Area */}
        <div className="flex-1 overflow-y-auto p-5 space-y-4">
          {error && (
            <div className="flex items-start gap-2.5 rounded-lg border border-destructive/40 bg-destructive/10 p-3 text-xs text-destructive">
              <AlertCircle className="h-4 w-4 shrink-0 mt-0.5" />
              <div className="flex-1">{error}</div>
            </div>
          )}

          {!inspection && (
            <div className="flex flex-col items-center justify-center rounded-xl border-2 border-dashed border-border p-10 text-center hover:border-primary/50 transition-colors">
              <div className="mb-4 rounded-full bg-primary/10 p-4 text-primary">
                {inspecting ? (
                  <Loader2 className="h-8 w-8 animate-spin" />
                ) : (
                  <FileArchive className="h-8 w-8" />
                )}
              </div>
              <h3 className="text-sm font-semibold text-foreground">
                {inspecting ? 'Inspecting Export Archive…' : 'Select Export File (.zip or .json)'}
              </h3>
              <p className="mt-1.5 max-w-sm text-xs text-muted-foreground leading-relaxed">
                Upload your ChatGPT data export (<code className="font-mono">conversations.json</code>)
                or Claude data export zip. Relay preserves the source and imports working assets.
              </p>
              <button
                type="button"
                disabled={inspecting}
                onClick={handlePickFile}
                className="mt-5 inline-flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground shadow-sm hover:opacity-90 transition-opacity disabled:opacity-50"
              >
                <Upload className="h-3.5 w-3.5" />
                <span>Choose File…</span>
              </button>
            </div>
          )}

          {inspection && (
            <div className="space-y-4">
              {/* Inspection Summary Bar */}
              <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-border bg-muted/40 px-4 py-3">
                <div className="flex items-center gap-2.5">
                  <span className="rounded-full bg-primary/15 px-2.5 py-1 text-xs font-semibold text-primary">
                    {inspection.provider_display}
                  </span>
                  <span className="text-xs text-muted-foreground">
                    Found <strong>{inspection.total_conversations}</strong> conversation(s)
                  </span>
                </div>
                <button
                  type="button"
                  onClick={handlePickFile}
                  className="text-xs text-muted-foreground hover:text-foreground underline underline-offset-2 transition-colors"
                >
                  Change file
                </button>
              </div>

              {/* Filter Search */}
              {inspection.total_conversations > 1 && (
                <div className="relative">
                  <Search className="pointer-events-none absolute left-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
                  <input
                    type="search"
                    value={filter}
                    onChange={(e) => setFilter(e.target.value)}
                    placeholder="Filter conversations by title…"
                    className="w-full rounded-lg border border-border bg-background py-1.5 pl-8 pr-3 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring"
                  />
                </div>
              )}

              {/* Conversation List */}
              <div className="max-h-64 overflow-y-auto divide-y divide-border/60 rounded-lg border border-border bg-background">
                {filteredConversations.length === 0 ? (
                  <div className="p-6 text-center text-xs text-muted-foreground">
                    No conversations match your filter.
                  </div>
                ) : (
                  filteredConversations.map((c) => {
                    const isSelected = selectedId === c.id;
                    return (
                      <div
                        key={c.id}
                        onClick={() => setSelectedId(c.id)}
                        className={`flex items-center justify-between gap-3 p-3 text-xs cursor-pointer transition-colors ${
                          isSelected ? 'bg-primary/10 text-foreground' : 'hover:bg-muted/50 text-foreground/90'
                        }`}
                      >
                        <div className="flex items-center gap-3 min-w-0">
                          <div
                            className={`flex h-4 w-4 shrink-0 items-center justify-center rounded-full border ${
                              isSelected
                                ? 'border-primary bg-primary text-primary-foreground'
                                : 'border-border'
                            }`}
                          >
                            {isSelected && <Check className="h-2.5 w-2.5" />}
                          </div>
                          <div className="min-w-0">
                            <p className="font-medium truncate">{c.title}</p>
                            <p className="text-[11px] text-muted-foreground">
                              {c.message_count} turn(s)
                              {c.created_at ? ` · ${c.created_at.slice(0, 10)}` : ''}
                            </p>
                          </div>
                        </div>

                        <div className="flex items-center gap-2 shrink-0">
                          {c.has_assets && (
                            <span className="rounded bg-muted px-2 py-0.5 text-[10px] font-medium text-muted-foreground">
                              Assets Included
                            </span>
                          )}
                          {c.already_imported_id && (
                            <span className="rounded bg-amber-500/10 px-2 py-0.5 text-[10px] font-medium text-amber-600 dark:text-amber-400">
                              Already in Vault
                            </span>
                          )}
                        </div>
                      </div>
                    );
                  })
                )}
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <footer className="flex items-center justify-between border-t border-border bg-muted/20 px-5 py-3">
          <button
            type="button"
            onClick={onClose}
            className="rounded-md border border-border px-3.5 py-1.5 text-xs font-medium text-foreground hover:bg-muted transition-colors"
          >
            Cancel
          </button>

          {inspection && (
            <button
              type="button"
              disabled={!selectedId || importing}
              onClick={handleImport}
              className="inline-flex items-center gap-2 rounded-md bg-primary px-4 py-1.5 text-xs font-medium text-primary-foreground shadow-sm hover:opacity-90 transition-opacity disabled:opacity-50"
            >
              {importing ? (
                <>
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  <span>Importing & Analyzing…</span>
                </>
              ) : (
                <>
                  <Sparkles className="h-3.5 w-3.5" />
                  <span>Import Selected</span>
                </>
              )}
            </button>
          )}
        </footer>
      </div>
    </div>
  );
};
