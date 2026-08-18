import { AppSidebar } from "@/components/app-sidebar"
import { SiteHeader } from "@/components/site-header"
import {
    SidebarInset,
    SidebarProvider,
} from "@/components/ui/sidebar"

import { cookies } from "next/headers"
import { currentUser } from "@/lib/auth"
import { UserProvider } from "@/contexts/user-context"

export default async function DashboardLayout({
    children,
}: {
    children: React.ReactNode
}) {
    // Read the dev override from cookies directly
    const cookieStore = await cookies()
    const rawDevOverride = cookieStore.get('dev-role-override')?.value
    const user = await currentUser();

    const baseRole = "Admin"; // Hardcoded for boilerplate

    return (
        <UserProvider user={{
            id: "1",
            name: "Admin User",
            email: "admin@example.com",
            avatar: "",
        }}>
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
