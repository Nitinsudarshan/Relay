import React, { useState, useEffect } from 'react';
import { Sun, Moon } from 'lucide-react';
import { Button } from '@/components/ui/button';

export type ThemeMode = 'light' | 'dark';

export const ThemeToggle: React.FC = () => {
  const [theme, setTheme] = useState<ThemeMode>(() => {
    const saved = localStorage.getItem('relay-theme');
    if (saved === 'dark' || saved === 'light') {
      return saved;
    }
    return typeof window !== 'undefined' &&
      window.matchMedia('(prefers-color-scheme: dark)').matches
      ? 'dark'
      : 'light';
  });

  useEffect(() => {
    const isDark = theme === 'dark';
    if (isDark) {
      document.documentElement.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
    }
    localStorage.setItem('relay-theme', theme);

    try {
      import('@tauri-apps/api/event')
        .then(({ emit }) => {
          emit('relay-theme-changed', theme).catch(() => {});
        })
        .catch(() => {});
    } catch {}
  }, [theme]);

  const toggleTheme = () => {
    setTheme((prev) => (prev === 'light' ? 'dark' : 'light'));
  };

  const isLight = theme === 'light';

  return (
    <Button
      variant="outline"
      size="icon"
      onClick={toggleTheme}
      className={`h-8 w-8 rounded-lg border transition-all duration-200 cursor-pointer shadow-xs group ${
        isLight
          ? 'bg-card border-border/80 hover:bg-muted/80 hover:border-border text-slate-700'
          : 'bg-card/90 border-border/60 hover:bg-muted/50 hover:border-border text-amber-400'
      }`}
      title={isLight ? 'Switch to Dark mode' : 'Switch to Light mode'}
      aria-label={isLight ? 'Switch to Dark mode' : 'Switch to Light mode'}
    >
      {isLight ? (
        <Moon className="h-4 w-4 text-slate-700 transition-transform duration-300 group-hover:scale-110 group-hover:-rotate-12" />
      ) : (
        <Sun className="h-4 w-4 text-amber-400 transition-transform duration-300 group-hover:scale-110 group-hover:rotate-45" />
      )}
    </Button>
  );
};

