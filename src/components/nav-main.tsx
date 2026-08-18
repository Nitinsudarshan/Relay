"use client"

import { ChevronRight, type LucideIcon, Lock } from "lucide-react"
import * as React from "react"

import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible"
import {
  SidebarGroup,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuAction,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
  useSidebar,
} from "@/components/ui/sidebar"
import Link from "next/link"

export type NavItem = {
  title: string
  url: string
  icon?: LucideIcon
  isActive?: boolean
  disabled?: boolean
  tooltipOverride?: string
  items?: NavItem[]
}

function NavItem({ 
  item, 
  level = 0,
  isOpen: controlledIsOpen,
  onOpenChange: controlledOnOpenChange
}: { 
  item: NavItem, 
  level?: number,
  isOpen?: boolean,
  onOpenChange?: (open: boolean) => void 
}) {
  const { setOpenMobile, isMobile } = useSidebar()
  const hasSubItems = !!item.items?.length
  
  const [localIsOpen, setLocalIsOpen] = React.useState(!!item.isActive)

  const isOpen = controlledIsOpen !== undefined ? controlledIsOpen : localIsOpen;
  const setIsOpen = controlledOnOpenChange !== undefined ? controlledOnOpenChange : setLocalIsOpen;

  if (!hasSubItems) {
    const Component = level === 0 ? SidebarMenuButton : SidebarMenuSubButton
    const ItemWrapper = level === 0 ? SidebarMenuItem : SidebarMenuSubItem

    return (
      <ItemWrapper key={item.title}>
        <Component
          asChild={!item.disabled}
          tooltip={item.tooltipOverride || item.title}
          isActive={item.isActive}
          disabled={item.disabled}
          className={item.disabled ? "opacity-50 cursor-not-allowed" : ""}
        >
          {item.disabled ? (
            <div className="flex items-center gap-2 px-2 py-1.5 grayscale">
              {item.icon && <item.icon />}
              <span>{item.title}</span>
              <div className="ml-auto">
                <Lock className="w-3 h-3" />
              </div>
            </div>
          ) : (
            <Link 
              href={item.url}
              onClick={() => {
                if (isMobile) setOpenMobile(false)
              }}
            >
              {item.icon && <item.icon />}
              <span>{item.title}</span>
            </Link>
          )}
        </Component>
      </ItemWrapper>
    )
  }

  if (level === 0) {
    return (
      <Collapsible
        key={item.title}
        asChild
        open={isOpen}
        onOpenChange={setIsOpen}
        className="group/collapsible"
      >
        <SidebarMenuItem>
          <CollapsibleTrigger asChild>
            <SidebarMenuButton tooltip={item.tooltipOverride || item.title} isActive={item.isActive}>
              {item.icon && <item.icon />}
              <span>{item.title}</span>
              <ChevronRight className="ml-auto transition-transform duration-200 group-data-[state=open]/collapsible:rotate-90" />
            </SidebarMenuButton>
          </CollapsibleTrigger>
          <CollapsibleContent>
            <SidebarMenuSub>
              {item.items?.map((subItem) => (
                <NavItem key={subItem.title} item={subItem} level={level + 1} />
              ))}
            </SidebarMenuSub>
          </CollapsibleContent>
        </SidebarMenuItem>
      </Collapsible>
    )
  }

  // Nested levels (e.g. level 1 submenu containing level 2 items)
  return (
    <Collapsible
      key={item.title}
      asChild
      open={isOpen}
      onOpenChange={setIsOpen}
      className="group/collapsible-sub"
    >
      <SidebarMenuSubItem>
        <CollapsibleTrigger asChild>
          <SidebarMenuSubButton isActive={item.isActive} className="justify-between">
            <span>{item.title}</span>
            <ChevronRight className="h-3 w-3 transition-transform duration-200 group-data-[state=open]/collapsible-sub:rotate-90" />
          </SidebarMenuSubButton>
        </CollapsibleTrigger>
        <CollapsibleContent>
          <SidebarMenuSub className="ml-2 border-l pl-2 mt-1">
            {item.items?.map((subItem) => (
              <NavItem key={subItem.title} item={subItem} level={level + 1} />
            ))}
          </SidebarMenuSub>
        </CollapsibleContent>
      </SidebarMenuSubItem>
    </Collapsible>
  )
}

export function NavMain({
  items,
  label,
}: {
  items: NavItem[]
  label?: string
}) {
  const [openItem, setOpenItem] = React.useState<string | null>(
    items.find((item) => item.isActive || item.items?.some(sub => sub.isActive))?.title || null
  )

  return (
    <SidebarGroup>
      {label && <SidebarGroupLabel>{label}</SidebarGroupLabel>}
      <SidebarMenu>
        {items.map((item) => (
          <NavItem 
            key={item.title} 
            item={item} 
            isOpen={openItem === item.title}
            onOpenChange={(isOpen) => setOpenItem(isOpen ? item.title : null)}
          />
        ))}
      </SidebarMenu>
    </SidebarGroup>
  )
}
