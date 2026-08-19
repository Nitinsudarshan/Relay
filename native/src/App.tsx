import React, { useState, useEffect } from 'react';
import { PTTWidget } from './components/capture/PTTWidget';
import { KanbanBoard } from './components/kanban/KanbanBoard';
import { ScribbleViewer } from './components/scribble/ScribbleViewer';
import { ChatPanel } from './components/chat/ChatPanel';
import { TriggerSettings } from './components/settings/TriggerSettings';
import { ProviderSettings } from './components/settings/ProviderSettings';
import { KanbanCard, ProcessedPipelineResult } from './types';
import { invoke } from '@tauri-apps/api/core';
import { Mic, Kanban, Sparkles, Zap, Settings, ShieldCheck, Activity, Bot } from 'lucide-react';

export const App: React.FC = () => {
  const [activeTab, setActiveTab] = useState<
    'capture' | 'kanban' | 'scribble' | 'chat' | 'triggers' | 'settings'
  >('capture');
  const [cards, setCards] = useState<KanbanCard[]>([]);
  const [lastResult, setLastResult] = useState<ProcessedPipelineResult | null>(null);

  const fetchKanbanCards = async () => {
    try {
      const data = await invoke<KanbanCard[]>('get_kanban_cards');
      setCards(data);
    } catch (err) {
      console.error('Failed to fetch Kanban cards', err);
    }
  };

  useEffect(() => {
    fetchKanbanCards();
  }, []);

  const handleProcessComplete = (result: ProcessedPipelineResult) => {
    setLastResult(result);
    fetchKanbanCards();
    if (result.mode === 'scribble') {
      setActiveTab('scribble');
    } else if (result.mode === 'meeting') {
      setActiveTab('kanban');
    }
  };

  return (
    <div className="flex h-screen w-screen bg-slate-950 text-slate-100 overflow-hidden font-sans">
      {/* Navigation Sidebar */}
      <aside className="w-64 bg-slate-900/80 border-r border-slate-800 flex flex-col p-4">
        <div className="flex items-center gap-3 px-2 py-3 mb-6 border-b border-slate-800/80">
          <div className="w-9 h-9 rounded-xl bg-gradient-to-tr from-blue-600 to-indigo-600 flex items-center justify-center text-white font-bold shadow-lg shadow-blue-500/20">
            <Mic className="w-5 h-5" />
          </div>
          <div>
            <h1 className="font-bold text-base tracking-tight text-slate-100">RELAY</h1>
            <p className="text-[10px] text-slate-400 font-mono uppercase tracking-wider">AI Voice & Memory</p>
          </div>
        </div>

        <nav className="flex-1 space-y-1">
          <button
            onClick={() => setActiveTab('capture')}
            className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-xs font-medium transition-all ${
              activeTab === 'capture'
                ? 'bg-blue-600/20 text-blue-400 border border-blue-500/30'
                : 'text-slate-400 hover:bg-slate-800/60 hover:text-slate-200'
            }`}
          >
            <Mic className="w-4 h-4" />
            Voice Capture
          </button>

          <button
            onClick={() => setActiveTab('kanban')}
            className={`w-full flex items-center justify-between px-3 py-2.5 rounded-xl text-xs font-medium transition-all ${
              activeTab === 'kanban'
                ? 'bg-blue-600/20 text-blue-400 border border-blue-500/30'
                : 'text-slate-400 hover:bg-slate-800/60 hover:text-slate-200'
            }`}
          >
            <div className="flex items-center gap-3">
              <Kanban className="w-4 h-4" />
              Kanban Board
            </div>
            {cards.length > 0 && (
              <span className="text-[10px] font-bold bg-slate-800 text-slate-300 px-2 py-0.5 rounded-full">
                {cards.length}
              </span>
            )}
          </button>

          <button
            onClick={() => setActiveTab('scribble')}
            className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-xs font-medium transition-all ${
              activeTab === 'scribble'
                ? 'bg-purple-600/20 text-purple-400 border border-purple-500/30'
                : 'text-slate-400 hover:bg-slate-800/60 hover:text-slate-200'
            }`}
          >
            <Sparkles className="w-4 h-4" />
            Structured Scribbles
          </button>

          <button
            onClick={() => setActiveTab('chat')}
            className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-xs font-medium transition-all ${
              activeTab === 'chat'
                ? 'bg-blue-600/20 text-blue-400 border border-blue-500/30'
                : 'text-slate-400 hover:bg-slate-800/60 hover:text-slate-200'
            }`}
          >
            <Bot className="w-4 h-4" />
            Voice Chat
          </button>

          <button
            onClick={() => setActiveTab('triggers')}
            className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-xs font-medium transition-all ${
              activeTab === 'triggers'
                ? 'bg-amber-600/20 text-amber-400 border border-amber-500/30'
                : 'text-slate-400 hover:bg-slate-800/60 hover:text-slate-200'
            }`}
          >
            <Zap className="w-4 h-4" />
            Trigger Phrases
          </button>

          <button
            onClick={() => setActiveTab('settings')}
            className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-xs font-medium transition-all ${
              activeTab === 'settings'
                ? 'bg-slate-800 text-slate-200 border border-slate-700'
                : 'text-slate-400 hover:bg-slate-800/60 hover:text-slate-200'
            }`}
          >
            <Settings className="w-4 h-4" />
            LLM Provider Settings
          </button>
        </nav>

        {/* Footer info */}
        <div className="pt-4 border-t border-slate-800/80 text-[11px] text-slate-500 flex items-center justify-between">
          <div className="flex items-center gap-1.5">
            <ShieldCheck className="w-3.5 h-3.5 text-emerald-400" />
            <span>Local Mode ($0)</span>
          </div>
          <div className="flex items-center gap-1">
            <Activity className="w-3.5 h-3.5 text-blue-400" />
            <span>v0.1.0</span>
          </div>
        </div>
      </aside>

      {/* Main Content View */}
      <main className="flex-1 p-6 overflow-hidden flex flex-col">
        {activeTab === 'capture' && (
          <div className="flex-1 flex flex-col max-w-4xl mx-auto w-full">
            <PTTWidget onProcessComplete={handleProcessComplete} />

            {lastResult && (
              <div className="mt-4 flex-1">
                {lastResult.mode === 'scribble' ? (
                  <ScribbleViewer content={lastResult.output_markdown} transcript={lastResult.transcript} />
                ) : (
                  <div className="glass-panel rounded-xl p-4 border border-slate-800 text-xs font-mono text-slate-300">
                    <p className="font-semibold text-emerald-400 mb-1">Result Summary:</p>
                    <p>{lastResult.output_markdown}</p>
                  </div>
                )}
              </div>
            )}
          </div>
        )}

        {activeTab === 'kanban' && <KanbanBoard cards={cards} onRefresh={fetchKanbanCards} />}
        {activeTab === 'chat' && <ChatPanel />}
        {activeTab === 'scribble' && (
          <ScribbleViewer
            content={lastResult?.output_markdown || ''}
            transcript={lastResult?.transcript || ''}
          />
        )}
        {activeTab === 'triggers' && <TriggerSettings />}
        {activeTab === 'settings' && <ProviderSettings />}
      </main>
    </div>
  );
};
