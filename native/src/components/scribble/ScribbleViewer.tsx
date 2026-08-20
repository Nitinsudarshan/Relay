import React, { useState } from 'react';
import {
  Search,
  SlidersHorizontal,
  Star,
  Trash2,
  Copy,
  Check,
  FileText,
  Sparkles,
  Edit3,
  RefreshCw,
  Languages,
  Share2,
  Folder,
  Download,
  ShieldCheck,
  MessageSquarePlus,
  ArrowRight,
} from 'lucide-react';
import { Card } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';

interface ScribbleViewerProps {
  content: string;
  transcript: string;
  isLoading?: boolean;
}

interface ScribbleNote {
  id: string;
  title: string;
  date: string;
  cleanedText: string;
  rawTranscript: string;
  isStarred?: boolean;
}

const DEMO_NOTES: ScribbleNote[] = [
  {
    id: 'note-1',
    title: 'Q3 Product Strategy & Roadmap Sync',
    date: 'Today, 2:15 PM',
    cleanedText: `# Q3 Product Strategy & Roadmap Sync\n\n## Key Takeaways\n- Focus on Windows Native dictation pill performance.\n- LanceDB vector RAG search for Obsidian vault notes.\n- Supabase cloud auth for hybrid remote access.\n\n## Action Items\n- [x] Refactor UI tokens to Monochrome & Electric Blue.\n- [ ] Wire audio level meter keyframes to Rust WASAPI recorder.\n- [ ] Finalize Kanban priority badge 3-way semantic split.`,
    rawTranscript: 'Okay so for Q3 we really need to focus on native dictation pill performance and getting the LanceDB RAG search working over Obsidian notes...',
    isStarred: true,
  },
  {
    id: 'note-2',
    title: 'Architecture Review: MCP Trigger Routing',
    date: 'Yesterday, 4:30 PM',
    cleanedText: `# Architecture Review: MCP Trigger Routing\n\n## Overview\nDynamic phrase matching maps user voice input (e.g. "Schedule quick sync") to Google Calendar tool calls via JSON-RPC MCP handlers.`,
    rawTranscript: 'Let us make sure the trigger engine properly extracts parameters and routes to google calendar mcp server without blocking the main looper thread...',
    isStarred: false,
  },
];

export const ScribbleViewer: React.FC<ScribbleViewerProps> = ({
  content,
  transcript,
  isLoading = false,
}) => {
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedNoteId, setSelectedNoteId] = useState<string>('note-1');
  const [copied, setCopied] = useState(false);
  const [notes, setNotes] = useState<ScribbleNote[]>(() => {
    if (content) {
      return [
        {
          id: 'note-live',
          title: content.split('\n')[0].replace(/^#\s*/, '') || 'Latest Voice Scribble',
          date: 'Just now',
          cleanedText: content,
          rawTranscript: transcript || 'Live push-to-talk voice recording',
          isStarred: false,
        },
        ...DEMO_NOTES,
      ];
    }
    return DEMO_NOTES;
  });

  const selectedNote = notes.find((n) => n.id === selectedNoteId) || notes[0];

  const handleCopy = () => {
    if (!selectedNote) return;
    navigator.clipboard.writeText(selectedNote.cleanedText);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const toggleStar = (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setNotes((prev) =>
      prev.map((n) => (n.id === id ? { ...n, isStarred: !n.isStarred } : n))
    );
  };

  return (
    <div className="flex-1 flex flex-col md:flex-row gap-4 min-h-0 overflow-hidden">
      {/* Master List Pane */}
      <aside className="w-full md:w-80 flex flex-col shrink-0 bg-card rounded-lg border border-border overflow-hidden">
        {/* Master List Header: Search & Filter Row */}
        <div className="p-3 border-b border-border space-y-2">
          <div className="relative">
            <Search className="w-3.5 h-3.5 absolute left-3 top-2.5 text-muted-foreground" />
            <Input
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search vault scribbles..."
              className="pl-8 text-xs h-8 bg-muted/30"
            />
          </div>

          <div className="flex items-center justify-between text-[11px] text-muted-foreground px-1 pt-1">
            <span className="font-mono uppercase tracking-wider text-[10px] font-bold">
              {notes.length} VAULT SCRIBBLES
            </span>
            <Button variant="ghost" size="sm" className="h-6 px-2 text-[10px] gap-1">
              <SlidersHorizontal className="w-3 h-3" />
              <span>Sort</span>
            </Button>
          </div>
        </div>

        {/* Master List Items */}
        <div className="flex-1 overflow-y-auto p-2 space-y-1">
          {notes
            .filter((n) => n.title.toLowerCase().includes(searchQuery.toLowerCase()))
            .map((note) => (
              <div
                key={note.id}
                onClick={() => setSelectedNoteId(note.id)}
                className={`p-3 rounded-lg border text-left cursor-pointer transition-all ${
                  selectedNoteId === note.id
                    ? 'bg-accent/60 border-primary/50 shadow-xs'
                    : 'bg-card border-transparent hover:bg-muted/40'
                }`}
              >
                <div className="flex items-start justify-between gap-2 mb-1">
                  <h4 className="text-xs font-bold text-foreground line-clamp-1 flex-1">
                    {note.title}
                  </h4>
                  <button
                    type="button"
                    onClick={(e) => toggleStar(note.id, e)}
                    className="text-muted-foreground hover:text-amber-500 transition-colors"
                  >
                    <Star
                      className={`w-3.5 h-3.5 ${
                        note.isStarred ? 'fill-amber-400 text-amber-400' : ''
                      }`}
                    />
                  </button>
                </div>

                <p className="text-[11px] text-muted-foreground line-clamp-2 leading-snug mb-2">
                  {note.rawTranscript}
                </p>

                <div className="flex items-center justify-between text-[10px] text-muted-foreground font-mono">
                  <span>{note.date}</span>
                  <Badge variant="outline" className="text-[9px] px-1 py-0 border-border">
                    Markdown
                  </Badge>
                </div>
              </div>
            ))}
        </div>
      </aside>

      {/* Detail Pane */}
      <main className="flex-1 flex flex-col bg-card rounded-lg border border-border p-6 overflow-y-auto min-h-0">
        {selectedNote ? (
          <div className="space-y-6 flex-1 flex flex-col">
            {/* Breadcrumb & Top Bar */}
            <div className="flex flex-wrap items-center justify-between gap-2 pb-4 border-b border-border">
              <div>
                <p className="font-mono text-[10px] font-semibold text-muted-foreground uppercase tracking-widest mb-1">
                  RELAY · VAULT NOTE · {selectedNote.date.toUpperCase()}
                </p>
                <h2 className="text-xl font-extrabold text-foreground tracking-tight">
                  {selectedNote.title}
                </h2>
              </div>

              {/* Action Toolbar of Buttons */}
              <div className="flex items-center gap-1.5 flex-wrap">
                <Button size="sm" variant="outline" className="rounded-lg text-xs gap-1.5 h-8">
                  <Edit3 className="w-3.5 h-3.5" />
                  <span>Edit</span>
                </Button>
                <Button size="sm" variant="outline" className="rounded-lg text-xs gap-1.5 h-8">
                  <RefreshCw className="w-3.5 h-3.5" />
                  <span>Transform</span>
                </Button>
                <Button size="sm" variant="outline" className="rounded-lg text-xs gap-1.5 h-8">
                  <Languages className="w-3.5 h-3.5" />
                  <span>Translate</span>
                </Button>
                <Button size="sm" variant="outline" className="rounded-lg text-xs gap-1.5 h-8">
                  <Share2 className="w-3.5 h-3.5" />
                  <span>Share</span>
                </Button>
                <Button size="sm" variant="default" className="rounded-lg text-xs gap-1.5 h-8">
                  <Sparkles className="w-3.5 h-3.5" />
                  <span>Ask Relay</span>
                </Button>

                <div className="h-4 w-px bg-border mx-1" />

                {/* Icon-Only Quick Actions */}
                <Button
                  size="icon"
                  variant="ghost"
                  onClick={handleCopy}
                  className="h-8 w-8 rounded-lg text-muted-foreground hover:text-foreground"
                  title="Copy Markdown"
                >
                  {copied ? <Check className="w-3.5 h-3.5 text-emerald-500" /> : <Copy className="w-3.5 h-3.5" />}
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  className="h-8 w-8 rounded-lg text-muted-foreground hover:text-foreground"
                  title="Folder Location"
                >
                  <Folder className="w-3.5 h-3.5" />
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  className="h-8 w-8 rounded-lg text-muted-foreground hover:text-foreground"
                  title="Export File"
                >
                  <Download className="w-3.5 h-3.5" />
                </Button>
              </div>
            </div>

            {/* Cleaned Structured Markdown View */}
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <span className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest">
                  STRUCTURED MARKNOWN NOTE
                </span>
                <Badge variant="outline" className="text-[10px] font-mono border-primary/30 text-primary">
                  Obsidian Compatible
                </Badge>
              </div>
              <div className="p-5 rounded-lg bg-muted/30 border border-border font-mono text-xs text-foreground whitespace-pre-wrap leading-relaxed">
                {selectedNote.cleanedText}
              </div>
            </div>

            {/* Labeled Raw Transcript Section */}
            <div className="space-y-2">
              <span className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest">
                LABELED RAW TRANSCRIPT
              </span>
              <div className="p-4 rounded-lg bg-card border border-border text-xs text-muted-foreground italic leading-relaxed">
                "{selectedNote.rawTranscript}"
              </div>
            </div>

            {/* Standing "Ask Relay to Reshape" Suggestions Block */}
            <div className="p-4 rounded-lg bg-accent/30 border border-accent-foreground/20 space-y-3">
              <div className="flex items-center gap-2 text-xs font-bold text-foreground">
                <MessageSquarePlus className="w-4 h-4 text-primary" />
                <span>Ask Relay to Reshape</span>
              </div>
              <div className="flex flex-wrap gap-2">
                <Button variant="outline" size="sm" className="rounded-lg text-xs bg-card gap-1.5">
                  <span>Pull out action items</span>
                  <ArrowRight className="w-3 h-3 text-primary" />
                </Button>
                <Button variant="outline" size="sm" className="rounded-lg text-xs bg-card gap-1.5">
                  <span>Draft executive summary email</span>
                  <ArrowRight className="w-3 h-3 text-primary" />
                </Button>
                <Button variant="outline" size="sm" className="rounded-lg text-xs bg-card gap-1.5">
                  <span>Summarize in 3 key bullet points</span>
                  <ArrowRight className="w-3 h-3 text-primary" />
                </Button>
              </div>
            </div>

            {/* Quiet Reassurance Line Near Export */}
            <div className="pt-4 border-t border-border flex items-center justify-between text-[11px] text-muted-foreground">
              <div className="flex items-center gap-1.5">
                <ShieldCheck className="w-3.5 h-3.5 text-emerald-500" />
                <span>Both versions kept — your raw voice is never overwritten</span>
              </div>
              <span className="font-mono text-[10px]">Vault Storage: .relay/vault/notes</span>
            </div>
          </div>
        ) : (
          <div className="flex-1 flex flex-col items-center justify-center text-center p-8 text-muted-foreground">
            <FileText className="w-10 h-10 mb-2 opacity-40" />
            <p className="text-sm font-semibold">Select a scribble note to view</p>
          </div>
        )}
      </main>
    </div>
  );
};
