import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Brain,
  Search,
  Network,
  Tag,
  Database,
  Layers,
  Sparkles,
  ShieldCheck,
  CheckCircle2,
  AlertCircle,
  RefreshCw,
  Terminal,
  FileText,
} from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  KnowledgeTelemetrySnapshot,
  RetrievalResult,
  RetrievedItem,
  ContextPack,
  CandidateMemory,
  MemoryFormationOutcome,
  ResolvedEntity,
} from '@/types';

export const KnowledgeArchitectureDiagnostics: React.FC = () => {
  const [telemetry, setTelemetry] = useState<KnowledgeTelemetrySnapshot | null>(null);
  const [loadingTelemetry, setLoadingTelemetry] = useState(false);

  // Test retrieval
  const [searchQuery, setSearchQuery] = useState('Orca coding agents');
  const [retrievalResult, setRetrievalResult] = useState<RetrievalResult | null>(null);
  const [contextPack, setContextPack] = useState<ContextPack | null>(null);
  const [testingQuery, setTestingQuery] = useState(false);

  // Test memory formation
  const [memSubject, setMemSubject] = useState('Orca evaluation');
  const [memContent, setMemContent] = useState('Evaluating Orca for parallel coding-agent workflows');
  const [memReason, setMemReason] = useState('Durable project workflow evaluation');
  const [formationOutcome, setFormationOutcome] = useState<MemoryFormationOutcome | null>(null);
  const [formingMemory, setFormingMemory] = useState(false);

  // Entity listing
  const [entities, setEntities] = useState<ResolvedEntity[]>([]);
  const [loadingEntities, setLoadingEntities] = useState(false);

  const fetchTelemetry = async () => {
    setLoadingTelemetry(true);
    try {
      const data = await invoke<KnowledgeTelemetrySnapshot>('get_knowledge_telemetry');
      setTelemetry(data);
    } catch (err) {
      console.error('Failed to fetch knowledge telemetry:', err);
    } finally {
      setLoadingTelemetry(false);
    }
  };

  const fetchEntities = async () => {
    setLoadingEntities(true);
    try {
      const data = await invoke<ResolvedEntity[]>('list_entities', { category: null });
      setEntities(data);
    } catch (err) {
      console.error('Failed to list entities:', err);
    } finally {
      setLoadingEntities(false);
    }
  };

  useEffect(() => {
    fetchTelemetry();
    fetchEntities();
  }, []);

  const handleTestRetrieval = async () => {
    if (!searchQuery.trim()) return;
    setTestingQuery(true);
    setRetrievalResult(null);
    setContextPack(null);
    try {
      const [retrieval, pack] = await Promise.all([
        invoke<RetrievalResult>('unified_retrieve', {
          request: {
            query: searchQuery,
            limit: 10,
            char_budget: 15000,
            include_evidence: true,
          },
        }),
        invoke<ContextPack>('assemble_context_pack', {
          packType: 'general',
          query: searchQuery,
          intent: 'diagnostic_query',
          charBudget: 15000,
        }),
      ]);
      setRetrievalResult(retrieval);
      setContextPack(pack);
    } catch (err) {
      console.error('Failed test retrieval:', err);
    } finally {
      setTestingQuery(false);
    }
  };

  const handleTestMemoryFormation = async () => {
    if (!memSubject.trim() || !memContent.trim()) return;
    setFormingMemory(true);
    setFormationOutcome(null);
    try {
      const candidate: CandidateMemory = {
        memory_type: 'project_context',
        subject: memSubject,
        content: memContent,
        evidence: 'Diagnostics interactive test fixture',
        source_id: 'diag-001',
        confidence: 0.95,
        reason_for_retention: memReason,
      };
      const res = await invoke<MemoryFormationOutcome>('form_memory_candidate', { candidate });
      setFormationOutcome(res);
      fetchTelemetry();
    } catch (err) {
      console.error('Failed to form memory:', err);
    } finally {
      setFormingMemory(false);
    }
  };

  return (
    <div className="space-y-6">
      {/* Telemetry Overview Cards */}
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-semibold text-foreground flex items-center gap-2">
            <Brain className="w-4 h-4 text-primary" />
            Knowledge Architecture (11–20 Ultimate)
          </h3>
          <p className="text-xs text-muted-foreground mt-0.5">
            Unified retrieval, persistent entities, operational relationships, deliberate memory formation & canonical context packs.
          </p>
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={() => {
            fetchTelemetry();
            fetchEntities();
          }}
          disabled={loadingTelemetry}
          className="h-8 gap-1.5 text-xs"
        >
          <RefreshCw className={`w-3.5 h-3.5 ${loadingTelemetry ? 'animate-spin' : ''}`} />
          Refresh Stats
        </Button>
      </div>

      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <div className="p-3 rounded-lg border border-border bg-card/60">
          <div className="flex items-center justify-between text-muted-foreground mb-1">
            <span className="text-[11px] font-semibold flex items-center gap-1.5">
              <Brain className="w-3.5 h-3.5 text-indigo-400" />
              Active Memories
            </span>
            <Badge variant="secondary" className="text-[9px] px-1 py-0">Epistemic</Badge>
          </div>
          <p className="text-xl font-bold font-mono text-foreground">
            {telemetry?.active_memories ?? 0}
          </p>
          <p className="text-[10px] text-muted-foreground mt-0.5">
            Superseded / Deduplicated managed
          </p>
        </div>

        <div className="p-3 rounded-lg border border-border bg-card/60">
          <div className="flex items-center justify-between text-muted-foreground mb-1">
            <span className="text-[11px] font-semibold flex items-center gap-1.5">
              <Tag className="w-3.5 h-3.5 text-emerald-400" />
              Resolved Entities
            </span>
            <Badge variant="secondary" className="text-[9px] px-1 py-0">Safe Merge</Badge>
          </div>
          <p className="text-xl font-bold font-mono text-foreground">
            {telemetry?.total_entities ?? 0}
          </p>
          <p className="text-[10px] text-muted-foreground mt-0.5">
            {entities.length} canonical entities
          </p>
        </div>

        <div className="p-3 rounded-lg border border-border bg-card/60">
          <div className="flex items-center justify-between text-muted-foreground mb-1">
            <span className="text-[11px] font-semibold flex items-center gap-1.5">
              <Network className="w-3.5 h-3.5 text-sky-400" />
              Relationships
            </span>
            <Badge variant="secondary" className="text-[9px] px-1 py-0">Operational</Badge>
          </div>
          <p className="text-xl font-bold font-mono text-foreground">
            {telemetry?.total_relationships ?? 0}
          </p>
          <p className="text-[10px] text-muted-foreground mt-0.5">
            Auto-linked on derivation
          </p>
        </div>

        <div className="p-3 rounded-lg border border-border bg-card/60">
          <div className="flex items-center justify-between text-muted-foreground mb-1">
            <span className="text-[11px] font-semibold flex items-center gap-1.5">
              <Database className="w-3.5 h-3.5 text-purple-400" />
              Vault Sources
            </span>
            <Badge variant="secondary" className="text-[9px] px-1 py-0">Local Vault</Badge>
          </div>
          <p className="text-xl font-bold font-mono text-foreground">
            {(telemetry?.total_captures ?? 0) + (telemetry?.total_scribbles ?? 0) + (telemetry?.total_notes ?? 0)}
          </p>
          <p className="text-[10px] text-muted-foreground mt-0.5">
            {telemetry?.total_captures ?? 0} caps · {telemetry?.total_notes ?? 0} notes · {telemetry?.total_scribbles ?? 0} scribbles
          </p>
        </div>
      </div>

      {/* Interactive Unified Retrieval & Context Assembly Tester */}
      <div className="p-4 rounded-xl border border-border bg-card/40 space-y-4">
        <div>
          <h4 className="text-xs font-semibold text-foreground flex items-center gap-2">
            <Search className="w-3.5 h-3.5 text-primary" />
            Unified Retrieval & Context Pack Assembly Tester
          </h4>
          <p className="text-[11px] text-muted-foreground mt-0.5">
            Executes unified retrieval with multi-signal ranking, explainability, and context assembly with prompt boundary isolation.
          </p>
        </div>

        <div className="flex gap-2">
          <Input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Enter query (e.g. What is Orca? or What coding agents does Orca support?)"
            className="text-xs font-mono"
            onKeyDown={(e) => e.key === 'Enter' && handleTestRetrieval()}
          />
          <Button
            size="sm"
            onClick={handleTestRetrieval}
            disabled={testingQuery || !searchQuery.trim()}
            className="gap-1.5 text-xs shrink-0"
          >
            <Sparkles className={`w-3.5 h-3.5 ${testingQuery ? 'animate-spin' : ''}`} />
            Retrieve & Assemble
          </Button>
        </div>

        {retrievalResult && (
          <div className="space-y-3 pt-2">
            <div className="flex items-center justify-between text-xs text-muted-foreground">
              <span>
                Found <strong className="text-foreground">{retrievalResult.items.length}</strong> items (Budget used: {retrievalResult.budget_used} chars)
              </span>
              {contextPack && (
                <span className="font-mono text-[11px] text-sky-400">
                  Context Pack: {contextPack.items.length} items · {contextPack.total_chars} chars
                </span>
              )}
            </div>

            <div className="space-y-2 max-h-72 overflow-y-auto pr-1">
              {retrievalResult.items.map((item, idx) => (
                <div key={item.id || idx} className="p-2.5 rounded-lg border border-border bg-background/60 text-xs space-y-1.5">
                  <div className="flex items-center justify-between">
                    <span className="font-medium text-foreground flex items-center gap-1.5">
                      <FileText className="w-3.5 h-3.5 text-primary" />
                      {item.title || item.id}
                    </span>
                    <div className="flex items-center gap-1.5">
                      <Badge variant="outline" className="text-[10px] font-mono">
                        {item.source_type}
                      </Badge>
                      <Badge variant="secondary" className="text-[10px] font-mono font-bold">
                        Score: {item.score.toFixed(2)}
                      </Badge>
                    </div>
                  </div>

                  <p className="text-[11px] text-muted-foreground line-clamp-2 font-mono">
                    {item.snippet}
                  </p>

                  {item.explainability && (
                    <div className="p-1.5 rounded bg-muted/30 border border-muted text-[10px] space-y-0.5">
                      <div className="text-muted-foreground font-semibold flex items-center gap-1">
                        <CheckCircle2 className="w-3 h-3 text-emerald-400" />
                        Explainability:
                      </div>
                      {item.explainability.why.map((reason, rIdx) => (
                        <div key={rIdx} className="text-muted-foreground">
                          • {reason}
                        </div>
                      ))}
                      {item.explainability.boosts_applied.length > 0 && (
                        <div className="text-sky-400 text-[9px] pt-0.5">
                          Boosts: {item.explainability.boosts_applied.join(' · ')}
                        </div>
                      )}
                    </div>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}
      </div>

      {/* Memory Formation Tester */}
      <div className="p-4 rounded-xl border border-border bg-card/40 space-y-4">
        <div>
          <h4 className="text-xs font-semibold text-foreground flex items-center gap-2">
            <ShieldCheck className="w-3.5 h-3.5 text-indigo-400" />
            Deliberate Memory Formation & Conflict Engine
          </h4>
          <p className="text-[11px] text-muted-foreground mt-0.5">
            Validates eligibility, detects semantic conflicts on the same subject, and creates explicit superseded_by chains.
          </p>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
          <Input
            value={memSubject}
            onChange={(e) => setMemSubject(e.target.value)}
            placeholder="Subject"
            className="text-xs font-mono"
          />
          <Input
            value={memContent}
            onChange={(e) => setMemContent(e.target.value)}
            placeholder="Content"
            className="text-xs font-mono"
          />
          <Input
            value={memReason}
            onChange={(e) => setMemReason(e.target.value)}
            placeholder="Reason for retention"
            className="text-xs font-mono"
          />
        </div>

        <div className="flex items-center justify-between">
          <Button
            size="sm"
            onClick={handleTestMemoryFormation}
            disabled={formingMemory || !memSubject.trim() || !memContent.trim()}
            className="gap-1.5 text-xs"
          >
            <Brain className={`w-3.5 h-3.5 ${formingMemory ? 'animate-spin' : ''}`} />
            Evaluate Formation Policy
          </Button>

          {formationOutcome && (
            <div className="flex items-center gap-2 text-xs">
              <Badge
                variant={
                  formationOutcome.action === 'created'
                    ? 'emerald'
                    : formationOutcome.action === 'superseded'
                    ? 'amber'
                    : 'secondary'
                }
                className="font-mono uppercase text-[10px]"
              >
                {formationOutcome.action}
              </Badge>
              <span className="text-muted-foreground text-[11px] font-mono">
                {formationOutcome.reason}
              </span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
