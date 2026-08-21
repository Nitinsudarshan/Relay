import React, { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { TrashItem } from '../../types';
import {
  Trash2,
  RotateCcw,
  Clock,
  Mic,
  Sparkles,
  RefreshCw,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { ConfirmationModal } from '../common/ConfirmationModal';

export const TrashSettings: React.FC = () => {
  const [items, setItems] = useState<TrashItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [actionBusy, setActionBusy] = useState(false);
  const [confirmEmpty, setConfirmEmpty] = useState(false);
  const [itemToDelete, setItemToDelete] = useState<TrashItem | null>(null);

  const fetchTrash = useCallback(async () => {
    setLoading(true);
    try {
      const list = await invoke<TrashItem[]>('get_trash_items');
      setItems(list);
    } catch (err) {
      console.error('Failed to load trash items:', err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchTrash();
  }, [fetchTrash]);

  const handleRestore = async (trashId: string) => {
    setActionBusy(true);
    try {
      await invoke('restore_trash_item', { trashId });
      setItems((prev) => prev.filter((item) => item.id !== trashId));
    } catch (err) {
      console.error('Failed to restore item:', err);
    } finally {
      setActionBusy(false);
    }
  };

  const handleDeletePermanently = async () => {
    if (!itemToDelete) return;
    setActionBusy(true);
    try {
      await invoke('delete_trash_item_permanently', { trashId: itemToDelete.id });
      setItems((prev) => prev.filter((item) => item.id !== itemToDelete.id));
      setItemToDelete(null);
    } catch (err) {
      console.error('Failed to permanently delete item:', err);
    } finally {
      setActionBusy(false);
    }
  };

  const handleEmptyTrash = async () => {
    setActionBusy(true);
    try {
      await invoke('empty_trash');
      setItems([]);
      setConfirmEmpty(false);
    } catch (err) {
      console.error('Failed to empty trash:', err);
    } finally {
      setActionBusy(false);
    }
  };

  const getDaysRemaining = (expiresAt: string): number => {
    const exp = new Date(expiresAt).getTime();
    const now = Date.now();
    const diffDays = Math.ceil((exp - now) / (1000 * 60 * 60 * 24));
    return Math.max(0, diffDays);
  };

  return (
    <div className="space-y-6">
      {/* Header & Empty Trash Toolbar */}
      <div className="flex flex-wrap items-center justify-between gap-3 pb-3 border-b border-border">
        <div>
          <h3 className="text-sm font-bold text-foreground flex items-center gap-2">
            <Trash2 className="w-4 h-4 text-muted-foreground" />
            <span>Trash & Deleted Items</span>
          </h3>
          <p className="text-xs text-muted-foreground mt-0.5">
            Deleted Voice Notes and Scribbles remain recoverable for 30 days before permanent automatic purge.
          </p>
        </div>

        <div className="flex items-center gap-2">
          <Button
            size="sm"
            variant="outline"
            onClick={fetchTrash}
            disabled={loading || actionBusy}
            className="h-8 text-xs gap-1.5"
            title="Refresh trash"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${loading ? 'animate-spin' : ''}`} />
            <span>Refresh</span>
          </Button>

          {items.length > 0 && (
            <Button
              size="sm"
              variant="destructive"
              onClick={() => setConfirmEmpty(true)}
              disabled={actionBusy}
              className="h-8 text-xs gap-1.5 font-semibold"
            >
              <Trash2 className="w-3.5 h-3.5" />
              <span>Empty Trash</span>
            </Button>
          )}
        </div>
      </div>

      {/* Trash List */}
      {loading ? (
        <div className="text-center py-12 text-xs text-muted-foreground">
          Checking trash items…
        </div>
      ) : items.length === 0 ? (
        <div className="text-center py-16 bg-card rounded-lg border border-border/60 text-muted-foreground space-y-2">
          <Trash2 className="w-10 h-10 mx-auto opacity-30" />
          <p className="text-sm font-semibold text-foreground">Trash is empty</p>
          <p className="text-xs max-w-sm mx-auto">
            Deleted Voice Notes and Scribbles will appear here for 30 days before permanent deletion.
          </p>
        </div>
      ) : (
        <div className="space-y-3">
          {items.map((item) => {
            const daysLeft = getDaysRemaining(item.expires_at);

            return (
              <div
                key={item.id}
                className="p-4 rounded-lg bg-card border border-border space-y-2 transition-all"
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1 flex-wrap">
                      <Badge
                        variant="outline"
                        className="text-[9px] font-mono uppercase px-1.5 py-0 gap-1 bg-muted"
                      >
                        {item.item_type === 'voice_note' ? (
                          <>
                            <Mic className="w-2.5 h-2.5" /> VOICE NOTE
                          </>
                        ) : (
                          <>
                            <Sparkles className="w-2.5 h-2.5" /> SCRIBBLE
                          </>
                        )}
                      </Badge>
                      <span className="text-[10px] text-muted-foreground font-mono flex items-center gap-1">
                        <Clock className="w-3 h-3" />
                        <span>
                          Deleted {new Date(item.deleted_at).toLocaleDateString([], { month: 'short', day: 'numeric' })}
                        </span>
                      </span>
                      <span className="text-[10px] text-amber-500 font-mono font-medium">
                        · {daysLeft} day{daysLeft === 1 ? '' : 's'} remaining
                      </span>
                    </div>

                    <h4 className="text-sm font-bold text-foreground truncate">{item.title}</h4>
                    <p className="text-xs text-muted-foreground line-clamp-2 mt-1 leading-relaxed">
                      {item.snippet}
                    </p>
                  </div>

                  {/* Actions */}
                  <div className="flex items-center gap-1.5 shrink-0">
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => handleRestore(item.id)}
                      disabled={actionBusy}
                      className="h-7 px-2 text-xs gap-1"
                      title="Restore item to active"
                    >
                      <RotateCcw className="w-3.5 h-3.5 text-primary" />
                      <span>Restore</span>
                    </Button>

                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => setItemToDelete(item)}
                      disabled={actionBusy}
                      className="h-7 px-2 text-xs text-muted-foreground hover:text-destructive hover:bg-destructive/10"
                      title="Delete permanently"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </Button>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Viewport-level Modal for Empty Trash */}
      <ConfirmationModal
        isOpen={confirmEmpty}
        title="Empty Trash permanently?"
        description={`All ${items.length} item${items.length === 1 ? '' : 's'} in Trash will be permanently deleted from your local disk. This action cannot be undone.`}
        confirmLabel="Empty Trash"
        cancelLabel="Cancel"
        variant="destructive"
        isBusy={actionBusy}
        onConfirm={handleEmptyTrash}
        onCancel={() => setConfirmEmpty(false)}
      />

      {/* Viewport-level Modal for Permanent Delete */}
      <ConfirmationModal
        isOpen={itemToDelete !== null}
        title="Permanently delete item?"
        description={`"${itemToDelete?.title}" will be permanently removed from your disk. This action cannot be undone.`}
        confirmLabel="Delete permanently"
        cancelLabel="Cancel"
        variant="destructive"
        isBusy={actionBusy}
        onConfirm={handleDeletePermanently}
        onCancel={() => setItemToDelete(null)}
      />
    </div>
  );
};
