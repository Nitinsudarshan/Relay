import React, { useEffect, useRef, useState, useId } from 'react';
import mermaid from 'mermaid';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Code2, AlertTriangle, Check, Copy, Eye, GitFork, Sparkles } from 'lucide-react';

interface MarkdownViewProps {
  content: string;
  className?: string;
}

// Regex to identify if a line marks the beginning of a Mermaid diagram
const MERMAID_START_REGEX = /^(graph\s+(LR|RL|TD|TB|BT|)|flowchart\s+(LR|RL|TD|TB|BT|)|sequenceDiagram|classDiagram|stateDiagram(-v2)?|erDiagram|journey|gantt|pie|quadrantChart|requirementDiagram|gitGraph|mindmap|timeline|sankey(-beta)?|block(-beta)?|architecture(-beta)?|c4(Context|Container|Component|Dynamic|Deployment)|zenuml)(\s|$)/i;

export function isMermaidCode(text: string): boolean {
  if (!text) return false;
  const trimmed = text.trim();
  const firstLine = trimmed.split('\n')[0].trim();
  return MERMAID_START_REGEX.test(firstLine);
}

// Helper to determine human-friendly diagram name
function getDiagramLabel(code: string): string {
  const firstLine = code.trim().split('\n')[0].trim().toLowerCase();
  if (firstLine.startsWith('graph') || firstLine.startsWith('flowchart')) {
    const dir = firstLine.match(/\b(lr|rl|td|tb|bt)\b/i)?.[1]?.toUpperCase() || '';
    return dir ? `Flowchart (${dir})` : 'Flowchart';
  }
  if (firstLine.startsWith('sequencediagram')) return 'Sequence Diagram';
  if (firstLine.startsWith('classdiagram')) return 'Class Diagram';
  if (firstLine.startsWith('statediagram')) return 'State Diagram';
  if (firstLine.startsWith('erdiagram')) return 'ER Diagram';
  if (firstLine.startsWith('mindmap')) return 'Mind Map';
  if (firstLine.startsWith('gantt')) return 'Gantt Chart';
  if (firstLine.startsWith('pie')) return 'Pie Chart';
  if (firstLine.startsWith('gitgraph')) return 'Git Graph';
  if (firstLine.startsWith('timeline')) return 'Timeline';
  if (firstLine.startsWith('c4')) return 'C4 Architecture';
  return 'Mermaid Diagram';
}

interface MermaidBlockProps {
  code: string;
}

// Helper to sanitize common LLM Mermaid syntax glitches
export function sanitizeMermaidCode(code: string): string {
  let cleaned = code.trim();
  if (!cleaned) return cleaned;

  // 1. Fix LLM edge label syntax glitch: -->|label text|> B  ==>  -->|label text| B
  cleaned = cleaned.replace(/(-->|---|==>|-\.->)\s*\|([^|\n]+)\|>\s*/g, '$1|$2| ');

  // 2. Fix backticks inside bracketed node labels: [Node `text` contract] ==> [Node 'text' contract]
  cleaned = cleaned.replace(/\[([^\]\n]+)\]/g, (_match, inner) => {
    const sanitizedInner = inner.replace(/`/g, "'");
    return `[${sanitizedInner}]`;
  });

  return cleaned;
}

export const MermaidBlock: React.FC<MermaidBlockProps> = ({ code }) => {
  const [svgContent, setSvgContent] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showSource, setShowSource] = useState(false);
  const [copied, setCopied] = useState(false);
  const uniqueId = useId().replace(/[^a-zA-Z0-9_-]/g, 'm');

  useEffect(() => {
    let isMounted = true;

    const renderDiagram = async () => {
      try {
        setError(null);
        const cleanCode = sanitizeMermaidCode(code);
        if (!cleanCode) return;

        const isDark = document.documentElement.classList.contains('dark');

        // Configure Mermaid with theme corresponding to active dark/light mode
        mermaid.initialize({
          startOnLoad: false,
          theme: isDark ? 'dark' : 'default',
          themeVariables: isDark
            ? {
                darkMode: true,
                background: '#18181b',
                primaryColor: '#3b82f6',
                primaryTextColor: '#f8fafc',
                primaryBorderColor: '#3b82f6',
                lineColor: '#94a3b8',
                secondaryColor: '#27272a',
                tertiaryColor: '#1e293b',
                fontFamily: 'ui-sans-serif, system-ui, -apple-system, sans-serif',
                fontSize: '12px',
              }
            : {
                darkMode: false,
                background: '#ffffff',
                primaryColor: '#2563eb',
                primaryTextColor: '#0f172a',
                primaryBorderColor: '#2563eb',
                lineColor: '#64748b',
                secondaryColor: '#f1f5f9',
                tertiaryColor: '#e2e8f0',
                fontFamily: 'ui-sans-serif, system-ui, -apple-system, sans-serif',
                fontSize: '12px',
              },
          securityLevel: 'loose',
          flowchart: {
            useMaxWidth: true,
            htmlLabels: true,
            curve: 'basis',
          },
        });

        const renderId = `m_${uniqueId}_${Date.now()}`;
        const { svg } = await mermaid.render(renderId, cleanCode);
        if (isMounted) {
          setSvgContent(svg);
        }
      } catch (err: any) {
        if (isMounted) {
          console.warn('[MermaidBlock] Render error:', err);
          setError(err?.message || 'Could not parse Mermaid diagram syntax.');
        }
      }
    };

    renderDiagram();

    return () => {
      isMounted = false;
    };
  }, [code, uniqueId]);

  const handleCopy = () => {
    navigator.clipboard.writeText(code.trim());
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const diagramTitle = getDiagramLabel(code);

  if (error) {
    return (
      <div className="my-3 rounded-lg border border-destructive/30 bg-destructive/5 overflow-hidden shadow-xs">
        <div className="flex items-center justify-between px-3 py-2 bg-destructive/10 border-b border-destructive/20 text-xs">
          <div className="flex items-center gap-1.5 font-medium text-destructive">
            <AlertTriangle className="w-3.5 h-3.5" />
            <span>Diagram Syntax Error</span>
          </div>
          <Button
            size="sm"
            variant="ghost"
            onClick={handleCopy}
            className="h-6 px-2 text-[10px] gap-1 text-destructive hover:bg-destructive/10"
          >
            {copied ? <Check className="w-3 h-3 text-emerald-500" /> : <Copy className="w-3 h-3" />}
            <span>{copied ? 'Copied' : 'Copy'}</span>
          </Button>
        </div>
        <div className="p-3 space-y-2">
          <p className="text-[11px] text-destructive/90 font-mono leading-relaxed line-clamp-2">
            {error}
          </p>
          <pre className="p-2.5 rounded-lg bg-background/80 border border-border font-mono text-[11px] text-foreground overflow-x-auto whitespace-pre-wrap">
            {code.trim()}
          </pre>
        </div>
      </div>
    );
  }

  return (
    <div className="my-3 rounded-lg border border-border/80 bg-card/90 backdrop-blur-xs overflow-hidden shadow-xs group transition-all duration-200 hover:border-primary/40">
      {/* Header bar */}
      <div className="flex items-center justify-between px-3 py-1.5 bg-muted/40 border-b border-border/60 text-xs select-none">
        <div className="flex items-center gap-2">
          <div className="p-1 rounded-md bg-primary/10 text-primary flex items-center justify-center">
            <GitFork className="w-3.5 h-3.5" />
          </div>
          <span className="text-xs font-semibold text-foreground tracking-tight">{diagramTitle}</span>
          <Badge variant="outline" className="text-[9px] font-mono h-4 px-1.5 bg-background/50 border-border/60 text-muted-foreground">
            Mermaid
          </Badge>
        </div>

        <div className="flex items-center gap-1">
          <Button
            size="sm"
            variant="ghost"
            onClick={() => setShowSource(!showSource)}
            className="h-6 px-2 text-[11px] gap-1 text-muted-foreground hover:text-foreground hover:bg-background/80 transition-colors"
            title={showSource ? 'Show Rendered Diagram' : 'Show Source Code'}
          >
            {showSource ? (
              <>
                <Eye className="w-3 h-3 text-primary" />
                <span>Visual</span>
              </>
            ) : (
              <>
                <Code2 className="w-3 h-3" />
                <span>Source</span>
              </>
            )}
          </Button>

          <Button
            size="sm"
            variant="ghost"
            onClick={handleCopy}
            className="h-6 px-2 text-[11px] gap-1 text-muted-foreground hover:text-foreground hover:bg-background/80 transition-colors"
            title="Copy Mermaid syntax"
          >
            {copied ? (
              <>
                <Check className="w-3 h-3 text-emerald-500" />
                <span className="text-emerald-500">Copied</span>
              </>
            ) : (
              <>
                <Copy className="w-3 h-3" />
                <span>Copy</span>
              </>
            )}
          </Button>
        </div>
      </div>

      {/* Body: Rendered SVG or Source View */}
      {showSource ? (
        <div className="p-3 bg-muted/20 overflow-x-auto font-mono text-[11px] text-foreground leading-relaxed whitespace-pre-wrap">
          <pre>{code.trim()}</pre>
        </div>
      ) : !svgContent ? (
        <div className="p-6 flex items-center justify-center min-h-[80px] text-xs text-muted-foreground animate-pulse gap-2">
          <Sparkles className="w-3.5 h-3.5 text-primary animate-spin" />
          <span>Rendering diagram…</span>
        </div>
      ) : (
        <div
          className="p-4 flex items-center justify-center overflow-x-auto bg-card/60 [&>svg]:max-w-full [&>svg]:h-auto [&>svg]:mx-auto"
          dangerouslySetInnerHTML={{ __html: svgContent }}
        />
      )}
    </div>
  );
};

export const MarkdownView: React.FC<MarkdownViewProps> = ({ content, className = '' }) => {
  if (!content) return null;

  const lines = content.split('\n');
  const elements: React.ReactNode[] = [];

  let inCodeBlock = false;
  let codeBlockType = '';
  let codeBlockBuffer: string[] = [];

  let inRawDiagram = false;
  let rawDiagramBuffer: string[] = [];

  // Track if we are inside a numbered section context to indent child bullet points
  let inNumberedContext = false;

  const flushRawDiagram = (indexKey: string) => {
    if (rawDiagramBuffer.length > 0) {
      const code = rawDiagramBuffer.join('\n');
      elements.push(<MermaidBlock key={`raw_mermaid_${indexKey}`} code={code} />);
      rawDiagramBuffer = [];
      inRawDiagram = false;
    }
  };

  for (let i = 0; i < lines.length; i++) {
    const rawLine = lines[i];
    const trimmed = rawLine.trim();

    // Calculate leading whitespace indentation (e.g. 2 spaces, 4 spaces, tabs)
    const leadingWhitespaceMatch = rawLine.match(/^(\s+)/);
    const leadingSpacesCount = leadingWhitespaceMatch
      ? leadingWhitespaceMatch[1].replace(/\t/g, '  ').length
      : 0;
    const explicitIndentLevel = Math.floor(leadingSpacesCount / 2);

    // 1. Triple backtick code block boundary
    if (trimmed.startsWith('```')) {
      flushRawDiagram(`flush_${i}`);
      inNumberedContext = false;

      if (inCodeBlock) {
        // Closing code block
        const code = codeBlockBuffer.join('\n');
        const isMermaid =
          codeBlockType === 'mermaid' ||
          codeBlockType === 'flowchart' ||
          codeBlockType === 'graph' ||
          codeBlockType === 'diagram' ||
          codeBlockType === 'mmd' ||
          codeBlockType === 'sequence' ||
          codeBlockType === 'mindmap' ||
          isMermaidCode(code);

        if (isMermaid) {
          elements.push(<MermaidBlock key={`mermaid_${i}`} code={code} />);
        } else {
          elements.push(
            <div key={`code_${i}`} className="my-2 p-3 rounded-lg bg-muted/40 border border-border font-mono text-[11px] overflow-x-auto">
              <pre className="text-foreground whitespace-pre-wrap">{code}</pre>
            </div>
          );
        }
        inCodeBlock = false;
        codeBlockType = '';
        codeBlockBuffer = [];
      } else {
        // Opening code block
        inCodeBlock = true;
        codeBlockType = trimmed.replace(/^```+/, '').trim().toLowerCase();
        codeBlockBuffer = [];
      }
      continue;
    }

    if (inCodeBlock) {
      codeBlockBuffer.push(rawLine);
      continue;
    }

    // 2. Unenclosed Raw Mermaid diagram detection
    if (!inRawDiagram && MERMAID_START_REGEX.test(trimmed)) {
      inRawDiagram = true;
      inNumberedContext = false;
      rawDiagramBuffer.push(rawLine);
      continue;
    }

    if (inRawDiagram) {
      const isBlockTerminator =
        !trimmed ||
        trimmed.startsWith('### ') ||
        trimmed.startsWith('## ') ||
        trimmed.startsWith('# ') ||
        trimmed.startsWith('- ') ||
        trimmed.startsWith('* ') ||
        trimmed.startsWith('• ') ||
        /^\d+\.\s+/.test(trimmed);

      if (isBlockTerminator) {
        flushRawDiagram(`${i}`);
      } else {
        rawDiagramBuffer.push(rawLine);
        continue;
      }
    }

    if (!trimmed) {
      continue;
    }

    // 3. Bullet points: "-", "*", "•"
    if (trimmed.startsWith('- ') || trimmed.startsWith('* ') || trimmed.startsWith('• ')) {
      const itemText = trimmed.replace(/^[-*•]\s+/, '');

      // Determine indentation class:
      // - If inside a numbered section (e.g. under "1. Refactor Meeting Interface:"), indent with ml-5.
      // - If explicit indent level (e.g. 2 spaces, 4 spaces), indent accordingly.
      let indentClass = 'ml-1';
      let isSubBullet = false;

      if (inNumberedContext) {
        if (explicitIndentLevel >= 2) {
          indentClass = 'ml-10';
          isSubBullet = true;
        } else if (explicitIndentLevel === 1) {
          indentClass = 'ml-7';
          isSubBullet = true;
        } else {
          indentClass = 'ml-5';
        }
      } else if (explicitIndentLevel >= 2) {
        indentClass = 'ml-8';
        isSubBullet = true;
      } else if (explicitIndentLevel === 1) {
        indentClass = 'ml-4';
        isSubBullet = true;
      }

      elements.push(
        <div
          key={`bullet_${i}`}
          className={`flex items-start gap-2.5 text-xs leading-relaxed text-foreground ${indentClass}`}
        >
          {isSubBullet ? (
            <span className="w-1.5 h-1.5 rounded-full border border-primary/70 bg-primary/20 mt-1.5 shrink-0" />
          ) : (
            <span className="w-1.5 h-1.5 rounded-full bg-primary mt-1.5 shrink-0" />
          )}
          <div className="flex-1">{renderFormattedText(itemText)}</div>
        </div>
      );
      continue;
    }

    // 4. Numbered lists: "1. ", "2. ", "3. "
    const numberedMatch = trimmed.match(/^(\d+)\.\s+(.*)$/);
    if (numberedMatch) {
      inNumberedContext = true;
      const num = numberedMatch[1];
      const itemText = numberedMatch[2];
      elements.push(
        <div key={`num_${i}`} className="flex items-start gap-2 text-xs leading-relaxed text-foreground mt-2 font-medium">
          <Badge
            variant="outline"
            className="text-[10px] font-mono font-semibold h-4.5 min-w-4.5 px-1 flex items-center justify-center shrink-0 mt-0.5 bg-primary/10 border-primary/30 text-primary"
          >
            {num}
          </Badge>
          <div className="flex-1">{renderFormattedText(itemText)}</div>
        </div>
      );
      continue;
    }

    // 5. Headings
    if (trimmed.startsWith('### ')) {
      inNumberedContext = false;
      elements.push(
        <h4 key={`h3_${i}`} className="text-xs font-bold text-foreground mt-3 mb-1 tracking-tight">
          {trimmed.replace(/^###\s+/, '')}
        </h4>
      );
      continue;
    }
    if (trimmed.startsWith('## ')) {
      inNumberedContext = false;
      elements.push(
        <h3 key={`h2_${i}`} className="text-sm font-bold text-foreground mt-3 mb-1 tracking-tight">
          {trimmed.replace(/^##\s+/, '')}
        </h3>
      );
      continue;
    }
    if (trimmed.startsWith('# ')) {
      inNumberedContext = false;
      elements.push(
        <h2 key={`h1_${i}`} className="text-base font-bold text-foreground mt-4 mb-1.5 tracking-tight">
          {trimmed.replace(/^#\s+/, '')}
        </h2>
      );
      continue;
    }

    // Non-list paragraph resets numbered context
    inNumberedContext = false;

    // 6. Standard paragraph
    elements.push(
      <p key={`p_${i}`} className="text-xs leading-relaxed text-foreground">
        {renderFormattedText(trimmed)}
      </p>
    );
  }

  // Flush any open raw diagram at end of content
  flushRawDiagram('end');

  if (inCodeBlock && codeBlockBuffer.length > 0) {
    const code = codeBlockBuffer.join('\n');
    const isMermaid =
      codeBlockType === 'mermaid' ||
      codeBlockType === 'flowchart' ||
      codeBlockType === 'graph' ||
      codeBlockType === 'diagram' ||
      codeBlockType === 'mmd' ||
      isMermaidCode(code);

    if (isMermaid) {
      elements.push(<MermaidBlock key="mermaid_end" code={code} />);
    } else {
      elements.push(
        <div key="code_end" className="my-2 p-3 rounded-lg bg-muted/40 border border-border font-mono text-[11px] overflow-x-auto">
          <pre className="text-foreground whitespace-pre-wrap">{code}</pre>
        </div>
      );
    }
  }

  return <div className={`space-y-1.5 ${className}`}>{elements}</div>;
};

// Inline formatter for bold (**text**), italic (*text*), and code (`code`)
function renderFormattedText(text: string): React.ReactNode[] {
  const parts: React.ReactNode[] = [];
  const regex = /(\*\*.*?\*\*|\*.*?\*|`.*?`)/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = regex.exec(text)) !== null) {
    if (match.index > lastIndex) {
      parts.push(text.substring(lastIndex, match.index));
    }

    const token = match[0];
    if (token.startsWith('**') && token.endsWith('**')) {
      parts.push(
        <strong key={`b_${match.index}`} className="font-semibold text-foreground">
          {token.slice(2, -2)}
        </strong>
      );
    } else if (token.startsWith('`') && token.endsWith('`')) {
      parts.push(
        <code key={`c_${match.index}`} className="px-1.5 py-0.5 rounded bg-muted font-mono text-[11px] text-primary border border-border/50">
          {token.slice(1, -1)}
        </code>
      );
    } else if (token.startsWith('*') && token.endsWith('*')) {
      parts.push(
        <em key={`i_${match.index}`} className="italic text-muted-foreground">
          {token.slice(1, -1)}
        </em>
      );
    }

    lastIndex = match.index + token.length;
  }

  if (lastIndex < text.length) {
    parts.push(text.substring(lastIndex));
  }

  return parts;
}
