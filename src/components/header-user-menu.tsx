"use client";

import { BadgeCheck, Bell, ChevronsUpDown, LogOut } from "lucide-react";
import { useRouter } from "next/navigation";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuGroup,
    DropdownMenuItem,
    DropdownMenuLabel,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Button } from "@/components/ui/button";
import { useUserContext } from "@/contexts/user-context";

function toTitleCase(str: string) {
    return str
        .toLowerCase()
        .replace(/\b\w/g, (c) => c.toUpperCase());
}

export function HeaderUserMenu() {
    const router = useRouter();
    const user = useUserContext();

    const signOut = () => {
        router.push("/login");
    };

    const firstName = user?.name ? toTitleCase(user.name.split(" ")[0]) : "User";
    const fullName = user?.name ? toTitleCase(user.name) : "User";
    const userEmail = user?.email || "user@example.com";
    const userAvatar = user?.avatar || "";
    const displayLabel = firstName;
    const initials = fullName
        ? fullName.split(" ").map((n) => n[0]).join("").toUpperCase().slice(0, 2)
        : displayLabel.slice(0, 2).toUpperCase();

    return (
        <DropdownMenu>
            <DropdownMenuTrigger asChild>
                <Button variant="ghost" className="h-9 gap-2 px-2 rounded-lg">
                    <Avatar className="h-7 w-7 rounded-md">
                        <AvatarImage src={userAvatar} alt={fullName} />
                        <AvatarFallback className="rounded-md text-xs">{initials}</AvatarFallback>
                    </Avatar>
                    <span className="hidden sm:block text-sm font-medium max-w-[100px] truncate">
                        {displayLabel}
                    </span>
                    <ChevronsUpDown className="h-4 w-4 text-muted-foreground" />
                </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-56">
                <DropdownMenuLabel className="p-0 font-normal">
                    <div className="flex items-center gap-2 px-2 py-2">
                        <Avatar className="h-8 w-8 rounded-md">
                            <AvatarImage src={userAvatar} alt={fullName} />
                            <AvatarFallback className="rounded-md text-xs">{initials}</AvatarFallback>
                        </Avatar>
                        <div className="flex flex-col min-w-0">
                            <span className="text-sm font-semibold truncate">{fullName}</span>
                            <span className="text-xs text-muted-foreground truncate">{userEmail}</span>
                        </div>
                    </div>
                </DropdownMenuLabel>
                <DropdownMenuSeparator />
                <DropdownMenuGroup>
                    <DropdownMenuItem onClick={() => router.push("/profile")}>
                        <BadgeCheck className="mr-2 h-4 w-4" />
                        Profile
                    </DropdownMenuItem>
                    <DropdownMenuItem onClick={() => router.push("/notifications")}>
                        <Bell className="mr-2 h-4 w-4" />
                        Notifications
                    </DropdownMenuItem>
                </DropdownMenuGroup>
                <DropdownMenuSeparator />
                <DropdownMenuItem onClick={() => signOut()}>
                    <LogOut className="mr-2 h-4 w-4" />
                    Log out
                </DropdownMenuItem>
            </DropdownMenuContent>
        </DropdownMenu>
    );
}
