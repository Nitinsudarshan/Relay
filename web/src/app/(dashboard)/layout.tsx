import { AppSidebar } from "@/components/app-sidebar"
import { SiteHeader } from "@/components/site-header"
import {
    SidebarInset,
    SidebarProvider,
} from "@/components/ui/sidebar"

import { UserProvider } from "@/contexts/user-context"

/**
 * Hybrid-mode dashboard shell.
 *
 * The signed-in user is still a placeholder: hybrid mode's real login
 * (decision 12) is not built yet, and RBAC is deliberately out of scope
 * (`rules/rbac-settings.md`). Until that lands, this renders a fixed local
 * identity rather than pretending to resolve one — there is no session to
 * read and no role to branch on.
 */
const PLACEHOLDER_USER = {
    id: "1",
    name: "Admin User",
    email: "admin@example.com",
    avatar: "",
} as const;

export default async function DashboardLayout({
    children,
}: {
    children: React.ReactNode
}) {
    return (
        <UserProvider user={{ ...PLACEHOLDER_USER }}>
            <div className="[--header-height:calc(--spacing(14))]">
                <SidebarProvider className="flex flex-col">
                    <SiteHeader />
                    <div className="flex flex-1">
                        <AppSidebar />
                        <SidebarInset>
                            {children}
                        </SidebarInset>
                    </div>
                </SidebarProvider>
            </div>
        </UserProvider>
    )
}
