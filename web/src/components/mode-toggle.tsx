"use client";

import * as React from "react";
import { Moon, Sun } from "lucide-react";
import { useTheme } from "next-themes";
import { Button } from "@/components/ui/button";

export function ModeToggle() {
  const { resolvedTheme, setTheme } = useTheme();
  const [mounted, setMounted] = React.useState(false);

  React.useEffect(() => {
    setMounted(true);
  }, []);

  if (!mounted) {
    return (
      <Button
        variant="outline"
        size="icon"
        className="h-8 w-8 rounded-lg border border-border/80 bg-card text-muted-foreground shadow-xs"
        aria-label="Toggle theme"
      >
        <Moon className="h-4 w-4" />
      </Button>
    );
  }

  const isLight = resolvedTheme !== "dark";

  const toggleTheme = () => {
    setTheme(isLight ? "dark" : "light");
  };

  return (
    <Button
      variant="outline"
      size="icon"
      onClick={toggleTheme}
      className={`h-8 w-8 rounded-lg border transition-all duration-200 cursor-pointer shadow-xs group ${
        isLight
          ? "bg-card border-border/80 hover:bg-muted/80 hover:border-border text-slate-700"
          : "bg-card/90 border-border/60 hover:bg-muted/50 hover:border-border text-amber-400"
      }`}
      title={isLight ? "Switch to Dark mode" : "Switch to Light mode"}
      aria-label={isLight ? "Switch to Dark mode" : "Switch to Light mode"}
    >
      {isLight ? (
        <Moon className="h-4 w-4 text-slate-700 transition-transform duration-300 group-hover:scale-110 group-hover:-rotate-12" />
      ) : (
        <Sun className="h-4 w-4 text-amber-400 transition-transform duration-300 group-hover:scale-110 group-hover:rotate-45" />
      )}
    </Button>
  );
}
