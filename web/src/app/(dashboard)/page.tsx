import React from "react";
import { getSupabaseClient } from "@/lib/supabase/client";
import { Kanban, CheckCircle2, Clock, AlertCircle, User, Calendar, Layers } from "lucide-react";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { PageHeader } from "@/components/page-header";
import { EmptyState } from "@/components/empty-state";

export default async function DashboardPage() {
  const supabase = getSupabaseClient();
  const { data: cards } = await supabase.getKanbanCards();

  const todoCards = cards?.filter((c) => c.status === "todo") || [];
  const inProgressCards = cards?.filter((c) => c.status === "in_progress") || [];
  const doneCards = cards?.filter((c) => c.status === "done") || [];

  const renderColumn = (
    title: string,
    columnCards: typeof todoCards,
    badgeClass: string,
    icon: React.ReactNode
  ) => (
    <Card className="flex-1 flex flex-col p-4 bg-card border-border shadow-xs">
      <div className="flex items-center justify-between mb-4 pb-2 border-b border-border">
        <div className="flex items-center gap-2 font-semibold text-foreground text-sm">
          {icon}
          <span>{title}</span>
        </div>
        <Badge variant="outline" className={`px-2 py-0.5 text-xs font-mono font-bold ${badgeClass}`}>
          {columnCards.length}
        </Badge>
      </div>

      <div className="flex-1 overflow-y-auto space-y-3 pr-1">
        {columnCards.length === 0 ? (
          <EmptyState
            icon={Layers}
            title={`No tasks in ${title.toLowerCase()}`}
            description="Synced state from Relay desktop app"
            minHeight="min-h-[160px]"
          />
        ) : (
          columnCards.map((card) => (
            <Card
              key={card.id}
              className="p-4 bg-background hover:border-primary/40 transition-all border-border shadow-xs hover:shadow-sm group"
            >
              <div className="flex items-start justify-between gap-2 mb-2">
                <h4 className="font-medium text-foreground text-sm line-clamp-2 group-hover:text-primary transition-colors">
                  {card.title}
                </h4>
                <Badge
                  variant="outline"
                  className={`uppercase text-[10px] px-1.5 py-0 font-bold tracking-wider shrink-0 ${
                    card.priority === "high"
                      ? "bg-destructive/10 text-destructive border-destructive/30"
                      : card.priority === "medium"
                      ? "bg-warning text-warning-foreground border-warning/30"
                      : "bg-success text-success-foreground border-success/30"
                  }`}
                >
                  {card.priority}
                </Badge>
              </div>

              <p className="text-xs text-muted-foreground line-clamp-2 mb-3 leading-relaxed">
                {card.description}
              </p>

              <div className="flex items-center justify-between text-[11px] text-muted-foreground pt-2.5 border-t border-border">
                <div className="flex items-center gap-1.5 text-foreground/80 font-medium">
                  <User className="w-3.5 h-3.5 text-muted-foreground" />
                  <span>{card.assignee}</span>
                </div>

                {card.due_date && (
                  <div className="flex items-center gap-1 text-muted-foreground font-mono text-[10px]">
                    <Calendar className="w-3.5 h-3.5 text-primary" />
                    <span>{card.due_date}</span>
                  </div>
                )}
              </div>
            </Card>
          ))
        )}
      </div>
    </Card>
  );

  return (
    <div className="flex flex-1 flex-col gap-6 p-4 md:p-6 lg:p-8 max-w-7xl mx-auto w-full">
      {/* Centralized Page Header */}
      <PageHeader
        kicker="RELAY · KANBAN"
        title="Structured tasks,"
        highlightText="extracted live."
        description={`${cards?.length || 0} action card${(cards?.length || 0) === 1 ? "" : "s"} synced across Windows desktop & cloud.`}
      />

      {/* Synced Kanban Cards View */}
      <div className="flex flex-col gap-4">
        <div className="flex items-center justify-between bg-card p-3 rounded-lg border border-border">
          <h2 className="text-sm font-bold text-foreground flex items-center gap-2">
            <Kanban className="w-4 h-4 text-primary" />
            <span>Hybrid Synced Board</span>
          </h2>
          <Badge variant="outline" className="text-xs font-mono border-border">
            Supabase Cloud RLS Active
          </Badge>
        </div>

        {/* Responsive Kanban Columns Grid */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 flex-1">
          {renderColumn("To Do", todoCards, "bg-primary/10 text-primary border-primary/20", <AlertCircle className="w-4 h-4 text-primary" />)}
          {renderColumn("In Progress", inProgressCards, "bg-warning text-warning-foreground border-warning/30", <Clock className="w-4 h-4 text-warning-foreground" />)}
          {renderColumn("Done", doneCards, "bg-success text-success-foreground border-success/30", <CheckCircle2 className="w-4 h-4 text-success-foreground" />)}
        </div>
      </div>
    </div>
  );
}

