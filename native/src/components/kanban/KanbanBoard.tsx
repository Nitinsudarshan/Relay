import React from 'react';
import { KanbanCard } from '../../types';
import { CheckCircle2, Clock, AlertCircle, User, Calendar, RefreshCw, Layers, HardDrive, ShieldCheck, Kanban } from 'lucide-react';
import { Card } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Skeleton } from '@/components/ui/skeleton';

interface KanbanBoardProps {
  cards: KanbanCard[];
  isLoading?: boolean;
  onRefresh: () => void;
}

export const KanbanBoard: React.FC<KanbanBoardProps> = ({ cards, isLoading = false, onRefresh }) => {
  const todoCards = cards.filter((c) => c.status === 'todo');
  const inProgressCards = cards.filter((c) => c.status === 'in_progress');
  const doneCards = cards.filter((c) => c.status === 'done');

  const renderColumn = (
    title: string,
    columnCards: KanbanCard[],
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
        {isLoading ? (
          <div className="space-y-3">
            <Skeleton className="h-24 w-full rounded-lg" />
            <Skeleton className="h-24 w-full rounded-lg" />
          </div>
        ) : columnCards.length === 0 ? (
          <div className="text-center py-12 px-4 rounded-lg border border-dashed border-border bg-muted/20 flex flex-col items-center justify-center h-full min-h-[160px]">
            <Layers className="w-8 h-8 text-muted-foreground/30 mb-2" />
            <p className="text-xs font-medium text-muted-foreground">No tasks in {title.toLowerCase()}</p>
            <p className="text-[11px] text-muted-foreground/60 mt-0.5">Push-to-talk dictates task cards live</p>
          </div>
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
                    card.priority === 'high'
                      ? 'bg-destructive/10 text-destructive border-destructive/30'
                      : card.priority === 'medium'
                      ? 'bg-warning text-warning-foreground border-warning/30'
                      : 'bg-success text-success-foreground border-success/30'
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
    <div className="flex-1 flex flex-col h-full overflow-hidden gap-4">
      {/* Kanban Board Top Bar */}
      <div className="flex items-center justify-between bg-card p-3 rounded-lg border border-border shrink-0">
        <div className="flex items-center gap-2">
          <Badge variant="outline" className="gap-1.5 text-xs font-mono px-2.5 py-0.5 border-border">
            <HardDrive className="w-3 h-3 text-primary" /> Vault Storage
          </Badge>
          <span className="text-xs text-muted-foreground font-mono">
            {cards.length} card{cards.length === 1 ? '' : 's'} active
          </span>
        </div>

        <Button
          size="sm"
          variant="outline"
          onClick={onRefresh}
          disabled={isLoading}
          aria-label="Refresh Kanban Board cards"
          className="gap-1.5 text-xs"
        >
          <RefreshCw className={`w-3.5 h-3.5 ${isLoading ? 'animate-spin' : ''}`} />
          <span>Refresh</span>
        </Button>
      </div>

      {/* Kanban Board Columns View */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4 flex-1 min-h-0 overflow-hidden">
        {renderColumn('To Do', todoCards, 'bg-primary/10 text-primary border-primary/20', <AlertCircle className="w-4 h-4 text-primary" />)}
        {renderColumn('In Progress', inProgressCards, 'bg-warning text-warning-foreground border-warning/30', <Clock className="w-4 h-4 text-warning-foreground" />)}
        {renderColumn('Done', doneCards, 'bg-success text-success-foreground border-success/30', <CheckCircle2 className="w-4 h-4 text-success-foreground" />)}
      </div>
    </div>
  );
};

