import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  Sparkles,
  Plus,
  Search,
  Trash2,
  Edit3,
  Copy,
  Check,
  ToggleLeft,
  ToggleRight,
  Info,
  X,
  FileCode,
  Wand2,
  Layers,
  ArrowUpRight,
  Play,
} from 'lucide-react';
import { AppSettings, PromptItem } from '../../types';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Switch } from '@/components/ui/switch';
import { cn } from '@/lib/utils';
import { PromptTransformModal } from './PromptTransformModal';

export const PromptsPage: React.FC = () => {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [prompts, setPrompts] = useState<PromptItem[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [testPrompt, setTestPrompt] = useState<PromptItem | null>(null);

  // Modal State for Create / Edit
  const [modalOpen, setModalOpen] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [formName, setFormName] = useState('');
  const [formDescription, setFormDescription] = useState('');
  const [formBody, setFormBody] = useState('');
  const [formEnabled, setFormEnabled] = useState(true);
  const [formError, setFormError] = useState<string | null>(null);

  // Delete confirmation
  const [deletingId, setDeletingId] = useState<string | null>(null);

  const loadSettingsAndPrompts = async () => {
    try {
      const appSettings = await invoke<AppSettings>('get_settings');
      setSettings(appSettings);
      setPrompts(appSettings.prompts || []);
    } catch (err) {
      console.error('Failed to load settings in PromptsPage:', err);
    }
  };

  useEffect(() => {
    loadSettingsAndPrompts();

    const unlistenPromise = listen<AppSettings>('settings-changed', ({ payload }) => {
      if (payload) {
        setSettings(payload);
        setPrompts(payload.prompts || []);
      }
    });

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  const saveUpdatedPrompts = async (updatedPrompts: PromptItem[]) => {
    setPrompts(updatedPrompts);
    let current = settings;
    if (!current) {
      try {
        current = await invoke<AppSettings>('get_settings');
      } catch {
        return;
      }
    }

    const updatedSettings: AppSettings = {
      ...current,
      prompts: updatedPrompts,
    };

    setSettings(updatedSettings);
    try {
      await invoke('save_settings', { settings: updatedSettings });
    } catch (err) {
      console.error('Failed to persist prompts:', err);
    }
  };

  const handleOpenCreateModal = () => {
    setIsEditing(false);
    setEditingId(null);
    setFormName('');
    setFormDescription('');
    setFormBody('Transform the following dictated text into:\n\n{{text}}');
    setFormEnabled(true);
    setFormError(null);
    setModalOpen(true);
  };

  const handleOpenEditModal = (prompt: PromptItem) => {
    setIsEditing(true);
    setEditingId(prompt.id);
    setFormName(prompt.name);
    setFormDescription(prompt.description || '');
    setFormBody(prompt.prompt_body);
    setFormEnabled(prompt.enabled);
    setFormError(null);
    setModalOpen(true);
  };

  const handleSaveModal = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!formName.trim()) {
      setFormError('Please provide a prompt name.');
      return;
    }
    if (!formBody.trim()) {
      setFormError('Please provide prompt instructions or template body.');
      return;
    }

    if (isEditing && editingId) {
      const updated = prompts.map((p) =>
        p.id === editingId
          ? {
              ...p,
              name: formName.trim(),
              description: formDescription.trim() || null,
              prompt_body: formBody.trim(),
              enabled: formEnabled,
            }
          : p
      );
      await saveUpdatedPrompts(updated);
    } else {
      const newPrompt: PromptItem = {
        id: `prompt_${Date.now()}_${Math.random().toString(36).substring(2, 7)}`,
        name: formName.trim(),
        description: formDescription.trim() || null,
        prompt_body: formBody.trim(),
        enabled: formEnabled,
      };
      await saveUpdatedPrompts([newPrompt, ...prompts]);
    }

    setModalOpen(false);
  };

  const handleToggleEnabled = async (id: string) => {
    const updated = prompts.map((p) =>
      p.id === id ? { ...p, enabled: !p.enabled } : p
    );
    await saveUpdatedPrompts(updated);
  };

  const handleDelete = async (id: string) => {
    const updated = prompts.filter((p) => p.id !== id);
    await saveUpdatedPrompts(updated);
    setDeletingId(null);
  };

  const handleDuplicate = async (prompt: PromptItem) => {
    const duplicatePrompt: PromptItem = {
      id: `prompt_${Date.now()}_${Math.random().toString(36).substring(2, 7)}`,
      name: `${prompt.name} (Copy)`,
      description: prompt.description,
      prompt_body: prompt.prompt_body,
      enabled: prompt.enabled,
    };
    await saveUpdatedPrompts([duplicatePrompt, ...prompts]);
  };

  const handleCopyBody = (prompt: PromptItem) => {
    navigator.clipboard.writeText(prompt.prompt_body).catch(() => {});
    setCopiedId(prompt.id);
    setTimeout(() => setCopiedId(null), 2000);
  };

  const filteredPrompts = prompts.filter((p) => {
    const query = searchQuery.toLowerCase().trim();
    if (!query) return true;
    return (
      p.name.toLowerCase().includes(query) ||
      (p.description && p.description.toLowerCase().includes(query)) ||
      p.prompt_body.toLowerCase().includes(query)
    );
  });

  const activeCount = prompts.filter((p) => p.enabled).length;

  return (
    <div className="space-y-6 max-w-6xl pb-16">
      {/* Top Controls & Search Bar */}
      <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4">
        <div className="flex items-center gap-3">
          <div className="relative flex-1 sm:w-80">
            <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
            <Input
              type="text"
              placeholder="Search prompts or instructions..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-9 bg-card border-border/80 text-xs h-9"
            />
          </div>
          {searchQuery && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setSearchQuery('')}
              className="h-8 text-xs text-muted-foreground hover:text-foreground"
            >
              Clear
            </Button>
          )}
        </div>

        <div className="flex items-center gap-3">
          <div className="text-xs text-muted-foreground font-mono hidden md:flex items-center gap-2">
            <span>{activeCount} of {prompts.length} active</span>
          </div>
          <Button
            onClick={handleOpenCreateModal}
            className="h-9 px-3.5 text-xs font-semibold gap-1.5 bg-primary text-primary-foreground shadow-xs hover:bg-primary/90"
          >
            <Plus className="w-4 h-4" />
            <span>Create Prompt</span>
          </Button>
        </div>
      </div>

      {/* Prompts Grid */}
      {filteredPrompts.length === 0 ? (
        <div className="rounded-xl border border-dashed border-border/80 bg-card/40 p-12 text-center space-y-3">
          <div className="w-10 h-10 rounded-full bg-primary/10 text-primary flex items-center justify-center mx-auto">
            <Wand2 className="w-5 h-5" />
          </div>
          <h3 className="text-sm font-semibold text-foreground">
            {searchQuery ? 'No prompts matched your search' : 'No custom prompts saved yet'}
          </h3>
          <p className="text-xs text-muted-foreground max-w-sm mx-auto">
            {searchQuery
              ? 'Try searching with different keywords or clear the search filter.'
              : 'Create transformation templates to summarize, rewrite, format, or extract data from dictated speech.'}
          </p>
          {!searchQuery && (
            <Button
              onClick={handleOpenCreateModal}
              variant="outline"
              size="sm"
              className="mt-2 text-xs font-medium gap-1.5"
            >
              <Plus className="w-3.5 h-3.5" />
              <span>Create your first prompt</span>
            </Button>
          )}
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {filteredPrompts.map((prompt) => {
            const isDeleting = deletingId === prompt.id;
            const hasPlaceholder = prompt.prompt_body.includes('{{text}}');

            return (
              <div
                key={prompt.id}
                className={cn(
                  'group relative flex flex-col justify-between rounded-xl border bg-card p-4 transition-all duration-200 hover:shadow-md',
                  prompt.enabled
                    ? 'border-border/90 hover:border-primary/40'
                    : 'border-border/50 opacity-70 bg-card/60'
                )}
              >
                <div className="space-y-2.5">
                  {/* Top Bar: Title & Toggle */}
                  <div className="flex items-start justify-between gap-2">
                    <div className="space-y-0.5 flex-1 min-w-0">
                      <h4 className="text-sm font-bold text-foreground truncate" title={prompt.name}>
                        {prompt.name}
                      </h4>
                      {prompt.description && (
                        <p className="text-xs text-muted-foreground line-clamp-2 leading-relaxed">
                          {prompt.description}
                        </p>
                      )}
                    </div>
                    <Switch
                      checked={prompt.enabled}
                      onCheckedChange={() => handleToggleEnabled(prompt.id)}
                      className="shrink-0 scale-90 data-[state=checked]:bg-primary"
                      title={prompt.enabled ? 'Enabled' : 'Disabled'}
                    />
                  </div>

                  {/* Body Preview */}
                  <div className="relative rounded-lg bg-muted/40 border border-border/60 p-2.5 font-mono text-[11px] text-muted-foreground max-h-28 overflow-y-auto whitespace-pre-wrap select-text leading-relaxed">
                    {prompt.prompt_body}
                  </div>

                  {/* Tags / Info */}
                  <div className="flex items-center gap-1.5 flex-wrap">
                    {hasPlaceholder ? (
                      <Badge variant="outline" className="text-[10px] font-mono border-primary/30 text-primary bg-primary/5 py-0 px-1.5">
                        {'{{text}}'} target
                      </Badge>
                    ) : (
                      <Badge variant="outline" className="text-[10px] font-mono text-muted-foreground border-border py-0 px-1.5">
                        Appended transcript
                      </Badge>
                    )}
                    {prompt.enabled && (
                      <Badge variant="outline" className="text-[10px] font-mono text-emerald-500 border-emerald-500/30 bg-emerald-500/5 py-0 px-1.5">
                        Active
                      </Badge>
                    )}
                  </div>
                </div>

                {/* Bottom Action Footer */}
                <div className="pt-3 mt-3 border-t border-border/60 flex items-center justify-between">
                  {isDeleting ? (
                    <div className="flex items-center gap-1.5 w-full justify-end animate-in fade-in duration-150">
                      <span className="text-[11px] text-rose-500 font-medium mr-1">Delete prompt?</span>
                      <Button
                        size="sm"
                        variant="destructive"
                        onClick={() => handleDelete(prompt.id)}
                        className="h-7 px-2 text-[11px]"
                      >
                        Confirm
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => setDeletingId(null)}
                        className="h-7 px-2 text-[11px]"
                      >
                        Cancel
                      </Button>
                    </div>
                  ) : (
                    <>
                      <div className="flex items-center gap-1">
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => handleCopyBody(prompt)}
                          className="h-7 w-7 text-muted-foreground hover:text-foreground"
                          title="Copy prompt text"
                        >
                          {copiedId === prompt.id ? (
                            <Check className="w-3.5 h-3.5 text-emerald-500" />
                          ) : (
                            <Copy className="w-3.5 h-3.5" />
                          )}
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => handleDuplicate(prompt)}
                          className="h-7 w-7 text-muted-foreground hover:text-foreground"
                          title="Duplicate prompt"
                        >
                          <Layers className="w-3.5 h-3.5" />
                        </Button>
                      </div>

                      <div className="flex items-center gap-1">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => setTestPrompt(prompt)}
                          className="h-7 px-2 text-xs text-sky-500 hover:text-sky-400 hover:bg-sky-500/10 gap-1"
                          title="Test prompt with sample text"
                        >
                          <Play className="w-3 h-3 fill-current" />
                          <span>Test</span>
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => handleOpenEditModal(prompt)}
                          className="h-7 px-2 text-xs text-muted-foreground hover:text-foreground gap-1"
                        >
                          <Edit3 className="w-3.5 h-3.5" />
                          <span>Edit</span>
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => setDeletingId(prompt.id)}
                          className="h-7 w-7 text-muted-foreground hover:text-rose-500"
                          title="Delete prompt"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </Button>
                      </div>
                    </>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Create / Edit Prompt Modal Overlay */}
      {modalOpen && (
        <div className="fixed inset-0 z-50 bg-black/60 backdrop-blur-xs flex items-center justify-center p-4 animate-in fade-in duration-150">
          <div className="relative w-full max-w-xl rounded-xl border border-border bg-card shadow-2xl p-6 space-y-4 animate-in zoom-in-95 duration-150">
            <div className="flex items-center justify-between border-b border-border pb-3">
              <div className="flex items-center gap-2">
                <div className="w-7 h-7 rounded-lg bg-primary/10 text-primary flex items-center justify-center">
                  <Wand2 className="w-4 h-4" />
                </div>
                <div>
                  <h3 className="text-sm font-bold text-foreground">
                    {isEditing ? 'Edit Prompt Template' : 'Create Prompt Template'}
                  </h3>
                  <p className="text-[11px] text-muted-foreground">
                    Define system instructions and formatting rules for AI processing.
                  </p>
                </div>
              </div>
              <button
                type="button"
                onClick={() => setModalOpen(false)}
                className="text-muted-foreground hover:text-foreground p-1 rounded-lg transition-colors cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <form onSubmit={handleSaveModal} className="space-y-4">
              {formError && (
                <div className="rounded-lg bg-rose-500/10 border border-rose-500/30 p-2.5 text-xs text-rose-500 flex items-center gap-2">
                  <Info className="w-4 h-4 shrink-0" />
                  <span>{formError}</span>
                </div>
              )}

              <div className="space-y-1.5">
                <label className="text-xs font-semibold text-foreground">
                  Prompt Name <span className="text-rose-500">*</span>
                </label>
                <Input
                  type="text"
                  placeholder="e.g. Summarize into Bullet Points"
                  value={formName}
                  onChange={(e) => setFormName(e.target.value)}
                  className="text-xs bg-muted/40 border-border/80 h-9"
                  autoFocus
                />
              </div>

              <div className="space-y-1.5">
                <label className="text-xs font-semibold text-foreground">
                  Description <span className="text-muted-foreground text-[10px] font-normal">(optional)</span>
                </label>
                <Input
                  type="text"
                  placeholder="e.g. Converts rambling speech into crisp, formatted bullet points."
                  value={formDescription}
                  onChange={(e) => setFormDescription(e.target.value)}
                  className="text-xs bg-muted/40 border-border/80 h-9"
                />
              </div>

              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <label className="text-xs font-semibold text-foreground">
                    Prompt Body & Instructions <span className="text-rose-500">*</span>
                  </label>
                  <span className="text-[10px] font-mono text-muted-foreground">
                    Use <code className="text-primary font-bold">{'{{text}}'}</code> as transcript placeholder
                  </span>
                </div>
                <textarea
                  rows={6}
                  placeholder="Summarize the following transcript into clear bullet points:&#10;&#10;{{text}}"
                  value={formBody}
                  onChange={(e) => setFormBody(e.target.value)}
                  className="w-full rounded-lg border border-border/80 bg-muted/40 p-3 font-mono text-xs text-foreground placeholder:text-muted-foreground/60 focus:outline-none focus:ring-1 focus:ring-primary select-text leading-relaxed"
                />
              </div>

              <div className="flex items-center justify-between pt-1">
                <div className="flex items-center gap-2">
                  <Switch
                    id="modal-enabled-switch"
                    checked={formEnabled}
                    onCheckedChange={setFormEnabled}
                    className="data-[state=checked]:bg-primary"
                  />
                  <label htmlFor="modal-enabled-switch" className="text-xs font-medium text-foreground cursor-pointer">
                    Active / Enabled
                  </label>
                </div>

                <div className="flex items-center gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() => setModalOpen(false)}
                    className="h-8 text-xs font-medium"
                  >
                    Cancel
                  </Button>
                  <Button
                    type="submit"
                    size="sm"
                    className="h-8 text-xs font-semibold bg-primary text-primary-foreground hover:bg-primary/90"
                  >
                    {isEditing ? 'Save Changes' : 'Create Prompt'}
                  </Button>
                </div>
              </div>
            </form>
          </div>
        </div>
      )}
      {/* Test / Run Prompt Modal */}
      {testPrompt && (
        <PromptTransformModal
          isOpen={Boolean(testPrompt)}
          onClose={() => setTestPrompt(null)}
          inputText="Here is an example transcript demonstrating how this prompt template transforms voice dictations into polished structured notes."
          sourceTitle={testPrompt.name}
          sourceType="voice_note"
        />
      )}
    </div>
  );
};
