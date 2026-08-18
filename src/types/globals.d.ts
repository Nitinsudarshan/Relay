export { }

declare global {
    interface CustomJwtSessionClaims {
        metadata: {
            role?: "Admin" | "Program" | "Operations" | "Volunteer";
            volunteerEnabled?: boolean;
            userManagementEnabled?: boolean;
        };
    }
}
