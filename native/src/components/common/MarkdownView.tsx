import React, { useEffect, useRef, useState, useId } from 'react';
import mermaid from 'mermaid';
import { Badge } from '@/components/ui/badge';
import { Code2, AlertTriangle } from 'lucide-react';

interface MarkdownViewProps {
  content: string;
  className?: string;
}

// Initialize Mermaid once with sleek dark theme matching Relay
mermaid.initialize({
  startOnLoad: false,
  theme: 'dark',
  themeVariables: {
    darkMode: true,
    background: '#18181b',
    primaryColor: '#3b82f6',
    primaryTextColor: '#f8fafc',
    primaryBorderColor: '#3b82f6',
    lineColor: '#64748b',
    secondaryColor: '#27272a',
    tertiaryColor: '#1e293b',
    fontFamily: 'ui-sans-serif, system-ui, sans-serif',
    fontSize: '11px',
  },
  securityLevel: 'loose',
  flowchart: {
    useMaxWidth: true,
    htmlLabels: true,
    curve: 'basis',
  },
});

interface MermaidBlockProps {
  code: string;
}

const MermaidBlock: React.FC<MermaidBlockProps> = ({ code }) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const [svgContent, setSvgContent] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const uniqueId = useId().replace(/[^a-zA-Z0-9_-]/g, 'm');

  useEffect(() => {
    let isMounted = true;
    const renderDiagram = async () => {
      try {
        setError(null);
        const cleanCode = code.trim();
        if (!cleanCode) return;
        const { svg } = await mermaid.render(`mermaid_${uniqueId}_${Date.now()}`, cleanCode);
        if (isMounted) {
          setSvgContent(svg);
        }
      } catch (err: any) {
        if (isMounted) {
          console.warn('Mermaid render error:', err);
          setError(err?.message || 'Could not render diagram');
        }
      }
    };

    renderDiagram();
    return () => {
      isMounted = false;
    };
  }, [code, uniqueId]);

  if (error) {
    return (
      <div className="my-2 p-2.5 rounded-lg bg-muted/30 border border-border text-[11px] font-mono text-muted-foreground overflow-x-auto space-y-1">
        <div className="flex items-center gap-1.5 text-amber-500 font-sans text-xs">
          <AlertTriangle className="w-3.5 h-3.5" />
          <span>Diagram Source</span>
        </div>
        <pre className="text-[10px] leading-relaxed text-foreground whitespace-pre-wrap">{code}</pre>
      </div>
    );
  }

  if (!svgContent) {
    return (
      <div className="my-2 p-4 rounded-lg bg-card/60 border border-border/60 flex items-center justify-center min-h-[60px] text-xs text-muted-foreground animate-pulse">
        Rendering diagram…
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      className="my-2.5 p-3 rounded-lg bg-card/90 border border-border/80 flex items-center justify-center overflow-x-auto shadow-xs [&>svg]:max-w-full [&>svg]:h-auto"
      dangerouslySetInnerHTML={{ __html: svgContent }}
    />
  );
};

export const MarkdownView: React.FC<MarkdownViewProps> = ({ content, className = '' }) => {
  if (!content) return null;

  // Split by code blocks or lines
  const lines = content.split('\n');
  const elements: React.ReactNode[] = [];
  let inCodeBlock = false;
  let codeBlockType = '';
  let codeBlockBuffer: string[] = [];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const trimmed = line.trim();

    if (trimmed.startsWith('```')) {
      if (inCodeBlock) {
        // End of code block
        const code = codeBlockBuffer.join('\n');
        if (codeBlockType === 'mermaid' || codeBlockType === 'flowchart') {
          elements.push(<MermaidBlock key={`mermaid_${i}`} code={code} />);
        } else {
          elements.push(
            <div key={`code_${i}`} className="my-2 p-2.5 rounded-lg bg-muted/40 border border-border font-mono text-[11px] overflow-x-auto">
              <pre className="text-foreground whitespace-pre-wrap">{code}</pre>
            </div>
          );
        }
        inCodeBlock = false;
        codeBlockType = '';
        codeBlockBuffer = [];
      } else {
        // Start of code block
        inCodeBlock = true;
        codeBlockType = trimmed.replace('```', '').trim().toLowerCase();
        codeBlockBuffer = [];
      }
      continue;
    }

    if (inCodeBlock) {
      codeBlockBuffer.push(line);
      continue;
    }

    if (!trimmed) {
      continue;
    }

    // Bullet points: "-", "*", "•"
    if (trimmed.startsWith('- ') || trimmed.startsWith('* ') || trimmed.startsWith('• ')) {
      const itemText = trimmed.replace(/^[-*•]\s+/, '');
      elements.push(
        <div key={`bullet_${i}`} className="flex items-start gap-2 text-xs leading-relaxed text-foreground">
          <span className="w-1.5 h-1.5 rounded-full bg-primary mt-1.5 shrink-0" />
          <div className="flex-1">{renderFormattedText(itemText)}</div>
        </div>
      );
      continue;
    }

    // Numbered lists: "1. ", "2. "
    const numberedMatch = trimmed.match(/^(\d+)\.\s+(.*)$/);
    if (numberedMatch) {
      const num = numberedMatch[1];
      const itemText = numberedMatch[2];
      elements.push(
        <div key={`num_${i}`} className="flex items-start gap-2 text-xs leading-relaxed text-foreground">
          <Badge variant="outline" className="text-[9px] font-mono h-4 w-4 p-0 flex items-center justify-center shrink-0 mt-0.5 bg-muted">
            {num}
          </Badge>
          <div className="flex-1">{renderFormattedText(itemText)}</div>
        </div>
      );
      continue;
    }

    // Headings
    if (trimmed.startsWith('### ')) {
      elements.push(
        <h4 key={`h3_${i}`} className="text-xs font-bold text-foreground mt-2 mb-1 tracking-tight">
          {trimmed.replace(/^###\s+/, '')}
        </h4>
      );
      continue;
    }
    if (trimmed.startsWith('## ')) {
      elements.push(
        <h3 key={`h2_${i}`} className="text-sm font-bold text-foreground mt-2 mb-1 tracking-tight">
          {trimmed.replace(/^##\s+/, '')}
        </h3>
      );
      continue;
    }

    // Standard paragraph
    elements.push(
      <p key={`p_${i}`} className="text-xs leading-relaxed text-foreground">
        {renderFormattedText(trimmed)}
      </p>
    );
  }

  // Flush any unclosed code block
  if (inCodeBlock && codeBlockBuffer.length > 0) {
    const code = codeBlockBuffer.join('\n');
    if (codeBlockType === 'mermaid' || codeBlockType === 'flowchart') {
      elements.push(<MermaidBlock key="mermaid_end" code={code} />);
    } else {
      elements.push(
        <div key="code_end" className="my-2 p-2.5 rounded-lg bg-muted/40 border border-border font-mono text-[11px] overflow-x-auto">
          <pre className="text-foreground whitespace-pre-wrap">{code}</pre>
        </div>
      );
    }
  }

  return <div className={`space-y-2 ${className}`}>{elements}</div>;
};

// Inline formatter for bold (**text**), italic (*text*), and code (`code`)
function renderFormattedText(text: string): React.ReactNode[] {
  // Regex to match **bold**, *italic*, `code`
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
