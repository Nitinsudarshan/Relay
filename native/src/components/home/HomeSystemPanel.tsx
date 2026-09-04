import React from 'react';
import {
  AlertTriangle,
  Check,
  Cloud,
  Cpu,
  Database,
  FolderOpen,
  ScrollText,
  Volume2,
  Waves,
  type LucideIcon,
} from 'lucide-react';

import { Button } from '@/components/ui/button';

import type { SettingsSection } from '@/components/settings/ProviderSettings';
import type { AppSettings, RelayAccount, VaultLocationInfo } from '@/types';

export interface HomeSystemPanelProps {
  settings: AppSettings | null;
  account: RelayAccount | null;
  vaultLocation: VaultLocationInfo | null;
  appVersion: string;
  onOpenSettings: (section: SettingsSection) => void;
  onOpenChangelog: () => void;
}

interface SystemRow {
  id: string;
  icon: LucideIcon;
  label: string;
  /** What is actually configured, verbatim where a path or model name exists. */
  value: string;
  /** `null` for rows that are statements of fact rather than readiness checks. */
  ok: boolean | null;
  cta?: { label: string; onClick: () => void };
}

/** Last path segment, for a Windows or POSIX path. */
const basename = (path: string): string => path.split(/[\\/]/).filter(Boolean).pop() ?? path;

/** Display names for the provider ids in `ProviderSettings.active_provider`. */
const PROVIDER_LABELS: Record<string, string> = {
  ollama: 'Ollama',
  cloud_openai: 'OpenAI',
  cloud_gemini: 'Google Gemini',
  cloud_anthropic: 'Anthropic Claude',
};

/**
 * What Relay is running on, on this machine.
 *
 * Every row reports what is *configured*, and says so in those words — a
 * configured model is not a verified one, and this panel does not probe. Rows
 * that are not configured carry the way to fix them rather than a bare warning
 * (`rules/ui-components.md`, "No fake controls").
 */
export const HomeSystemPanel: React.FC<HomeSystemPanelProps> = ({
  settings,
  account,
  vaultLocation,
  appVersion,
  onOpenSettings,
  onOpenChangelog,
}) => {
  const provider = settings?.provider;
  const usingOllama = provider?.active_provider === 'ollama';
  const llmConfigured = usingOllama
    ? Boolean(provider?.ollama_model)
    : Boolean(provider?.cloud_api_key);
  const llmValue = !provider
    ? 'Settings unavailable'
    : usingOllama
      ? provider.ollama_model
        ? `${PROVIDER_LABELS.ollama} · ${provider.ollama_model}`
        : `${PROVIDER_LABELS.ollama} · no model selected`
      : `${PROVIDER_LABELS[provider.active_provider] ?? provider.active_provider} · ${
          provider.cloud_api_key ? provider.cloud_model || 'default model' : 'no API key'
        }`;

  const whisperPath = settings?.stt?.whisper_model_path;
  const ttsConfigured = Boolean(settings?.tts?.piper_binary_path && settings?.tts?.piper_voice_path);

  const rows: SystemRow[] = [
    {
      id: 'mode',
      icon: account?.authenticated ? Cloud : Database,
      label: 'Storage mode',
      value: account?.authenticated
        ? `Hybrid cloud sync · ${account.email ?? 'signed in'}`
        : 'Local vault · 100% on-device',
      ok: null,
      cta: { label: 'Account', onClick: () => onOpenSettings('account') },
    },
    {
      id: 'vault',
      icon: FolderOpen,
      label: 'Vault',
      value: vaultLocation
        ? `${vaultLocation.path}${vaultLocation.accessible ? '' : ' — unreachable'}`
        : 'Location unknown',
      ok: vaultLocation ? vaultLocation.accessible : null,
      cta: { label: 'Change', onClick: () => onOpenSettings('general') },
    },
    {
      id: 'llm',
      icon: Cpu,
      label: 'Language model',
      value: llmValue,
      ok: llmConfigured,
      cta: llmConfigured ? undefined : { label: 'Configure', onClick: () => onOpenSettings('advanced') },
    },
    {
      id: 'stt',
      icon: Waves,
      label: 'Speech-to-text',
      value: whisperPath ? `Whisper · ${basename(whisperPath)}` : 'No Whisper model selected',
      ok: Boolean(whisperPath),
      cta: whisperPath ? undefined : { label: 'Configure', onClick: () => onOpenSettings('advanced') },
    },
    {
      id: 'tts',
      icon: Volume2,
      label: 'Talkback voice',
      value: ttsConfigured ? 'Local speech engine installed' : 'Not installed — Talkback stays text-only',
      ok: ttsConfigured,
      cta: ttsConfigured ? undefined : { label: 'Install', onClick: () => onOpenSettings('talkback') },
    },
  ];

  return (
    <section className="space-y-2.5">
      <div className="flex items-center justify-between gap-2">
        <h2 className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground">
          Relay on this machine
        </h2>
        <Button
          size="sm"
          variant="ghost"
          onClick={onOpenChangelog}
          className="h-6 text-[10px] gap-1.5 px-2 text-muted-foreground hover:text-foreground"
        >
          <ScrollText className="w-3 h-3" />
          <span className="font-mono">v{appVersion} release notes</span>
        </Button>
      </div>

      <div className="rounded-lg border border-border bg-card divide-y divide-border">
        {rows.map((row) => {
          const Icon = row.icon;
          return (
            <div key={row.id} className="flex items-center gap-3 p-3">
              <Icon className="w-3.5 h-3.5 text-muted-foreground shrink-0" />

              <div className="min-w-0 flex-1">
                <p className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground">
                  {row.label}
                </p>
                <p className="text-xs text-foreground truncate" title={row.value}>
                  {row.value}
                </p>
              </div>

              {row.ok === true && <Check className="w-3.5 h-3.5 text-emerald-500 shrink-0" />}
              {row.ok === false && (
                <AlertTriangle className="w-3.5 h-3.5 text-amber-500 shrink-0" aria-label="Not configured" />
              )}

              {row.cta && (
                <Button
                  size="sm"
                  variant="outline"
                  onClick={row.cta.onClick}
                  aria-label={`${row.cta.label} — ${row.label}`}
                  className="h-7 text-[10px] px-2.5 shrink-0"
                >
                  {row.cta.label}
                </Button>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
};
