"use client";

import * as React from "react";
import {
  BadgeCheck,
  Bell,
  ChevronsUpDown,
  CreditCard,
  LogOut,
  Sparkles,
  ShieldCheck,
  Activity,
  User,
} from "lucide-react";

import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@/components/ui/avatar";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  useSidebar,
} from "@/components/ui/sidebar";
import { Badge } from "@/components/ui/badge";

export interface UserData {
  name: string;
  email: string;
  avatar?: string;
  plan?: string;
  version?: string;
}

export function NavUser({
  user,
  onOpenChangelog,
}: {
  user: UserData;
  onOpenChangelog?: () => void;
}) {
  const { isMobile } = useSidebar();
  const initials = user.name
    ? user.name
        .split(" ")
        .map((n) => n[0])
        .join("")
        .toUpperCase()
        .slice(0, 2)
    : "U";

  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <SidebarMenuButton
              size="lg"
              className="data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground cursor-pointer"
            >
              <Avatar className="h-8 w-8 rounded-lg">
                {user.avatar && <AvatarImage src={user.avatar} alt={user.name} />}
                <AvatarFallback className="rounded-lg bg-primary/20 text-primary font-bold text-xs">
                  {initials}
                </AvatarFallback>
              </Avatar>
              <div className="grid flex-1 text-left text-sm leading-tight">
                <span className="truncate font-semibold">{user.name}</span>
                <span className="truncate text-xs text-muted-foreground">{user.email}</span>
              </div>
              <ChevronsUpDown className="ml-auto size-4 text-muted-foreground" />
            </SidebarMenuButton>
          </DropdownMenuTrigger>
          <DropdownMenuContent
            className="w-(--radix-dropdown-menu-trigger-width) min-w-60 rounded-xl p-1.5 shadow-lg border border-border"
            side={isMobile ? "bottom" : "right"}
            align="end"
            sideOffset={4}
          >
            <DropdownMenuLabel className="p-0 font-normal">
              <div className="flex items-center gap-2.5 px-2 py-2 text-left text-sm rounded-lg bg-muted/40">
                <Avatar className="h-8 w-8 rounded-lg">
                  {user.avatar && <AvatarImage src={user.avatar} alt={user.name} />}
                  <AvatarFallback className="rounded-lg bg-primary/20 text-primary font-bold text-xs">
                    {initials}
                  </AvatarFallback>
                </Avatar>
                <div className="grid flex-1 text-left text-sm leading-tight min-w-0">
                  <div className="flex items-center justify-between gap-1">
                    <span className="truncate font-bold text-foreground">{user.name}</span>
                    <Badge variant="outline" className="text-[9px] font-mono px-1 py-0 text-primary border-primary/30">
                      {user.plan || "Hybrid"}
                    </Badge>
                  </div>
                  <span className="truncate text-[11px] text-muted-foreground">{user.email}</span>
                </div>
              </div>
            </DropdownMenuLabel>
            
            <DropdownMenuSeparator className="my-1" />
            
            <DropdownMenuGroup>
              <DropdownMenuItem className="cursor-pointer gap-2 py-2 text-xs">
                <ShieldCheck className="size-4 text-emerald-500" />
                <div className="flex flex-col">
                  <span className="font-medium">Hybrid Cloud Sync</span>
                  <span className="text-[10px] text-muted-foreground">Connected to Supabase</span>
                </div>
              </DropdownMenuItem>
            </DropdownMenuGroup>
            
            <DropdownMenuSeparator className="my-1" />
            
            <DropdownMenuGroup>
              <DropdownMenuItem className="cursor-pointer gap-2 py-2 text-xs">
                <BadgeCheck className="size-4 text-muted-foreground" />
                <span>Account Profile</span>
              </DropdownMenuItem>
              <DropdownMenuItem className="cursor-pointer gap-2 py-2 text-xs">
                <Bell className="size-4 text-muted-foreground" />
                <span>Notifications</span>
              </DropdownMenuItem>
              {onOpenChangelog && (
                <DropdownMenuItem
                  onClick={onOpenChangelog}
                  className="cursor-pointer gap-2 py-2 text-xs"
                >
                  <Activity className="size-4 text-primary" />
                  <span>Release Notes {user.version ? `(v${user.version})` : ""}</span>
                </DropdownMenuItem>
              )}
            </DropdownMenuGroup>
            
            <DropdownMenuSeparator className="my-1" />
            
            <DropdownMenuItem className="cursor-pointer gap-2 py-2 text-xs text-destructive focus:text-destructive">
              <LogOut className="size-4" />
              <span>Log out</span>
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </SidebarMenuItem>
    </SidebarMenu>
  );
}
