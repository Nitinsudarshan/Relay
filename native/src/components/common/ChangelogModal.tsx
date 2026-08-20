import React, { useEffect, useState } from 'react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { X, Activity } from 'lucide-react';
import { cn } from '@/lib/utils';
import { invoke } from '@tauri-apps/api/core';
import { ChangelogEntry } from '@/types';

interface ChangelogModalProps {
  open: boolean;
  onClose: () => void;
  currentVersion?: string;
}

export const ChangelogModal: React.FC<ChangelogModalProps> = ({
  open,
  onClose,
  currentVersion: propVersion,
}) => {
  const [entries, setEntries] = useState<ChangelogEntry[]>([]);
  const [version, setVersion] = useState<string>(propVersion || '');
  const [loading, setLoading] = useState<boolean>(true);

  useEffect(() => {
    if (!open) return;

    let isMounted = true;
    setLoading(true);

    const loadData = async () => {
      try {
        const [fetchedEntries, fetchedVersion] = await Promise.all([
          invoke<ChangelogEntry[]>('get_changelog'),
          propVersion ? Promise.resolve(propVersion) : invoke<string>('get_app_version'),
        ]);
        if (isMounted) {
          setEntries(fetchedEntries || []);
          if (fetchedVersion) setVersion(fetchedVersion);
        }
      } catch (err) {
        console.error('Failed to load changelog:', err);
      } finally {
        if (isMounted) setLoading(false);
      }
    };

    loadData();

    return () => {
      isMounted = false;
    };
  }, [open, propVersion]);

  if (!open) return null;

  const displayVersion = version || (entries.length > 0 ? entries[0].version : '0.6.0');

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4 animate-in fade-in duration-200">
      <div className="bg-popover border border-border shadow-2xl rounded-2xl w-[80vw] max-w-5xl max-h-[85vh] flex flex-col overflow-hidden text-popover-foreground">
        {/* Modal Header */}
        <div className="p-4 md:p-5 border-b border-border flex items-center justify-between shrink-0">
          <div className="flex items-center gap-2.5">
            <div className="p-2 rounded-xl bg-primary/10 text-primary">
              <Activity className="w-5 h-5" />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <h3 className="text-base font-extrabold text-foreground">Relay Release Notes</h3>
                <Badge variant="outline" className="text-xs font-mono border-primary/30 text-primary">
                  v{displayVersion}
                </Badge>
              </div>
              <p className="text-xs text-muted-foreground">Live version history & categorized release tags</p>
            </div>
          </div>

          <Button
            size="icon"
            variant="ghost"
            onClick={onClose}
            className="h-8 w-8 rounded-full text-muted-foreground hover:text-foreground"
          >
            <X className="w-4 h-4" />
          </Button>
        </div>

        {/* Modal Content Scrollable Area */}
        <div className="flex-1 overflow-y-auto p-4 md:p-5 space-y-6">
          {loading && entries.length === 0 ? (
            <div className="p-8 text-center text-xs text-muted-foreground font-mono">
              Loading release notes...
            </div>
          ) : entries.length === 0 ? (
            <div className="p-8 text-center text-xs text-muted-foreground font-mono">
              No changelog entries found.
            </div>
          ) : (
            entries.map((entry) => (
              <div key={entry.version} className="space-y-2 border-b border-border/60 pb-5 last:border-none last:pb-0">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-mono font-bold text-xs text-primary bg-primary/10 px-2 py-0.5 rounded-md">
                      v{entry.version}
                    </span>
                    <span className="text-xs font-bold text-foreground">{entry.title}</span>

                    {/* Category Type Tags */}
                    {entry.tags.map((tag) => (
                      <Badge
                        key={tag}
                        variant="outline"
                        className={cn(
                          "text-[10px] font-mono uppercase px-1.5 py-0 border-transparent",
                          (tag === 'Features' || tag === 'Backend') && "bg-primary/10 text-primary border-primary/20",
                          tag === 'Fixes' && "bg-destructive/10 text-destructive border-destructive/20",
                          (tag === 'Improvements' || tag === 'Frontend') && "bg-emerald-500/10 text-emerald-500 border-emerald-500/20",
                          tag === 'Security' && "bg-amber-500/10 text-amber-500 border-amber-500/20"
                        )}
                      >
                        {tag}
                      </Badge>
                    ))}

                    {/* Domain Tags */}
                    {entry.domains.map((dom) => (
                      <Badge
                        key={dom}
                        variant="outline"
                        className="text-[9px] font-mono uppercase px-1.5 py-0 bg-muted/60 text-muted-foreground border-border"
                      >
                        {dom}
                      </Badge>
                    ))}
                  </div>
                  {entry.date && <span className="font-mono text-[10px] text-muted-foreground">{entry.date}</span>}
                </div>

                <ul className="space-y-1.5 pt-1 pl-2">
                  {entry.items.map((item, idx) => (
                    <li key={idx} className="text-xs text-muted-foreground flex items-start gap-2 leading-relaxed">
                      <div className="flex items-center gap-1 shrink-0 mt-0.5">
                        <Badge
                          variant="outline"
                          className={cn(
                            "text-[9px] font-mono uppercase px-1 py-0 rounded",
                            (item.category === 'Features' || item.category === 'Backend') && "bg-primary/10 text-primary border-primary/20",
                            item.category === 'Fixes' && "bg-destructive/10 text-destructive border-destructive/20",
                            (item.category === 'Improvements' || item.category === 'Frontend') && "bg-emerald-500/10 text-emerald-500 border-emerald-500/20",
                            item.category === 'Security' && "bg-amber-500/10 text-amber-500 border-amber-500/20"
                          )}
                        >
                          {item.category}
                        </Badge>
                        <Badge
                          variant="outline"
                          className="text-[8px] font-mono uppercase px-1 py-0 bg-muted text-muted-foreground border-border/70 rounded"
                        >
                          {item.domain}
                        </Badge>
                      </div>
                      <span>{item.text}</span>
                    </li>
                  ))}
                </ul>
              </div>
            ))
          )}
        </div>

        {/* Modal Footer */}
        <div className="p-3 bg-muted/30 border-t border-border flex items-center justify-between shrink-0 text-xs text-muted-foreground">
          <span className="font-mono text-[10px]">Root Registry: CHANGELOG.md (Dynamic)</span>
          <Button size="sm" variant="default" onClick={onClose} className="text-xs h-8">
            Close Changelog
          </Button>
        </div>
      </div>
    </div>
  );
};

