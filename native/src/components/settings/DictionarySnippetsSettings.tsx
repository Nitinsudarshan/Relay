import React, { useState } from 'react';
import {
  BookOpen,
  Sparkles,
  Plus,
  Trash2,
  Edit3,
  Search,
  Upload,
  Download,
  Check,
  Zap,
  Copy,
} from 'lucide-react';
import { AppSettings, SnippetItem } from '../../types';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Switch } from '@/components/ui/switch';
import { TriggerSettings } from './TriggerSettings';

interface DictionarySnippetsSettingsProps {
  settings: AppSettings;
  onUpdateSettings: (updater: (prev: AppSettings) => AppSettings) => void;
  onSaveDirect: () => Promise<void>;
}

const DEFAULT_SYSTEM_WORDS = [
  'Relay',
  'Whisper',
  'Tauri',
  'Rust',
  'Supabase',
  'LanceDB',
  'Ollama',
];

export const DictionarySnippetsSettings: React.FC<DictionarySnippetsSettingsProps> = ({
  settings,
  onUpdateSettings,
  onSaveDirect,
}) => {
  const [activeTab, setActiveTab] = useState<'dictionary' | 'snippets' | 'triggers'>('dictionary');

  // Dictionary State
  const [dictInput, setDictInput] = useState('');
  const [dictSearch, setDictSearch] = useState('');
  const [dictMessage, setDictMessage] = useState<string | null>(null);

  // Snippets State
  const [snippetTriggerInput, setSnippetTriggerInput] = useState('');
  const [snippetSearch, setSnippetSearch] = useState('');
  const [editingSnippet, setEditingSnippet] = useState<SnippetItem | null>(null);
  const [newSnippetModalOpen, setNewSnippetModalOpen] = useState(false);
  const [newSnippetTitle, setNewSnippetTitle] = useState('');
  const [newSnippetTrigger, setNewSnippetTrigger] = useState('');
  const [newSnippetText, setNewSnippetText] = useState('');
  const [copiedSnippetId, setCopiedSnippetId] = useState<string | null>(null);

  const words = settings.dictionary || DEFAULT_SYSTEM_WORDS;
  const snippets = settings.snippets || [];

  // --- DICTIONARY HANDLERS ---
  const handleAddDictionaryWords = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    if (!dictInput.trim()) return;

    const rawWords = dictInput
      .split(/[,;\n]/)
      .map((w) => w.trim())
      .filter((w) => w.length > 0);

    if (rawWords.length === 0) return;

    const currentWords = settings.dictionary || DEFAULT_SYSTEM_WORDS;
    const combined = Array.from(new Set([...currentWords, ...rawWords]));

    onUpdateSettings((prev) => ({
      ...prev,
      dictionary: combined,
    }));
    setDictInput('');
    setDictMessage(`Added ${rawWords.length} word${rawWords.length > 1 ? 's' : ''}`);
    setTimeout(() => setDictMessage(null), 2500);
    await onSaveDirect();
  };

  const handleDeleteDictionaryWord = async (wordToDelete: string) => {
    const currentWords = settings.dictionary || DEFAULT_SYSTEM_WORDS;
    const updated = currentWords.filter((w) => w.toLowerCase() !== wordToDelete.toLowerCase());
    onUpdateSettings((prev) => ({
      ...prev,
      dictionary: updated,
    }));
    await onSaveDirect();
  };

  const handleExportDictionary = () => {
    const currentWords = settings.dictionary || DEFAULT_SYSTEM_WORDS;
    const blob = new Blob([currentWords.join(', ')], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'relay-dictionary.txt';
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleImportDictionary = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = async (ev) => {
      const text = ev.target?.result as string;
      if (!text) return;
      const imported = text
        .split(/[,;\n]/)
        .map((w) => w.trim())
        .filter((w) => w.length > 0);
      if (imported.length > 0) {
        const currentWords = settings.dictionary || DEFAULT_SYSTEM_WORDS;
        const combined = Array.from(new Set([...currentWords, ...imported]));
        onUpdateSettings((prev) => ({
          ...prev,
          dictionary: combined,
        }));
        setDictMessage(`Imported ${imported.length} words`);
        setTimeout(() => setDictMessage(null), 2500);
        await onSaveDirect();
      }
    };
    reader.readAsText(file);
  };

  // --- SNIPPETS HANDLERS ---
  const handleQuickAddSnippet = (e: React.FormEvent) => {
    e.preventDefault();
    if (!snippetTriggerInput.trim()) return;
    setNewSnippetTrigger(snippetTriggerInput.trim());
    setNewSnippetTitle(snippetTriggerInput.trim());
    setNewSnippetText('');
    setNewSnippetModalOpen(true);
    setSnippetTriggerInput('');
  };

  const handleSaveNewSnippet = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newSnippetTrigger.trim() || !newSnippetText.trim()) return;

    const newSnippet: SnippetItem = {
      id: `snip_${Date.now()}`,
      trigger: newSnippetTrigger.trim(),
      snippet_text: newSnippetText.trim(),
      label: newSnippetTitle.trim() || newSnippetTrigger.trim(),
      enabled: true,
    };

    const updated = [...snippets, newSnippet];
    onUpdateSettings((prev) => ({
      ...prev,
      snippets: updated,
    }));
    setNewSnippetModalOpen(false);
    setNewSnippetTitle('');
    setNewSnippetTrigger('');
    setNewSnippetText('');
    await onSaveDirect();
  };

  const handleUpdateEditingSnippet = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!editingSnippet || !editingSnippet.trigger.trim() || !editingSnippet.snippet_text.trim()) return;

    const updated = snippets.map((s) => (s.id === editingSnippet.id ? editingSnippet : s));
    onUpdateSettings((prev) => ({
      ...prev,
      snippets: updated,
    }));
    setEditingSnippet(null);
    await onSaveDirect();
  };

  const handleDeleteSnippet = async (id: string) => {
    const updated = snippets.filter((s) => s.id !== id);
    onUpdateSettings((prev) => ({
      ...prev,
      snippets: updated,
    }));
    await onSaveDirect();
  };

  const handleToggleSnippet = async (id: string, enabled: boolean) => {
    const updated = snippets.map((s) => (s.id === id ? { ...s, enabled } : s));
    onUpdateSettings((prev) => ({
      ...prev,
      snippets: updated,
    }));
    await onSaveDirect();
  };

  const handleCopySnippetText = (id: string, text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedSnippetId(id);
    setTimeout(() => setCopiedSnippetId(null), 2000);
  };

  const filteredWords = words.filter((w) =>
    w.toLowerCase().includes(dictSearch.toLowerCase())
  );

  const filteredSnippets = snippets.filter(
    (s) =>
      s.trigger.toLowerCase().includes(snippetSearch.toLowerCase()) ||
      (s.label && s.label.toLowerCase().includes(snippetSearch.toLowerCase())) ||
      s.snippet_text.toLowerCase().includes(snippetSearch.toLowerCase())
  );

  return (
    <div className="space-y-6">
      {/* Header & Sub-Navigation */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 border-b border-border pb-4">
        <div>
          <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
            VOCABULARY & EXPANSIONS
          </p>
          <h2 className="text-lg font-bold text-foreground">Custom Dictionary & Spoken Snippets</h2>
        </div>

        {/* Tab Pills */}
        <div className="flex bg-muted p-1 rounded-lg border border-border shrink-0">
          <button
            type="button"
            onClick={() => setActiveTab('dictionary')}
            className={`flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-md transition-all ${
              activeTab === 'dictionary'
                ? 'bg-card text-foreground font-semibold shadow-xs'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            <BookOpen className="w-3.5 h-3.5" />
            <span>Dictionary</span>
            <Badge variant="secondary" className="text-[9px] px-1 py-0 ml-1">
              {words.length}
            </Badge>
          </button>
          <button
            type="button"
            onClick={() => setActiveTab('snippets')}
            className={`flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-md transition-all ${
              activeTab === 'snippets'
                ? 'bg-card text-foreground font-semibold shadow-xs'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            <Sparkles className="w-3.5 h-3.5" />
            <span>Snippets</span>
            <Badge variant="secondary" className="text-[9px] px-1 py-0 ml-1">
              {snippets.length}
            </Badge>
          </button>
          <button
            type="button"
            onClick={() => setActiveTab('triggers')}
            className={`flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-md transition-all ${
              activeTab === 'triggers'
                ? 'bg-card text-foreground font-semibold shadow-xs'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            <Zap className="w-3.5 h-3.5" />
            <span>MCP Triggers</span>
          </button>
        </div>
      </div>

      {/* 1. DICTIONARY TAB */}
      {activeTab === 'dictionary' && (
        <div className="space-y-4 animate-in fade-in-50">
          {/* Quick Add Bar (OpenWhispr Style) */}
          <form onSubmit={handleAddDictionaryWords} className="relative flex items-center gap-2">
            <div className="relative flex-1">
              <Input
                value={dictInput}
                onChange={(e) => setDictInput(e.target.value)}
                placeholder="Add words separated by commas — Relay, Supabase, John Snow, ARR"
                className="h-10 text-xs pl-3 pr-16 bg-muted/30 border-border focus:bg-background"
              />
              <span className="absolute right-3 top-1/2 -translate-y-1/2 text-[10px] text-muted-foreground font-mono">
                Press Enter ↵
              </span>
            </div>
            <Button type="submit" size="sm" className="h-10 px-4 text-xs gap-1.5" disabled={!dictInput.trim()}>
              <Plus className="w-3.5 h-3.5" />
              <span>Add</span>
            </Button>
          </form>

          {dictMessage && (
            <div className="text-xs text-emerald-500 flex items-center gap-1.5 animate-in fade-in">
              <Check className="w-3.5 h-3.5" />
              <span>{dictMessage}</span>
            </div>
          )}

          {/* Search & Actions Header */}
          <div className="flex items-center justify-between gap-3 pt-2">
            <div className="relative max-w-xs flex-1">
              <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground" />
              <Input
                value={dictSearch}
                onChange={(e) => setDictSearch(e.target.value)}
                placeholder="Search vocabulary..."
                className="h-8 text-xs pl-8 bg-background border-border"
              />
            </div>
            <div className="flex items-center gap-2">
              <label className="cursor-pointer inline-flex items-center justify-center">
                <input
                  type="file"
                  accept=".txt,.csv"
                  onChange={handleImportDictionary}
                  className="hidden"
                />
                <span className="inline-flex items-center justify-center h-8 px-3 text-xs font-medium rounded-md border border-input bg-background hover:bg-accent hover:text-accent-foreground gap-1.5 cursor-pointer">
                  <Upload className="w-3.5 h-3.5 text-muted-foreground" />
                  <span>Import List</span>
                </span>
              </label>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-8 text-xs gap-1.5"
                onClick={handleExportDictionary}
              >
                <Download className="w-3.5 h-3.5 text-muted-foreground" />
                <span>Export</span>
              </Button>
            </div>
          </div>

          {/* Words Chips Grid */}
          {words.length === 0 ? (
            <div className="text-center py-12 px-4 rounded-lg border border-dashed border-border bg-muted/10 flex flex-col items-center justify-center space-y-3">
              <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center text-primary">
                <BookOpen className="w-5 h-5" />
              </div>
              <div className="space-y-1">
                <h3 className="text-sm font-semibold text-foreground">Your dictionary is empty</h3>
                <p className="text-xs text-muted-foreground max-w-sm">
                  Add words Relay should always get right: technical terms, company names, acronyms, and proper nouns.
                </p>
              </div>
            </div>
          ) : (
            <div className="p-4 rounded-lg bg-card border border-border space-y-3 shadow-xs">
              <div className="flex items-center justify-between text-xs text-muted-foreground border-b border-border/60 pb-2">
                <span>Recognized Vocabulary ({filteredWords.length} words)</span>
                <span className="text-[10px]">Injected directly into STT initial prompt</span>
              </div>
              <div className="flex flex-wrap gap-2 pt-1 max-h-[380px] overflow-y-auto">
                {filteredWords.map((word) => {
                  const isDefault = DEFAULT_SYSTEM_WORDS.includes(word);
                  return (
                    <div
                      key={word}
                      className="group flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-muted/60 border border-border text-xs text-foreground hover:border-primary/50 transition-colors shadow-2xs"
                    >
                      <span className="font-medium">{word}</span>
                      {isDefault && (
                        <span className="text-[9px] text-muted-foreground/70 font-mono">
                          (default)
                        </span>
                      )}
                      <button
                        type="button"
                        onClick={() => handleDeleteDictionaryWord(word)}
                        className="opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-destructive transition-opacity ml-1"
                        title={`Remove ${word}`}
                      >
                        ×
                      </button>
                    </div>
                  );
                })}
              </div>
            </div>
          )}
        </div>
      )}

      {/* 2. SNIPPETS TAB */}
      {activeTab === 'snippets' && (
        <div className="space-y-5 animate-in fade-in-50">
          {/* Quick Add Trigger Input */}
          <form onSubmit={handleQuickAddSnippet} className="relative flex items-center gap-2">
            <div className="relative flex-1">
              <Input
                value={snippetTriggerInput}
                onChange={(e) => setSnippetTriggerInput(e.target.value)}
                placeholder='Add a trigger phrase — "investor ask", "cal link"'
                className="h-10 text-xs pl-3 pr-16 bg-muted/30 border-border focus:bg-background"
              />
              <span className="absolute right-3 top-1/2 -translate-y-1/2 text-[10px] text-muted-foreground font-mono">
                Press Enter ↵
              </span>
            </div>
            <Button type="submit" size="sm" className="h-10 px-4 text-xs gap-1.5" disabled={!snippetTriggerInput.trim()}>
              <Plus className="w-3.5 h-3.5" />
              <span>Add Snippet</span>
            </Button>
          </form>

          {/* Showcase Banner (OpenWhispr Style) */}
          <div className="p-4 rounded-lg bg-gradient-to-r from-blue-500/10 via-primary/5 to-transparent border border-blue-500/20 flex flex-col md:flex-row items-start md:items-center justify-between gap-4">
            <div className="space-y-1">
              <h3 className="text-sm font-bold text-foreground">The stuff you shouldn't have to say twice</h3>
              <p className="text-xs text-muted-foreground max-w-xl leading-relaxed">
                Speak a trigger phrase during dictation and Relay automatically replaces it with whatever you saved — URLs, intros, sign-offs, and complex prompts.
              </p>
            </div>
            <Button
              type="button"
              size="sm"
              variant="default"
              onClick={() => {
                setNewSnippetTitle('');
                setNewSnippetTrigger('');
                setNewSnippetText('');
                setNewSnippetModalOpen(true);
              }}
              className="gap-1.5 shrink-0 text-xs h-8"
            >
              <Plus className="w-3.5 h-3.5" />
              <span>New Snippet</span>
            </Button>
          </div>

          {/* Snippets List */}
          <div className="space-y-3">
            {snippets.length === 0 ? (
              <div className="text-center py-12 px-4 rounded-lg border border-dashed border-border bg-muted/10 flex flex-col items-center justify-center space-y-2">
                <Sparkles className="w-6 h-6 text-muted-foreground/50 mb-1" />
                <p className="text-xs font-semibold text-foreground">No snippets configured yet</p>
                <p className="text-[11px] text-muted-foreground">Add your first voice expansion snippet above</p>
              </div>
            ) : (
              snippets.map((snip) => (
                <div
                  key={snip.id}
                  className={`p-3.5 rounded-lg border transition-all ${
                    snip.enabled
                      ? 'bg-card border-border hover:border-primary/40 shadow-xs'
                      : 'bg-muted/20 border-border/50 opacity-60'
                  }`}
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="space-y-1.5 flex-1 min-w-0">
                      <div className="flex items-center gap-2 flex-wrap">
                        <Badge
                          variant="secondary"
                          className="font-semibold text-xs px-2.5 py-0.5 bg-blue-500/10 text-blue-600 dark:text-blue-400 border border-blue-500/20"
                        >
                          "{snip.trigger}"
                        </Badge>
                        <span className="text-xs text-muted-foreground">→</span>
                        {snip.label && snip.label !== snip.trigger && (
                          <span className="text-xs font-medium text-foreground">{snip.label}</span>
                        )}
                      </div>
                      <p className="text-xs text-muted-foreground font-mono bg-muted/40 p-2 rounded-md whitespace-pre-wrap line-clamp-3">
                        {snip.snippet_text}
                      </p>
                    </div>

                    <div className="flex items-center gap-2 shrink-0">
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        className="h-7 w-7 text-muted-foreground hover:text-foreground"
                        onClick={() => handleCopySnippetText(snip.id, snip.snippet_text)}
                        title="Copy expansion text"
                      >
                        {copiedSnippetId === snip.id ? (
                          <Check className="w-3.5 h-3.5 text-emerald-500" />
                        ) : (
                          <Copy className="w-3.5 h-3.5" />
                        )}
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        className="h-7 w-7 text-muted-foreground hover:text-foreground"
                        onClick={() => setEditingSnippet(snip)}
                        title="Edit snippet"
                      >
                        <Edit3 className="w-3.5 h-3.5" />
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        className="h-7 w-7 text-muted-foreground hover:text-destructive"
                        onClick={() => handleDeleteSnippet(snip.id)}
                        title="Delete snippet"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </Button>
                      <Switch
                        checked={snip.enabled}
                        onCheckedChange={(checked) => handleToggleSnippet(snip.id, checked)}
                      />
                    </div>
                  </div>
                </div>
              ))
            )}
          </div>

          {/* Modal: New Snippet */}
          {newSnippetModalOpen && (
            <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm p-4 animate-in fade-in-50">
              <div className="w-full max-w-lg bg-card border border-border rounded-lg p-6 shadow-2xl space-y-4">
                <div className="space-y-1">
                  <h3 className="text-sm font-bold text-foreground">Create Spoken Snippet</h3>
                  <p className="text-xs text-muted-foreground">
                    Define what you say, and what Relay expands it to when you speak.
                  </p>
                </div>

                <form onSubmit={handleSaveNewSnippet} className="space-y-3">
                  <div>
                    <label className="block text-xs font-medium text-muted-foreground mb-1">
                      Trigger Phrase (What you speak)
                    </label>
                    <Input
                      value={newSnippetTrigger}
                      onChange={(e) => setNewSnippetTrigger(e.target.value)}
                      placeholder='e.g. "my linkedin" or "cal link"'
                      className="h-8 text-xs"
                      required
                    />
                  </div>

                  <div>
                    <label className="block text-xs font-medium text-muted-foreground mb-1">
                      Label / Title (Optional)
                    </label>
                    <Input
                      value={newSnippetTitle}
                      onChange={(e) => setNewSnippetTitle(e.target.value)}
                      placeholder='e.g. "LinkedIn Profile URL"'
                      className="h-8 text-xs"
                    />
                  </div>

                  <div>
                    <label className="block text-xs font-medium text-muted-foreground mb-1">
                      Expansion Text (What gets typed / inserted)
                    </label>
                    <textarea
                      value={newSnippetText}
                      onChange={(e) => setNewSnippetText(e.target.value)}
                      placeholder="e.g. https://linkedin.com/in/yourname"
                      rows={4}
                      className="w-full bg-background border border-input rounded-lg p-2.5 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-ring font-mono"
                      required
                    />
                  </div>

                  <div className="flex justify-end gap-2 pt-2 border-t border-border">
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      className="text-xs h-8"
                      onClick={() => setNewSnippetModalOpen(false)}
                    >
                      Cancel
                    </Button>
                    <Button type="submit" size="sm" className="text-xs h-8">
                      Save Snippet
                    </Button>
                  </div>
                </form>
              </div>
            </div>
          )}

          {/* Modal: Edit Snippet */}
          {editingSnippet && (
            <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm p-4 animate-in fade-in-50">
              <div className="w-full max-w-lg bg-card border border-border rounded-lg p-6 shadow-2xl space-y-4">
                <div className="space-y-1">
                  <h3 className="text-sm font-bold text-foreground">Edit Snippet</h3>
                  <p className="text-xs text-muted-foreground">Modify trigger phrase or expansion text.</p>
                </div>

                <form onSubmit={handleUpdateEditingSnippet} className="space-y-3">
                  <div>
                    <label className="block text-xs font-medium text-muted-foreground mb-1">
                      Trigger Phrase
                    </label>
                    <Input
                      value={editingSnippet.trigger}
                      onChange={(e) =>
                        setEditingSnippet({ ...editingSnippet, trigger: e.target.value })
                      }
                      className="h-8 text-xs"
                      required
                    />
                  </div>

                  <div>
                    <label className="block text-xs font-medium text-muted-foreground mb-1">
                      Label / Title
                    </label>
                    <Input
                      value={editingSnippet.label || ''}
                      onChange={(e) =>
                        setEditingSnippet({ ...editingSnippet, label: e.target.value })
                      }
                      className="h-8 text-xs"
                    />
                  </div>

                  <div>
                    <label className="block text-xs font-medium text-muted-foreground mb-1">
                      Expansion Text
                    </label>
                    <textarea
                      value={editingSnippet.snippet_text}
                      onChange={(e) =>
                        setEditingSnippet({ ...editingSnippet, snippet_text: e.target.value })
                      }
                      rows={4}
                      className="w-full bg-background border border-input rounded-lg p-2.5 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-ring font-mono"
                      required
                    />
                  </div>

                  <div className="flex justify-end gap-2 pt-2 border-t border-border">
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      className="text-xs h-8"
                      onClick={() => setEditingSnippet(null)}
                    >
                      Cancel
                    </Button>
                    <Button type="submit" size="sm" className="text-xs h-8">
                      Update Snippet
                    </Button>
                  </div>
                </form>
              </div>
            </div>
          )}
        </div>
      )}

      {/* 3. MCP TRIGGERS TAB */}
      {activeTab === 'triggers' && (
        <div className="space-y-4 animate-in fade-in-50">
          <TriggerSettings />
        </div>
      )}
    </div>
  );
};
