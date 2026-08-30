"use client";

import React from "react";
import { Loader2, Sparkles } from "lucide-react";
import { cn } from "@/lib/utils";

export const LoadingSpinner = ({
  size = "md",
  className = "",
}: {
  size?: "sm" | "md" | "lg";
  simple?: boolean;
  className?: string;
}) => {
  const sizeClasses = {
    sm: "w-5 h-5",
    md: "w-10 h-10",
    lg: "w-16 h-16",
  };

  const iconSizes = {
    sm: "w-4 h-4",
    md: "w-6 h-6",
    lg: "w-10 h-10",
  };

  return (
    <div className={cn("relative flex items-center justify-center", sizeClasses[size], className)}>
      <div
        className={cn(
          "absolute inset-0 rounded-full border-2 border-muted border-t-primary animate-spin"
        )}
      />
      <Sparkles className={cn("text-primary/70 animate-pulse", iconSizes[size])} />
    </div>
  );
};

export const LoadingView = ({ fullScreen = true }: { fullScreen?: boolean }) => {
  return (
    <div
      className={cn(
        "flex items-center justify-center bg-background text-foreground animate-in fade-in duration-300",
        fullScreen ? "h-screen w-screen" : "h-full min-h-[400px] w-full"
      )}
    >
      <div className="flex flex-col items-center gap-5 text-center">
        <LoadingSpinner size="lg" />
        <div className="space-y-1.5">
          <h3 className="text-xl font-extrabold tracking-tight text-foreground">
            Relay
          </h3>
          <p className="text-xs text-muted-foreground font-mono tracking-wider uppercase animate-pulse">
            Loading your vault…
          </p>
        </div>
      </div>
    </div>
  );
};

export default LoadingView;


