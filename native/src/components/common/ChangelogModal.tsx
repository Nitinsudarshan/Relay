import React from 'react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { X, Activity } from 'lucide-react';
import { cn } from '@/lib/utils';

interface ChangelogModalProps {
  open: boolean;
  onClose: () => void;
  currentVersion: string;
}

export const CHANGELOG_DATA = [
  {
    version: '0.3.4',
    date: '2026-08-19',
    type: 'patch',
    title: 'Native Build Fix for npm run dev:native',
    tags: ['Fixes'],
    domains: ['Build', 'Settings'],
    items: [
      { category: 'Fixes', domain: 'Build', text: 'Set default features to empty in Cargo.toml so tauri dev runs without requiring cmake.' },
    ],
  },
  {
    version: '0.3.3',
    date: '2026-08-19',
    type: 'patch',
    title: 'Multi-Monitor Active Positioning & Floating Pill Consolidation',
    tags: ['Improvements'],
    domains: ['Dictation', 'UI', 'Build'],
    items: [
      { category: 'Improvements', domain: 'Dictation', text: 'Consolidated legacy dictation-indicator into unified dictation-pill overlay window.' },
      { category: 'Improvements', domain: 'UI', text: 'Implemented active-window monitor auto-detection for multi-monitor overlay positioning.' },
      { category: 'Fixes', domain: 'Dictation', text: 'Hardened focus preservation and session locks across global hotkeys and mouse click-to-talk.' },
    ],
  },
  {
    version: '0.3.2',
    date: '2026-08-19',
    type: 'patch',
    title: 'Push-to-Talk Floating Pill Upgrade',
    tags: ['Improvements'],
    domains: ['Dictation', 'Speech', 'UI'],
    items: [
      { category: 'Improvements', domain: 'Dictation', text: 'Bound floating pill overlay directly to backend capture state machine (IDLE, LISTENING, TRANSCRIBING, SUCCESS, ERROR).' },
      { category: 'Improvements', domain: 'Speech', text: 'Added real-time RMS microphone audio level calculations emitted from Rust at ~25Hz to drive waveform animation.' },
      { category: 'Fixes', domain: 'Dictation', text: 'Guaranteed zero OS focus theft on overlay window for universal text injection into active apps.' },
    ],
  },
  {
    version: '0.3.1',
    date: '2026-08-19',
    type: 'minor',
    title: 'Model Manager, Hotkey Recorder & Floating Overlay',
    tags: ['Features', 'Improvements'],
    domains: ['LLM', 'Speech', 'Dictation', 'Settings', 'UI'],
    items: [
      { category: 'Features', domain: 'LLM', text: 'Added Ollama daemon auto-detection, status monitoring, and one-click model pulling (llama3.2, qwen2.5).' },
      { category: 'Features', domain: 'Speech', text: 'Added Whisper GGML model selection & status monitoring (ggml-tiny.en.bin, ggml-base.en.bin).' },
      { category: 'Features', domain: 'Dictation', text: 'Added interactive HotkeyRecorder component for setting custom global hotkeys (Ctrl+Shift+Space, Ctrl+Space).' },
      { category: 'Features', domain: 'Dictation', text: 'Created non-focus-stealing transparent native desktop overlay window for instant speech capture.' },
      { category: 'Improvements', domain: 'UI', text: 'Expanded release notes modal to 80% width with category and domain tags.' },
    ],
  },
  {
    version: '0.3.0',
    date: '2026-08-19',
    type: 'minor',
    title: 'Universal Dictation, Global Hotkeys & Voice Chat',
    tags: ['Features'],
    domains: ['Dictation', 'Speech', 'LLM', 'Vault'],
    items: [
      { category: 'Features', domain: 'Dictation', text: 'Registered global hotkeys (Ctrl+Shift+Space, Ctrl+Space) that type transcribed speech into whichever OS field has focus.' },
      { category: 'Features', domain: 'LLM', text: 'Added in-app voice chat grounded in vault notes with source attribution.' },
      { category: 'Features', domain: 'Speech', text: 'Wired real microphone capture via cpal resampled to 16kHz mono.' },
      { category: 'Features', domain: 'Speech', text: 'Integrated local whisper-rs (whisper.cpp) transcription engine.' },
      { category: 'Improvements', domain: 'Vault', text: 'Added keyword-ranked search notes retrieval for voice grounding.' },
    ],
  },
  {
    version: '0.2.2',
    date: '2026-08-19',
    type: 'patch',
    title: 'UI & Layout Enhancements',
    tags: ['Improvements'],
    domains: ['UI', 'Modal'],
    items: [
      { category: 'Improvements', domain: 'UI', text: 'Expanded Changelog Modal container width to 80% (w-[80vw] max-w-5xl) across native and web.' },
      { category: 'Improvements', domain: 'UI', text: 'Added release category tags (Features, Fixes, Improvements) and domain tags.' },
    ],
  },
  {
    version: '0.2.1',
    date: '2026-08-19',
    type: 'patch',
    title: 'Fix Windows Build (clang.dll / whisper-rs-sys missing dependency)',
    tags: ['Fixes'],
    domains: ['Build', 'STT', 'LLM'],
    items: [
      { category: 'Fixes', domain: 'Build', text: 'Gated whisper-rs under optional whisper-local feature to prevent LLVM/clang.dll missing panics on Windows.' },
      { category: 'Improvements', domain: 'Speech', text: 'Added fallback heuristic STT engine when local Ollama or Whisper model is not configured.' },
    ],
  },
  {
    version: '0.2.0',
    date: '2026-08-19',
    type: 'minor',
    title: 'Relay Visual Identity Pass ("Monochrome & Electric Blue")',
    tags: ['Features', 'Improvements'],
    domains: ['UI', 'Dictation', 'Kanban', 'Vault', 'Settings'],
    items: [
      { category: 'Features', domain: 'UI', text: 'Updated CSS variables to Monochrome & Electric Blue palette (#2563EB light / #60A5FA dark) with 3-way semantic colors.' },
      { category: 'Features', domain: 'UI', text: 'Designed two-tone Relay logo mark and integrated across native sidebar, web dashboard, login page, and favicon.' },
      { category: 'Features', domain: 'Dictation', text: 'Rebuilt Push-to-Talk floating Dictation Pill overlay with state machine, live waveform, and rotating captions.' },
      { category: 'Improvements', domain: 'Settings', text: 'Restructured Provider Settings with General, Providers, Triggers, Vault, Account, and Data & Privacy domains.' },
      { category: 'Improvements', domain: 'Kanban', text: 'Unified native and web Kanban boards with 3-way semantic priority badges.' },
      { category: 'Improvements', domain: 'Vault', text: 'Restructured Scribble Notes with master list, detail pane, pill actions toolbar, and raw audio reassurance line.' },
    ],
  },
  {
    version: '0.1.0',
    date: '2026-08-19',
    type: 'major',
    title: 'Initial Release — Multi-Surface Architecture',
    tags: ['Features'],
    domains: ['Architecture', 'Speech', 'Sync'],
    items: [
      { category: 'Features', domain: 'Speech', text: 'Windows native Tauri app with WASAPI audio capture & Rust backend pipeline.' },
      { category: 'Features', domain: 'Sync', text: 'Next.js web dashboard with Supabase Cloud hybrid sync.' },
    ],
  },
];

export const ChangelogModal: React.FC<ChangelogModalProps> = ({
  open,
  onClose,
  currentVersion,
}) => {
  if (!open) return null;

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
                  v{currentVersion}
                </Badge>
              </div>
              <p className="text-xs text-muted-foreground">Version history & categorized release tags</p>
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
          {CHANGELOG_DATA.map((entry) => (
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
                        tag === 'Features' && "bg-primary/10 text-primary border-primary/20",
                        tag === 'Fixes' && "bg-destructive/10 text-destructive border-destructive/20",
                        tag === 'Improvements' && "bg-emerald-500/10 text-emerald-500 border-emerald-500/20",
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
                <span className="font-mono text-[10px] text-muted-foreground">{entry.date}</span>
              </div>

              <ul className="space-y-1.5 pt-1 pl-2">
                {entry.items.map((item, idx) => (
                  <li key={idx} className="text-xs text-muted-foreground flex items-start gap-2 leading-relaxed">
                    <div className="flex items-center gap-1 shrink-0 mt-0.5">
                      <Badge
                        variant="outline"
                        className={cn(
                          "text-[9px] font-mono uppercase px-1 py-0 rounded",
                          item.category === 'Features' && "bg-primary/10 text-primary border-primary/20",
                          item.category === 'Fixes' && "bg-destructive/10 text-destructive border-destructive/20",
                          item.category === 'Improvements' && "bg-emerald-500/10 text-emerald-500 border-emerald-500/20",
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
          ))}
        </div>

        {/* Modal Footer */}
        <div className="p-3 bg-muted/30 border-t border-border flex items-center justify-between shrink-0 text-xs text-muted-foreground">
          <span className="font-mono text-[10px]">Root Registry: VERSION</span>
          <Button size="sm" variant="default" onClick={onClose} className="text-xs h-8">
            Close Changelog
          </Button>
        </div>
      </div>
    </div>
  );
};
