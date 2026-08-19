import React, { useState, useEffect, useRef } from 'react';
import { Sun, Moon, Monitor, Check } from 'lucide-react';
import { Button } from '@/components/ui/button';

export type ThemeMode = 'light' | 'dark' | 'system';

export const ThemeToggle: React.FC = () => {
  const [theme, setTheme] = useState<ThemeMode>(() => {
    const saved = localStorage.getItem('relay-theme') as ThemeMode;
    return saved && ['light', 'dark', 'system'].includes(saved) ? saved : 'system';
  });

  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const applyTheme = (currentTheme: ThemeMode) => {
      const isSystemDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
      const effectiveDark = currentTheme === 'dark' || (currentTheme === 'system' && isSystemDark);

      if (effectiveDark) {
        document.documentElement.classList.add('dark');
      } else {
        document.documentElement.classList.remove('dark');
      }
    };

    applyTheme(theme);
    localStorage.setItem('relay-theme', theme);

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handleSystemChange = () => {
      if (theme === 'system') {
        applyTheme('system');
      }
    };

    mediaQuery.addEventListener('change', handleSystemChange);
    return () => mediaQuery.removeEventListener('change', handleSystemChange);
  }, [theme]);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const handleSelectTheme = (mode: ThemeMode) => {
    setTheme(mode);
    setIsOpen(false);
  };

  return (
    <div className="relative inline-block text-left" ref={dropdownRef}>
      <Button
        variant="ghost"
        size="icon"
        className="h-8 w-8 text-muted-foreground hover:text-foreground relative"
        onClick={() => setIsOpen(!isOpen)}
        aria-label="Toggle theme settings"
      >
        <Sun className="h-4 w-4 rotate-0 scale-100 transition-all dark:-rotate-90 dark:scale-0" />
        <Moon className="absolute h-4 w-4 rotate-90 scale-0 transition-all dark:rotate-0 dark:scale-100" />
      </Button>

      {isOpen && (
        <div className="absolute right-0 mt-2 w-36 rounded-xl border border-border bg-popover p-1 shadow-lg z-50 text-xs animate-in fade-in-50 zoom-in-95">
          <button
            onClick={() => handleSelectTheme('light')}
            className={`w-full flex items-center justify-between px-2.5 py-1.5 rounded-md transition-colors ${
              theme === 'light' ? 'bg-accent text-accent-foreground font-semibold' : 'text-popover-foreground hover:bg-muted'
            }`}
          >
            <div className="flex items-center gap-2">
              <Sun className="w-3.5 h-3.5 text-amber-500" />
              <span>Light</span>
            </div>
            {theme === 'light' && <Check className="w-3.5 h-3.5 text-primary" />}
          </button>

          <button
            onClick={() => handleSelectTheme('dark')}
            className={`w-full flex items-center justify-between px-2.5 py-1.5 rounded-md transition-colors ${
              theme === 'dark' ? 'bg-accent text-accent-foreground font-semibold' : 'text-popover-foreground hover:bg-muted'
            }`}
          >
            <div className="flex items-center gap-2">
              <Moon className="w-3.5 h-3.5 text-indigo-400" />
              <span>Dark</span>
            </div>
            {theme === 'dark' && <Check className="w-3.5 h-3.5 text-primary" />}
          </button>

          <button
            onClick={() => handleSelectTheme('system')}
            className={`w-full flex items-center justify-between px-2.5 py-1.5 rounded-md transition-colors ${
              theme === 'system' ? 'bg-accent text-accent-foreground font-semibold' : 'text-popover-foreground hover:bg-muted'
            }`}
          >
            <div className="flex items-center gap-2">
              <Monitor className="w-3.5 h-3.5 text-blue-400" />
              <span>System</span>
            </div>
            {theme === 'system' && <Check className="w-3.5 h-3.5 text-primary" />}
          </button>
        </div>
      )}
    </div>
  );
};
