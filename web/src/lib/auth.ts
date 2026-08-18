// dummy auth

export async function auth() {
    return {
        userId: "dummy-user-id",
        sessionClaims: {
            metadata: {
                role: 'Admin'
            },
            role: 'Admin'
        } as any
    };
}

export async function currentUser() {
    return {
        id: "dummy-user-id",
        emailAddresses: [{ emailAddress: "dummy@example.com" }],
        primaryEmailAddress: { emailAddress: "dummy@example.com" },
        firstName: "Dummy",
        lastName: "User",
        fullName: "Dummy User",
        imageUrl: "",
        publicMetadata: {
            role: 'Admin'
        }
    };
}


