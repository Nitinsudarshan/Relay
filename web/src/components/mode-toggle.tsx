"use client"

import * as React from "react"
import { Moon, Sun } from "lucide-react"
import { useTheme } from "next-themes"

export function ModeToggle() {
    const { theme, setTheme, resolvedTheme } = useTheme()
    const [mounted, setMounted] = React.useState(false)

    React.useEffect(() => {
        setMounted(true)
    }, [])

    const currentTheme = resolvedTheme || theme || "light"

    const toggleTheme = () => {
        setTheme(currentTheme === "dark" ? "light" : "dark")
    }

    if (!mounted) {
        return (
            <div className="inline-flex items-center p-0.5 rounded-lg border border-border bg-card h-7.5 w-14 opacity-50" />
        )
    }

    return (
        <button
            type="button"
            onClick={toggleTheme}
            className="group h-8 w-8 rounded-lg border border-border bg-card hover:bg-muted/80 text-foreground transition-all duration-200 flex items-center justify-center cursor-pointer shadow-xs active:scale-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring select-none"
            aria-label={currentTheme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
            title={currentTheme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
        >
            {currentTheme === "dark" ? (
                <Sun
                    key="sun-icon"
                    className="w-4 h-4 text-amber-400 group-hover:text-amber-300 transition-all duration-300 ease-out transform group-hover:rotate-45 group-hover:scale-110 animate-in fade-in-50 zoom-in-75 spin-in-90"
                />
            ) : (
                <Moon
                    key="moon-icon"
                    className="w-4 h-4 text-indigo-500 group-hover:text-indigo-600 transition-all duration-300 ease-out transform group-hover:-rotate-12 group-hover:scale-110 animate-in fade-in-50 zoom-in-75 spin-in--45"
                />
            )}
        </button>
    )
}
