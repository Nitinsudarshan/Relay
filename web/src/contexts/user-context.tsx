"use client";

import React, { createContext, useContext } from "react";

export type User = {
    id: string;
    name: string;
    email: string;
    avatar: string;
};

const UserContext = createContext<User | null>(null);

export function UserProvider({
    user,
    children,
}: {
    user: User | null;
    children: React.ReactNode;
}) {
    return (
        <UserContext.Provider value={user}>
            {children}
        </UserContext.Provider>
    );
}

export function useUserContext() {
    return useContext(UserContext);
}
