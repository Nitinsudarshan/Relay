"use client";

import * as React from "react";
import {
  BookOpen,
  LifeBuoy,
  Send,
  Kanban,
  Settings,
  Mic,
  FileText,
} from "lucide-react";

import { NavMain, NavItem } from "@/components/nav-main";
import { NavSecondary } from "@/components/nav-secondary";
import { RelayLogo } from "@/components/relay-logo";
import {
  Sidebar,
  SidebarContent,
  SidebarHeader,
  SidebarFooter,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  useSidebar,
} from "@/components/ui/sidebar";
import { Badge } from "@/components/ui/badge";
import { ShieldCheck, Activity } from "lucide-react";
import Link from "next/link";

import { ChangelogDialog } from "@/components/changelog-dialog";

const data = {
  navSecondary: [
    {
      title: "Documentation",
      url: "/docs",
      icon: BookOpen,
    },
    {
      title: "Support",
      url: "#",
      icon: LifeBuoy,
    },
    {
      title: "Feedback",
      url: "#",
      icon: Send,
    },
  ],
};

export function AppSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
  const { setOpenMobile, isMobile } = useSidebar();
  const [changelogOpen, setChangelogOpen] = React.useState(false);

  const navGeneral: NavItem[] = [
    {
      title: "Kanban Board",
      url: "/",
      icon: Kanban,
      isActive: true,
    },
    {
      title: "Vault Notes",
      url: "/notes",
      icon: FileText,
    },
    {
      title: "Settings",
      url: "/settings",
      icon: Settings,
    },
  ];

  return (
    <>
      <Sidebar
        className="top-(--header-height) h-[calc(100svh-var(--header-height))]!"
        {...props}
      >
        <SidebarHeader>
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton size="lg" asChild>
                <Link 
                  href="/"
                  onClick={() => {
                    if (isMobile) setOpenMobile(false);
                  }}
                >
                  <div className="flex aspect-square size-8 items-center justify-center rounded-lg bg-card border border-border text-foreground font-bold shadow-xs">
                    <RelayLogo className="size-5" />
                  </div>
                  <div className="grid flex-1 text-left text-sm leading-tight">
                    <span className="truncate font-bold tracking-wider">RELAY</span>
                    <span className="truncate text-[10px] text-muted-foreground font-mono uppercase tracking-widest">
                      Hybrid Dashboard
                    </span>
                  </div>
                </Link>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarHeader>
        <SidebarContent>
          <NavMain items={navGeneral} label="MENU" />
          <NavSecondary items={data.navSecondary} className="mt-auto" />
        </SidebarContent>
        <SidebarFooter className="border-t border-border p-3 space-y-2">
          <div className="p-2.5 rounded-xl bg-card border border-border flex items-center gap-2.5 shadow-xs">
            <div className="w-7 h-7 rounded-full bg-primary text-primary-foreground font-bold flex items-center justify-center text-xs shrink-0">
              N
            </div>
            <div className="grid flex-1 leading-tight min-w-0">
              <span className="text-xs font-bold text-foreground truncate">Nitin Sudarshan</span>
              <span className="text-[10px] text-muted-foreground truncate">nitin@example.com</span>
            </div>
            <Badge variant="outline" className="text-[9px] font-mono px-1.5 py-0 border-primary/30 text-primary">
              Cloud
            </Badge>
          </div>
          <div className="flex items-center justify-between text-[10px] text-muted-foreground px-1">
            <div className="flex items-center gap-1.5 font-medium text-emerald-500">
              <ShieldCheck className="w-3.5 h-3.5" />
              <span>Hybrid Sync</span>
            </div>
            <button
              type="button"
              onClick={() => setChangelogOpen(true)}
              className="flex items-center gap-1 font-mono hover:text-primary transition-colors cursor-pointer group"
              title="View Release Notes & Changelog"
            >
              <Activity className="w-3 h-3 text-primary group-hover:animate-pulse" />
              <span className="underline decoration-dotted underline-offset-2">v0.3.4</span>
            </button>
          </div>
        </SidebarFooter>
      </Sidebar>

      <ChangelogDialog
        open={changelogOpen}
        onClose={() => setChangelogOpen(false)}
        currentVersion="0.3.4"
      />
    </>
  );
}
