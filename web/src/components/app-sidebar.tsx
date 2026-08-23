"use client";

import * as React from "react";
import {
  Kanban,
  Settings,
  Mic,
  FileText,
  Calendar,
  Sparkles,
  Layers,
  Database,
  Cloud,
} from "lucide-react";

import { NavMain, NavItem } from "@/components/nav-main";
import { NavProjects, ProjectItem } from "@/components/nav-projects";
import { NavUser, UserData } from "@/components/nav-user";
import { TeamSwitcher, TeamItem } from "@/components/team-switcher";
import { RelayLogo } from "@/components/relay-logo";
import {
  Sidebar,
  SidebarContent,
  SidebarHeader,
  SidebarFooter,
  SidebarRail,
} from "@/components/ui/sidebar";
import { ChangelogDialog } from "@/components/changelog-dialog";

const teamsData: TeamItem[] = [
  {
    name: "Relay Cloud",
    logo: RelayLogo,
    plan: "Hybrid Workspace",
  },
  {
    name: "Local Vault",
    logo: Database,
    plan: "100% On-Device",
  },
  {
    name: "Personal Workspace",
    logo: Layers,
    plan: "Pro Sync",
  },
];

const navMainData: NavItem[] = [
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
    title: "Meetings",
    url: "/meetings",
    icon: Calendar,
  },
  {
    title: "Scribbles & Canvas",
    url: "/scribbles",
    icon: Sparkles,
  },
  {
    title: "Settings",
    url: "/settings",
    icon: Settings,
    items: [
      {
        title: "Account & Profile",
        url: "/settings#account",
      },
      {
        title: "Hybrid Sync & Cloud",
        url: "/settings#sync",
      },
      {
        title: "AI Models & Providers",
        url: "/settings#ai",
      },
    ],
  },
];

const quickProjectsData: ProjectItem[] = [
  {
    name: "Engineering Roadmap",
    url: "/notes?id=roadmap",
    icon: FileText,
  },
  {
    name: "Product Sprint Backlog",
    url: "/?board=sprint",
    icon: Kanban,
  },
  {
    name: "Weekly Sync Notes",
    url: "/meetings?id=weekly",
    icon: Calendar,
  },
];

const userData: UserData = {
  name: "Nitin Sudarshan",
  email: "nitin@example.com",
  plan: "Hybrid Cloud",
  version: "0.3.4",
};

export function AppSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
  const [changelogOpen, setChangelogOpen] = React.useState(false);

  return (
    <>
      <Sidebar
        collapsible="icon"
        className="top-(--header-height) h-[calc(100svh-var(--header-height))]!"
        {...props}
      >
        <SidebarHeader>
          <TeamSwitcher teams={teamsData} />
        </SidebarHeader>

        <SidebarContent>
          <NavMain items={navMainData} label="PLATFORM" />
          <NavProjects projects={quickProjectsData} label="QUICK VAULT" />
        </SidebarContent>

        <SidebarFooter>
          <NavUser
            user={userData}
            onOpenChangelog={() => setChangelogOpen(true)}
          />
        </SidebarFooter>

        <SidebarRail />
      </Sidebar>

      <ChangelogDialog
        open={changelogOpen}
        onClose={() => setChangelogOpen(false)}
        currentVersion="0.3.4"
      />
    </>
  );
}
