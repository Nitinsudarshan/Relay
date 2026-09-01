import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  FileText,
  UploadCloud,
  Search,
  Sparkles,
  Wand2,
  ExternalLink,
  BookOpen,
  Trash2,
  AlertCircle,
  FileCode,
  Tag,
  Layers,
  Filter,
  CheckCircle2
} from 'lucide-react';
import { VaultFile, Scribble } from '../../types';
import { FileDetailModal } from './FileDetailModal';
import { ConfirmationModal } from '../common/ConfirmationModal';

interface FilesPageProps {
  onNavigateTab?: (tab: string) => void;
}

const SUPPORTED_EXTENSIONS = ['pdf', 'docx', 'doc', 'md', 'markdown', 'txt'];

export const FilesPage: React.FC<FilesPageProps> = ({ onNavigateTab }) => {
  const [files, setFiles] = useState<VaultFile[]>([]);
  const [loading, setLoading] = useState(true);
  const [errorBanner, setErrorBanner] = useState<string | null>(null);
  const [successBanner, setSuccessBanner] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedFilter, setSelectedFilter] = useState<string>('all');
  const [selectedFile, setSelectedFile] = useState<VaultFile | null>(null);
  const [fileToDelete, setFileToDelete] = useState<VaultFile | null>(null);
  const [isDragOver, setIsDragOver] = useState(false);
  const [importing, setImporting] = useState(false);

  useEffect(() => {
    loadFiles();

    let unlisten: (() => void) | undefined;
    import('@tauri-apps/api/event')
      .then(({ listen }) => {
        listen<any>('tauri://drag-drop', (event) => {
          if (event.payload?.paths && Array.isArray(event.payload.paths)) {
            for (const path of event.payload.paths) {
              handleImportPath(path);
            }
          }
        }).then((fn) => {
          unlisten = fn;
        });
      })
      .catch(() => {});

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const loadFiles = async () => {
    setLoading(true);
    try {
      const res = await invoke<VaultFile[]>('get_vault_files');
      setFiles(res);
    } catch (err) {
      console.error('Failed to load vault files:', err);
      setErrorBanner('Failed to load Vault Files. Ensure Relay Vault is initialized.');
    } finally {
      setLoading(false);
    }
  };

  const handleImportPath = async (sourcePath: string) => {
    setErrorBanner(null);
    setSuccessBanner(null);
    const ext = sourcePath.split('.').pop()?.toLowerCase() || '';

    if (!SUPPORTED_EXTENSIONS.includes(ext)) {
      setErrorBanner(`Format .${ext} is not supported. Please import .md, .txt, .pdf, or .docx files.`);
      return;
    }

    setImporting(true);
    try {
      const imported = await invoke<VaultFile>('import_vault_file', { sourcePath });
      setSuccessBanner(`Successfully imported ${imported.original_filename} into Vault (original file left untouched).`);
      await loadFiles();
    } catch (err: any) {
      console.error('Failed to import file:', err);
      setErrorBanner(`Import failed: ${err?.message || err}`);
    } finally {
      setImporting(false);
    }
  };

  const handleImportFileObject = async (file: File) => {
    setErrorBanner(null);
    setSuccessBanner(null);
    const ext = file.name.split('.').pop()?.toLowerCase() || '';

    if (!SUPPORTED_EXTENSIONS.includes(ext)) {
      setErrorBanner(`Format .${ext} is not supported. Please import .md, .txt, .pdf, or .docx files.`);
      return;
    }

    setImporting(true);
    try {
      const rawPath = (file as any).path as string | undefined;

      // 1. Try importing by absolute source path if available
      if (rawPath && (rawPath.includes(':\\') || rawPath.startsWith('/'))) {
        try {
          const imported = await invoke<VaultFile>('import_vault_file', { sourcePath: rawPath });
          setSuccessBanner(`Successfully imported ${imported.original_filename} into Vault (original file left untouched).`);
          await loadFiles();
          return;
        } catch (pathErr) {
          console.warn('Path-based import failed, falling back to byte import:', pathErr);
        }
      }

      // 2. Read bytes directly from File object as fallback — 100% reliable for browser/drag-drop!
      const arrayBuffer = await file.arrayBuffer();
      const bytes = Array.from(new Uint8Array(arrayBuffer));
      const imported = await invoke<VaultFile>('import_vault_file_bytes', {
        filename: file.name,
        bytes,
        sourcePath: rawPath || null,
      });

      setSuccessBanner(`Successfully imported ${imported.original_filename} into Vault (original file left untouched).`);
      await loadFiles();
    } catch (err: any) {
      console.error('Failed to import file object:', err);
      setErrorBanner(`Import failed: ${err?.message || err}`);
    } finally {
      setImporting(false);
    }
  };

  const handlePickFile = async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        multiple: true,
        filters: [
          {
            name: 'Documents',
            extensions: ['pdf', 'docx', 'doc', 'md', 'markdown', 'txt']
          }
        ]
      });

      if (selected) {
        const paths = Array.isArray(selected) ? selected : [selected];
        for (const p of paths) {
          if (p) await handleImportPath(p);
        }
        return;
      }
    } catch (err) {
      console.warn('Native dialog open unavailable, using fallback HTML file input:', err);
    }

    // Fallback HTML File Input
    const input = document.createElement('input');
    input.type = 'file';
    input.multiple = true;
    input.accept = '.pdf,.docx,.doc,.md,.markdown,.txt';
    input.onchange = async (e: Event) => {
      const target = e.target as HTMLInputElement;
      if (target.files) {
        for (let i = 0; i < target.files.length; i++) {
          await handleImportFileObject(target.files[i]);
        }
      }
    };
    input.click();
  };

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(true);
  };

  const handleDragLeave = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(false);
  };

  const handleDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(false);

    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      for (let i = 0; i < e.dataTransfer.files.length; i++) {
        await handleImportFileObject(e.dataTransfer.files[i]);
      }
    }
  };

  const handleSummarize = async (id: string) => {
    try {
      const updated = await invoke<VaultFile>('summarize_vault_file', { id });
      setFiles((prev) => prev.map((f) => (f.id === id ? updated : f)));
      if (selectedFile?.id === id) setSelectedFile(updated);
      setSuccessBanner(`Summary generated for ${updated.original_filename}.`);
    } catch (err: any) {
      setErrorBanner(`Summarize failed: ${err?.message || err}`);
    }
  };

  const handleEnrich = async (id: string) => {
    try {
      const updated = await invoke<VaultFile>('enrich_vault_file', { id });
      setFiles((prev) => prev.map((f) => (f.id === id ? updated : f)));
      if (selectedFile?.id === id) setSelectedFile(updated);
      setSuccessBanner(`Analysis complete for ${updated.original_filename}.`);
    } catch (err: any) {
      setErrorBanner(`Analyse failed: ${err?.message || err}`);
    }
  };

  const handleCreateScribble = async (id: string) => {
    try {
      const scribble = await invoke<Scribble>('create_scribble_from_vault_file', { id });
      setSuccessBanner(`Created new Scribble "${scribble.title}" linked to file.`);
      if (onNavigateTab) onNavigateTab('scribbles');
      return scribble;
    } catch (err: any) {
      setErrorBanner(`Create Scribble failed: ${err?.message || err}`);
    }
  };

  const handleReprocess = async (id: string) => {
    try {
      const updated = await invoke<VaultFile>('reprocess_vault_file', { id });
      setFiles((prev) => prev.map((f) => (f.id === id ? updated : f)));
      if (selectedFile?.id === id) setSelectedFile(updated);
      setSuccessBanner(`Re-analyzed ${updated.original_filename}.`);
    } catch (err: any) {
      setErrorBanner(`Re-process failed: ${err?.message || err}`);
    }
  };

  const handleUpdateTags = async (id: string, tags: string[], topics: string[], entities: string[]) => {
    try {
      const updated = await invoke<VaultFile>('update_vault_file_tags', { id, tags, topics, entities });
      setFiles((prev) => prev.map((f) => (f.id === id ? updated : f)));
      if (selectedFile?.id === id) setSelectedFile(updated);
    } catch (err: any) {
      setErrorBanner(`Update tags failed: ${err?.message || err}`);
    }
  };

  const handleDelete = async (id: string, filename: string) => {
    try {
      await invoke('delete_vault_file', { id });
      setFiles((prev) => prev.filter((f) => f.id !== id));
      if (selectedFile?.id === id) setSelectedFile(null);
      setSuccessBanner(`Moved ${filename} to Relay Trash (original file remains untouched).`);
    } catch (err: any) {
      setErrorBanner(`Delete failed: ${err?.message || err}`);
    }
  };

  const handleOpenLocation = async (id: string) => {
    try {
      await invoke('open_vault_file_location', { id });
    } catch (err: any) {
      setErrorBanner(`Open folder failed: ${err?.message || err}`);
    }
  };

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  const formatDate = (iso: string) => {
    try {
      return new Date(iso).toLocaleDateString(undefined, {
        month: 'short',
        day: 'numeric',
        year: 'numeric'
      });
    } catch {
      return iso;
    }
  };

  const filteredFiles = files.filter((f) => {
    const matchesSearch =
      f.original_filename.toLowerCase().includes(searchQuery.toLowerCase()) ||
      f.tags.some((t) => t.toLowerCase().includes(searchQuery.toLowerCase())) ||
      f.topics.some((t) => t.toLowerCase().includes(searchQuery.toLowerCase())) ||
      f.entities.some((e) => e.toLowerCase().includes(searchQuery.toLowerCase())) ||
      f.content.toLowerCase().includes(searchQuery.toLowerCase());

    if (!matchesSearch) return false;

    if (selectedFilter === 'all') return true;
    if (selectedFilter === 'pdf') return f.file_type === 'pdf';
    if (selectedFilter === 'word') return f.file_type === 'docx' || f.file_type === 'doc';
    if (selectedFilter === 'markdown') return f.file_type === 'md' || f.file_type === 'markdown';
    if (selectedFilter === 'text') return f.file_type === 'txt';
    return true;
  });

  return (
    <div className="p-6 max-w-7xl mx-auto space-y-6">
      {/* Header */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-foreground flex items-center gap-2">
            <FileText className="w-7 h-7 text-primary" />
            Files Vault
          </h1>
          <p className="text-sm text-muted-foreground mt-1">
            Import existing documents into Relay's knowledge graph. Your original files outside Relay remain 100% untouched.
          </p>
        </div>

        <button
          onClick={handlePickFile}
          disabled={importing}
          className="inline-flex items-center gap-2 px-4 py-2.5 bg-primary text-primary-foreground font-medium rounded-xl hover:bg-primary/90 transition-colors shadow-lg shadow-primary/20 disabled:opacity-50"
        >
          <UploadCloud className="w-4 h-4" />
          {importing ? 'Importing...' : 'Add Files'}
        </button>
      </div>

      {/* Alert Banners */}
      {errorBanner && (
        <div className="p-4 rounded-xl bg-destructive/10 border border-destructive/20 text-destructive text-sm flex items-start justify-between gap-3 animate-in fade-in duration-150">
          <div className="flex items-start gap-2">
            <AlertCircle className="w-5 h-5 shrink-0 mt-0.5" />
            <span>{errorBanner}</span>
          </div>
          <button onClick={() => setErrorBanner(null)} className="text-destructive hover:opacity-80 font-bold">
            ×
          </button>
        </div>
      )}

      {successBanner && (
        <div className="p-4 rounded-xl bg-emerald-500/10 border border-emerald-500/20 text-emerald-600 dark:text-emerald-400 text-sm flex items-start justify-between gap-3 animate-in fade-in duration-150">
          <div className="flex items-start gap-2">
            <CheckCircle2 className="w-5 h-5 shrink-0 mt-0.5" />
            <span>{successBanner}</span>
          </div>
          <button onClick={() => setSuccessBanner(null)} className="hover:opacity-80 font-bold">
            ×
          </button>
        </div>
      )}

      {/* Drag and Drop Zone */}
      <div
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        onClick={handlePickFile}
        className={`p-8 border-2 border-dashed rounded-2xl cursor-pointer text-center transition-all duration-200 ${
          isDragOver
            ? 'border-primary bg-primary/5 scale-[1.01]'
            : 'border-border/70 hover:border-primary/50 hover:bg-muted/30 bg-card'
        }`}
      >
        <UploadCloud className={`w-10 h-10 mx-auto mb-3 transition-colors ${isDragOver ? 'text-primary' : 'text-muted-foreground'}`} />
        <p className="text-sm font-medium text-foreground">
          Drag and drop documents here, or <span className="text-primary underline">browse your computer</span>
        </p>
        <p className="text-xs text-muted-foreground mt-1">
          Supports <span className="font-semibold text-foreground">.pdf</span>, <span className="font-semibold text-foreground">.docx</span>, <span className="font-semibold text-foreground">.md</span>, <span className="font-semibold text-foreground">.txt</span>
        </p>
      </div>

      {/* Search and Filters Toolbar */}
      <div className="flex flex-col sm:flex-row items-center justify-between gap-3">
        <div className="relative w-full sm:w-80">
          <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
          <input
            type="text"
            placeholder="Search filenames, text, or tags..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full pl-9 pr-4 py-2 text-sm bg-card border border-border rounded-xl focus:outline-none focus:ring-2 focus:ring-primary"
          />
        </div>

        <div className="flex items-center gap-1 bg-muted/40 p-1 rounded-xl border border-border overflow-x-auto w-full sm:w-auto">
          {[
            { id: 'all', label: 'All Files' },
            { id: 'pdf', label: 'PDF' },
            { id: 'word', label: 'Word' },
            { id: 'markdown', label: 'Markdown' },
            { id: 'text', label: 'Text' }
          ].map((f) => (
            <button
              key={f.id}
              onClick={() => setSelectedFilter(f.id)}
              className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-colors whitespace-nowrap ${
                selectedFilter === f.id
                  ? 'bg-card text-foreground shadow-sm'
                  : 'text-muted-foreground hover:text-foreground'
              }`}
            >
              {f.label}
            </button>
          ))}
        </div>
      </div>

      {/* Files List / Grid */}
      {loading ? (
        <div className="p-12 text-center text-muted-foreground">Loading vault files...</div>
      ) : filteredFiles.length === 0 ? (
        <div className="p-12 text-center bg-card rounded-2xl border border-border/60">
          <FileCode className="w-12 h-12 mx-auto text-muted-foreground/40 mb-3" />
          <h3 className="text-base font-semibold text-foreground">No files found</h3>
          <p className="text-xs text-muted-foreground mt-1 max-w-md mx-auto">
            {searchQuery || selectedFilter !== 'all'
              ? 'No imported files match your search criteria.'
              : 'Add documents to Relay to extract text, summarize, analyze, and integrate into Talkback context.'}
          </p>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {filteredFiles.map((file) => (
            <div
              key={file.id}
              className="bg-card hover:bg-muted/20 border border-border rounded-xl p-5 shadow-sm hover:shadow-md transition-all flex flex-col justify-between space-y-4 group"
            >
              <div className="space-y-3">
                {/* Card Top Row */}
                <div className="flex items-start justify-between gap-3">
                  <div
                    onClick={() => setSelectedFile(file)}
                    className="flex items-center gap-3 cursor-pointer group-hover:text-primary transition-colors flex-1 min-w-0"
                  >
                    <div className="p-2.5 bg-primary/10 text-primary rounded-lg shrink-0">
                      <FileText className="w-5 h-5" />
                    </div>
                    <div className="min-w-0">
                      <h3 className="text-sm font-bold text-foreground truncate group-hover:text-primary transition-colors">
                        {file.original_filename}
                      </h3>
                      <p className="text-xs text-muted-foreground mt-0.5 flex items-center gap-2">
                        <span className="uppercase font-semibold text-[10px] bg-muted px-1.5 py-0.5 rounded">
                          {file.file_type}
                        </span>
                        <span>•</span>
                        <span>{formatBytes(file.size_bytes)}</span>
                      </p>
                    </div>
                  </div>

                  <button
                    onClick={() => setFileToDelete(file)}
                    title="Move file to Relay Trash"
                    className="p-1.5 text-muted-foreground hover:text-destructive opacity-0 group-hover:opacity-100 transition-opacity"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>

                {/* Summary Preview */}
                {file.summary && (
                  <p className="text-xs text-muted-foreground line-clamp-2 bg-muted/30 p-2.5 rounded-lg border border-border/40">
                    {file.summary.replace(/^[0-9]\.\s\*\*[^*]+\*\*\s*/, '')}
                  </p>
                )}

                {/* Topics & Tags */}
                {(file.topics.length > 0 || file.entities.length > 0) && (
                  <div className="flex flex-wrap gap-1">
                    {file.topics.slice(0, 3).map((t, i) => (
                      <span key={i} className="px-2 py-0.5 text-[10px] font-medium rounded-md bg-secondary text-secondary-foreground">
                        {t}
                      </span>
                    ))}
                    {file.entities.slice(0, 2).map((e, i) => (
                      <span key={i} className="px-2 py-0.5 text-[10px] font-medium rounded-md bg-accent text-accent-foreground">
                        {e}
                      </span>
                    ))}
                  </div>
                )}
              </div>

              {/* Card Footer Actions */}
              <div className="pt-3 border-t border-border/60 flex items-center justify-between text-xs">
                <span className="text-[10px] text-muted-foreground">Imported {formatDate(file.created_at)}</span>

                <div className="flex items-center gap-1">
                  <button
                    onClick={() => handleEnrich(file.id)}
                    title={file.ai_metadata?.last_enriched_at ? "Re-analyse file" : "Analyse file"}
                    className="p-1.5 text-muted-foreground hover:text-primary hover:bg-primary/10 rounded-lg transition-colors"
                  >
                    <Wand2 className="w-3.5 h-3.5 text-primary" />
                  </button>

                  {file.linked_scribble_id ? (
                    <button
                      onClick={() => onNavigateTab?.('scribble')}
                      title="Scribble Linked — View in Knowledge Layer"
                      className="p-1.5 text-amber-500 hover:bg-amber-500/10 rounded-lg transition-colors"
                    >
                      <Sparkles className="w-3.5 h-3.5 text-emerald-500" />
                    </button>
                  ) : (
                    <button
                      onClick={() => handleCreateScribble(file.id)}
                      title="Create Scribble linked to file"
                      className="p-1.5 text-muted-foreground hover:text-amber-500 hover:bg-amber-500/10 rounded-lg transition-colors"
                    >
                      <Sparkles className="w-3.5 h-3.5 text-amber-500" />
                    </button>
                  )}

                  <button
                    onClick={() => setSelectedFile(file)}
                    className="px-2.5 py-1 text-xs font-semibold bg-muted hover:bg-muted/80 text-foreground rounded-lg transition-colors"
                  >
                    View
                  </button>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Confirmation Modal for File Deletion */}
      <ConfirmationModal
        isOpen={Boolean(fileToDelete)}
        title="Move File to Trash"
        description={`Are you sure you want to move "${fileToDelete?.original_filename || 'this file'}" to Relay Trash? Deleted files remain in Trash for 30 days before permanent automatic purge.`}
        confirmLabel="Move to Trash"
        variant="destructive"
        onConfirm={async () => {
          if (fileToDelete) {
            await handleDelete(fileToDelete.id, fileToDelete.original_filename);
            setFileToDelete(null);
          }
        }}
        onCancel={() => setFileToDelete(null)}
      />

      {/* Detail Modal */}
      {selectedFile && (
        <FileDetailModal
          file={selectedFile}
          onClose={() => setSelectedFile(null)}
          onSummarize={handleSummarize}
          onEnrich={handleEnrich}
          onCreateScribble={handleCreateScribble}
          onReprocess={handleReprocess}
          onUpdateTags={handleUpdateTags}
          onOpenLocation={handleOpenLocation}
          onNavigateTab={onNavigateTab}
        />
      )}
    </div>
  );
};
