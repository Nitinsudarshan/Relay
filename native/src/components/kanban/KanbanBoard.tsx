import React from 'react';
import { KanbanCard } from '../../types';
import { CheckCircle2, Clock, AlertCircle, User, Calendar } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

interface KanbanBoardProps {
  cards: KanbanCard[];
  onRefresh: () => void;
}

export const KanbanBoard: React.FC<KanbanBoardProps> = ({ cards, onRefresh }) => {
  const todoCards = cards.filter((c) => c.status === 'todo');
  const inProgressCards = cards.filter((c) => c.status === 'in_progress');
  const doneCards = cards.filter((c) => c.status === 'done');

  const renderColumn = (
    title: string,
    columnCards: KanbanCard[],
    badgeVariant: 'default' | 'amber' | 'emerald',
    icon: React.ReactNode
  ) => (
    <Card className="flex-1 flex flex-col p-4 bg-slate-900/60 border-slate-800">
      <div className="flex items-center justify-between mb-4 pb-2 border-b border-slate-800">
        <div className="flex items-center gap-2 font-semibold text-slate-200 text-sm">
          {icon}
          <span>{title}</span>
        </div>
        <Badge variant={badgeVariant}>{columnCards.length}</Badge>
      </div>

      <div className="flex-1 overflow-y-auto space-y-3 pr-1">
        {columnCards.length === 0 ? (
          <div className="text-center py-8 text-slate-500 text-xs italic">
            No cards in {title.toLowerCase()}
          </div>
        ) : (
          columnCards.map((card) => (
            <Card
              key={card.id}
              className="p-3.5 bg-slate-950/80 hover:bg-slate-900 transition-all border-slate-800 shadow-sm"
            >
              <div className="flex items-start justify-between gap-2 mb-2">
                <h4 className="font-medium text-slate-100 text-sm line-clamp-2">
                  {card.title}
                </h4>
                <Badge
                  variant={
                    card.priority === 'high'
                      ? 'destructive'
                      : card.priority === 'medium'
                      ? 'amber'
                      : 'secondary'
                  }
                  className="uppercase text-[10px] px-1.5 py-0"
                >
                  {card.priority}
                </Badge>
              </div>

              <p className="text-xs text-slate-400 line-clamp-2 mb-3">
                {card.description}
              </p>

              <div className="flex items-center justify-between text-[11px] text-slate-400 pt-2 border-t border-slate-800/80">
                <div className="flex items-center gap-1.5 text-slate-300 font-medium">
                  <User className="w-3 h-3 text-slate-400" />
                  <span>{card.assignee}</span>
                </div>

                {card.due_date && (
                  <div className="flex items-center gap-1 text-slate-400">
                    <Calendar className="w-3 h-3" />
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
    <div className="flex-1 flex flex-col h-full overflow-hidden">
      <div className="flex items-center justify-between mb-4">
        <div>
          <h2 className="text-xl font-bold text-slate-100">Actionable Kanban Board</h2>
          <p className="text-xs text-slate-400">
            Automatically parsed from meeting transcripts & saved in vault
          </p>
        </div>
        <Button size="sm" variant="outline" onClick={onRefresh}>
          Refresh Board
        </Button>
      </div>

      <div className="grid grid-cols-3 gap-4 flex-1 overflow-hidden">
        {renderColumn('To Do', todoCards, 'default', <AlertCircle className="w-4 h-4 text-blue-400" />)}
        {renderColumn('In Progress', inProgressCards, 'amber', <Clock className="w-4 h-4 text-amber-400" />)}
        {renderColumn('Done', doneCards, 'emerald', <CheckCircle2 className="w-4 h-4 text-emerald-400" />)}
      </div>
    </div>
  );
};
