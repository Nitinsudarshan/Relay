import React from "react";
import { getSupabaseClient } from "@/lib/supabase/client";
import { Mic, Kanban, Cloud, ShieldCheck, CheckCircle2, Clock, Zap } from "lucide-react";

export default async function DashboardPage() {
  const supabase = getSupabaseClient();
  const { data: cards } = await supabase.getKanbanCards();

  const todoCards = cards?.filter((c) => c.status === "todo") || [];
  const inProgressCards = cards?.filter((c) => c.status === "in_progress") || [];
  const doneCards = cards?.filter((c) => c.status === "done") || [];

  return (
    <div className="flex flex-1 flex-col gap-6 p-4 md:p-6 lg:p-8 max-w-7xl mx-auto w-full">
      {/* Header Banner */}
      <div className="bg-gradient-to-r from-slate-900 via-zinc-900 to-slate-900 border border-zinc-800 p-6 rounded-2xl shadow-xl flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <div className="flex items-center gap-2.5 mb-1">
            <span className="bg-blue-500/20 text-blue-400 border border-blue-500/30 text-xs font-semibold px-2.5 py-0.5 rounded-full flex items-center gap-1">
              <Cloud className="w-3 h-3" /> Hybrid Cloud Mode
            </span>
            <span className="bg-emerald-500/20 text-emerald-400 border border-emerald-500/30 text-xs font-semibold px-2.5 py-0.5 rounded-full flex items-center gap-1">
              <ShieldCheck className="w-3 h-3" /> $0 Free Baseline
            </span>
          </div>
          <h1 className="text-2xl font-bold text-slate-100">Relay — AI Voice & Memory Dashboard</h1>
          <p className="text-sm text-slate-400 mt-1">
            Synced state across Windows Desktop capture & Web surface
          </p>
        </div>

        <div className="flex items-center gap-3">
          <div className="bg-zinc-950 border border-zinc-800 rounded-xl p-3 text-right">
            <div className="text-xs text-zinc-400">Synced Action Cards</div>
            <div className="text-xl font-bold text-slate-100">{cards?.length || 0}</div>
          </div>
        </div>
      </div>

      {/* Synced Kanban Cards View */}
      <div>
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-bold text-slate-100 flex items-center gap-2">
            <Kanban className="w-5 h-5 text-blue-400" />
            <span>Hybrid Synced Kanban Board</span>
          </h2>
          <span className="text-xs text-slate-400 font-mono">Synced with Supabase Cloud DB</span>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          {/* To Do */}
          <div className="bg-zinc-900/60 border border-zinc-800 rounded-xl p-4 flex flex-col">
            <div className="flex items-center justify-between pb-3 mb-3 border-b border-zinc-800 font-semibold text-sm text-slate-200">
              <span>To Do</span>
              <span className="bg-blue-500/20 text-blue-400 text-xs px-2 py-0.5 rounded-full">
                {todoCards.length}
              </span>
            </div>
            <div className="space-y-3">
              {todoCards.map((card) => (
                <div key={card.id} className="bg-zinc-950 border border-zinc-800 p-3.5 rounded-lg">
                  <div className="font-medium text-sm text-slate-100">{card.title}</div>
                  <p className="text-xs text-slate-400 mt-1">{card.description}</p>
                  <div className="flex items-center justify-between text-[11px] text-zinc-500 mt-3 pt-2 border-t border-zinc-900">
                    <span>Assignee: {card.assignee}</span>
                    <span className="uppercase text-[10px] font-semibold text-amber-400">{card.priority}</span>
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* In Progress */}
          <div className="bg-zinc-900/60 border border-zinc-800 rounded-xl p-4 flex flex-col">
            <div className="flex items-center justify-between pb-3 mb-3 border-b border-zinc-800 font-semibold text-sm text-slate-200">
              <span>In Progress</span>
              <span className="bg-amber-500/20 text-amber-400 text-xs px-2 py-0.5 rounded-full">
                {inProgressCards.length}
              </span>
            </div>
            <div className="space-y-3">
              {inProgressCards.map((card) => (
                <div key={card.id} className="bg-zinc-950 border border-zinc-800 p-3.5 rounded-lg">
                  <div className="font-medium text-sm text-slate-100">{card.title}</div>
                  <p className="text-xs text-slate-400 mt-1">{card.description}</p>
                  <div className="flex items-center justify-between text-[11px] text-zinc-500 mt-3 pt-2 border-t border-zinc-900">
                    <span>Assignee: {card.assignee}</span>
                    <span className="uppercase text-[10px] font-semibold text-red-400">{card.priority}</span>
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* Done */}
          <div className="bg-zinc-900/60 border border-zinc-800 rounded-xl p-4 flex flex-col">
            <div className="flex items-center justify-between pb-3 mb-3 border-b border-zinc-800 font-semibold text-sm text-slate-200">
              <span>Done</span>
              <span className="bg-emerald-500/20 text-emerald-400 text-xs px-2 py-0.5 rounded-full">
                {doneCards.length}
              </span>
            </div>
            <div className="space-y-3">
              {doneCards.length === 0 ? (
                <p className="text-xs text-slate-500 italic py-4 text-center">No completed tasks yet.</p>
              ) : (
                doneCards.map((card) => (
                  <div key={card.id} className="bg-zinc-950 border border-zinc-800 p-3.5 rounded-lg">
                    <div className="font-medium text-sm text-slate-100">{card.title}</div>
                    <p className="text-xs text-slate-400 mt-1">{card.description}</p>
                  </div>
                ))
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
