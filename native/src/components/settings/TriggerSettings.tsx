import React, { useState, useEffect } from 'react';
import { TriggerConfig } from '../../types';
import { invoke } from '@tauri-apps/api/core';
import { Zap, Plus, Trash2, Layers } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Skeleton } from '@/components/ui/skeleton';

export const TriggerSettings: React.FC = () => {
  const [triggers, setTriggers] = useState<TriggerConfig[]>([]);
  const [newPhrase, setNewPhrase] = useState('');
  const [newActionType, setNewActionType] = useState<TriggerConfig['action_type']>('mcp_calendar');
  const [newTargetTool, setNewTargetTool] = useState('google_calendar_create_event');
  const [isLoading, setIsLoading] = useState(true);
  const [message, setMessage] = useState('');

  const loadTriggers = async () => {
    try {
      setIsLoading(true);
      const data = await invoke<TriggerConfig[]>('get_triggers');
      setTriggers(data);
    } catch (err) {
      console.error('Failed to load triggers', err);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadTriggers();
  }, []);

  const handleAddTrigger = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newPhrase.trim()) return;

    const newTrigger: TriggerConfig = {
      id: `trig_${Date.now()}`,
      phrase: newPhrase.trim().toLowerCase(),
      action_type: newActionType,
      target_tool: newTargetTool,
      parameters: {},
      enabled: true,
    };

    const updated = [...triggers, newTrigger];
    await saveTriggers(updated);
    setNewPhrase('');
  };

  const handleDeleteTrigger = async (id: string) => {
    const updated = triggers.filter((t) => t.id !== id);
    await saveTriggers(updated);
  };

  const handleToggleTrigger = async (id: string) => {
    const updated = triggers.map((t) => (t.id === id ? { ...t, enabled: !t.enabled } : t));
    await saveTriggers(updated);
  };

  const saveTriggers = async (updated: TriggerConfig[]) => {
    try {
      await invoke('save_triggers', { triggers: updated });
      setTriggers(updated);
      setMessage('Triggers saved!');
      setTimeout(() => setMessage(''), 2000);
    } catch (err) {
      console.error('Failed to save triggers', err);
      setMessage('Failed to save triggers');
    }
  };

  return (
    <div className="space-y-4 pt-2">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Zap className="w-4 h-4 text-amber-500" />
          <p className="text-xs font-semibold text-foreground">User-Configurable Trigger Phrases</p>
        </div>
        {message && (
          <Badge variant="emerald" className="text-[10px] px-2 py-0.5 font-mono">
            {message}
          </Badge>
        )}
      </div>
      <p className="text-[11px] text-muted-foreground">
        Define custom spoken phrases that trigger automated system state & MCP actions when dictated.
      </p>

      {/* Add New Trigger Form */}
      <form onSubmit={handleAddTrigger} className="bg-muted/40 rounded-lg p-3 border border-border space-y-3">
        <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
          <div>
            <label htmlFor="trigger-phrase-input" className="block text-[11px] font-medium text-muted-foreground mb-1">
              Trigger Phrase
            </label>
            <Input
              id="trigger-phrase-input"
              type="text"
              placeholder="e.g. schedule team sync"
              value={newPhrase}
              onChange={(e) => setNewPhrase(e.target.value)}
              className="h-8 text-xs"
            />
          </div>

          <div>
            <label htmlFor="trigger-action-select" className="block text-[11px] font-medium text-muted-foreground mb-1">
              Action Type
            </label>
            <select
              id="trigger-action-select"
              value={newActionType}
              onChange={(e) => setNewActionType(e.target.value as TriggerConfig['action_type'])}
              className="w-full h-8 bg-background border border-border rounded-lg px-2.5 py-1 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
            >
              <option value="mcp_calendar">Google Calendar (MCP)</option>
              <option value="local_reminder">Local OS Reminder</option>
              <option value="mcp_notion">Push to Notion (MCP)</option>
              <option value="mcp_gdrive">Save to Google Drive (MCP)</option>
            </select>
          </div>

          <div>
            <label htmlFor="trigger-tool-input" className="block text-[11px] font-medium text-muted-foreground mb-1">
              Target Tool
            </label>
            <Input
              id="trigger-tool-input"
              type="text"
              value={newTargetTool}
              onChange={(e) => setNewTargetTool(e.target.value)}
              className="h-8 text-xs"
            />
          </div>
        </div>

        <div className="flex justify-end">
          <Button type="submit" size="sm" variant="default" className="gap-1.5 h-7 text-xs">
            <Plus className="w-3.5 h-3.5" />
            Add Trigger Mapping
          </Button>
        </div>
      </form>

      {/* Trigger Phrase Mappings List */}
      <div className="space-y-2">
        {isLoading ? (
          <div className="space-y-2">
            <Skeleton className="h-10 w-full rounded-lg" />
            <Skeleton className="h-10 w-full rounded-lg" />
          </div>
        ) : triggers.length === 0 ? (
          <div className="text-center py-6 px-4 rounded-lg border border-dashed border-border bg-muted/20 flex flex-col items-center justify-center">
            <Layers className="w-6 h-6 text-muted-foreground/40 mb-1.5" />
            <p className="text-xs font-medium text-muted-foreground">No trigger phrases configured yet</p>
            <p className="text-[10px] text-muted-foreground/70 mt-0.5">Add spoken shortcuts using the form above</p>
          </div>
        ) : (
          triggers.map((trig) => (
            <div
              key={trig.id}
              className="p-2.5 bg-card hover:bg-accent/30 transition-all rounded-lg border border-border flex items-center justify-between gap-3 shadow-xs"
            >
              <div className="flex items-center gap-3">
                <input
                  type="checkbox"
                  id={`trig-toggle-${trig.id}`}
                  checked={trig.enabled}
                  onChange={() => handleToggleTrigger(trig.id)}
                  aria-label={`Toggle trigger phrase ${trig.phrase}`}
                  className="rounded border-border bg-background text-primary focus:ring-ring h-4 w-4 cursor-pointer"
                />
                <div>
                  <div className="flex items-center gap-2">
                    <label htmlFor={`trig-toggle-${trig.id}`} className="font-semibold text-xs text-foreground cursor-pointer">
                      "{trig.phrase}"
                    </label>
                    <Badge variant="secondary" className="font-mono text-[9px] px-1.5 py-0">
                      {trig.action_type}
                    </Badge>
                  </div>
                  <span className="text-[10px] text-muted-foreground">Tool: {trig.target_tool}</span>
                </div>
              </div>

              <Button
                size="icon"
                variant="ghost"
                onClick={() => handleDeleteTrigger(trig.id)}
                aria-label={`Delete trigger phrase ${trig.phrase}`}
                className="h-7 w-7 text-muted-foreground hover:text-destructive"
              >
                <Trash2 className="w-3.5 h-3.5" />
              </Button>
            </div>
          ))
        )}
      </div>
    </div>
  );
};
