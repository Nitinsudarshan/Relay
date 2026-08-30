"use client";

import React from "react";
import { Sparkles } from "lucide-react";
import { cn } from "@/lib/utils";

export function MiniLoader({ className }: { className?: string }) {
  return (
    <div
      className={cn(
        "relative w-12 h-12 rounded-lg mx-auto flex items-center justify-center bg-card border border-border shadow-xs",
        className
      )}
    >
      <div className="absolute inset-1 rounded-full border-2 border-muted border-t-primary animate-spin" />
      <Sparkles className="w-5 h-5 text-primary animate-pulse" />
    </div>
  );
}

