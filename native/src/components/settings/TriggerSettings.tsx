import React, { useState, useEffect } from 'react';
import { TriggerConfig } from '../../types';
import { invoke } from '@tauri-apps/api/core';
import { Zap, Plus, Trash2 } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';

export const TriggerSettings: React.FC = () => {
  const [triggers, setTriggers] = useState<TriggerConfig[]>([]);
  const [newPhrase, setNewPhrase] = useState('');
  const [newActionType, setNewActionType] = useState<TriggerConfig['action_type']>('mcp_calendar');
  const [newTargetTool, setNewTargetTool] = useState('google_calendar_create_event');
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState('');

  const loadTriggers = async () => {
    try {
      const data = await invoke<TriggerConfig[]>('get_triggers');
      setTriggers(data);
    } catch (err) {
      console.error('Failed to load triggers', err);
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
      setSaving(true);
      await invoke('save_triggers', { triggers: updated });
      setTriggers(updated);
      setMessage('Trigger phrases saved!');
      setTimeout(() => setMessage(''), 2000);
    } catch (err) {
      console.error('Failed to save triggers', err);
      setMessage('Failed to save triggers');
    } finally {
      setSaving(false);
    }
  };

  return (
    <Card className="h-full flex flex-col border-slate-800">
      <CardHeader className="flex-row items-center justify-between pb-3 space-y-0">
        <div className="flex items-center gap-2">
          <Zap className="w-5 h-5 text-amber-400" />
          <div>
            <CardTitle>User-Configurable Trigger Phrases</CardTitle>
            <CardDescription>
              Define custom spoken phrases that trigger automated system state & MCP actions
            </CardDescription>
          </div>
        </div>
        {message && (
          <Badge variant="emerald" className="px-2.5 py-1">
            {message}
          </Badge>
        )}
      </CardHeader>

      <CardContent className="flex-1 flex flex-col space-y-4 overflow-hidden">
        {/* Add New Trigger Form */}
        <form onSubmit={handleAddTrigger} className="bg-slate-950 rounded-lg p-3.5 border border-slate-800 space-y-3">
          <div className="grid grid-cols-3 gap-3">
            <div>
              <label className="block text-[11px] font-medium text-slate-400 mb-1">Trigger Phrase</label>
              <Input
                type="text"
                placeholder="e.g. schedule team sync"
                value={newPhrase}
                onChange={(e) => setNewPhrase(e.target.value)}
              />
            </div>

            <div>
              <label className="block text-[11px] font-medium text-slate-400 mb-1">Action Type</label>
              <select
                value={newActionType}
                onChange={(e) => setNewActionType(e.target.value as TriggerConfig['action_type'])}
                className="w-full h-9 bg-slate-950 border border-slate-800 rounded-lg px-3 py-1 text-xs text-slate-100 focus:outline-none focus:border-blue-500"
              >
                <option value="mcp_calendar">Google Calendar (MCP)</option>
                <option value="local_reminder">Local OS Reminder</option>
                <option value="mcp_notion">Push to Notion (MCP)</option>
                <option value="mcp_gdrive">Save to Google Drive (MCP)</option>
              </select>
            </div>

            <div>
              <label className="block text-[11px] font-medium text-slate-400 mb-1">Target Tool</label>
              <Input
                type="text"
                value={newTargetTool}
                onChange={(e) => setNewTargetTool(e.target.value)}
              />
            </div>
          </div>

          <Button type="submit" size="sm" variant="default" className="self-end gap-1.5">
            <Plus className="w-3.5 h-3.5" />
            Add Trigger Mapping
          </Button>
        </form>

        {/* Trigger Phrase Mappings List */}
        <div className="flex-1 overflow-y-auto space-y-2.5 pr-1">
          {triggers.length === 0 ? (
            <p className="text-center py-6 text-slate-500 text-xs italic">No trigger phrases configured yet.</p>
          ) : (
            triggers.map((trig) => (
              <div
                key={trig.id}
                className="bg-slate-950/80 rounded-lg p-3 border border-slate-800 flex items-center justify-between gap-3"
              >
                <div className="flex items-center gap-3">
                  <input
                    type="checkbox"
                    checked={trig.enabled}
                    onChange={() => handleToggleTrigger(trig.id)}
                    className="rounded border-slate-700 bg-slate-950 text-blue-600 focus:ring-0"
                  />
                  <div>
                    <div className="flex items-center gap-2">
                      <span className="font-semibold text-xs text-slate-200">"{trig.phrase}"</span>
                      <Badge variant="secondary" className="font-mono text-[10px]">
                        {trig.action_type}
                      </Badge>
                    </div>
                    <span className="text-[11px] text-slate-400">Tool: {trig.target_tool}</span>
                  </div>
                </div>

                <Button
                  size="icon"
                  variant="ghost"
                  onClick={() => handleDeleteTrigger(trig.id)}
                  className="h-7 w-7 text-slate-500 hover:text-red-400"
                >
                  <Trash2 className="w-4 h-4" />
                </Button>
              </div>
            ))
          )}
        </div>
      </CardContent>
    </Card>
  );
};
