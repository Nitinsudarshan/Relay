# Relay — Production Google OAuth Architecture & Setup Guide

This document outlines Relay's centralized Google OAuth 2.0 PKCE architecture, required Google Cloud configuration, and developer environment setup.

---

## 1. High-Level Architecture Overview

Relay uses a single, centralized, production-grade **Desktop OAuth 2.0 PKCE** service (`oauth/`) across all Google integrations.

```
                    RELAY DESKTOP APP
                            │
                            ▼
                ┌───────────────────────┐
                │ Centralized OAuth API │
                │    (oauth/flow.rs)    │
                └───────────┬───────────┘
                            │
             ┌──────────────┴──────────────┐
             ▼                             ▼
    ┌───────────────────┐        ┌───────────────────┐
    │  Relay Identity   │        │  Google Calendar  │
    │  (openid, email,  │        │  (calendar.events │
    │   profile)        │        │   .readonly)      │
    └────────┬──────────┘        └─────────┬─────────┘
             │                             │
             ▼                             ▼
    OS Keyring:                   OS Keyring:
    com.relay.app.identity        com.relay.app.calendar
```

### Key Security & Architectural Invariants
1. **Public Desktop App (RFC 8252 / RFC 7636 PKCE)**:
   - Uses loopback redirect on `127.0.0.1:0`.
   - Generates cryptographically random `code_verifier` (base64url) and SHA-256 `code_challenge`.
   - Generates cryptographically random `state` validated on every callback.
   - **No Client Secret is required or stored** for desktop applications.
2. **Strict Scope Separation**:
   - **Relay Sign-In**: Requests `openid`, `userinfo.email`, and `userinfo.profile` only.
   - **Google Calendar Sync**: Requests `calendar.events.readonly` separately only when explicitly initiated by the user.
3. **Isolated Keyring Namespaces**:
   - Identity tokens: stored under `com.relay.app.identity` / `google_account_tokens`.
   - Calendar tokens: stored under `com.relay.app.calendar` / `google_calendar_tokens`.
   - Fallback stores located in `.relay/config/` (never in the user's markdown `vault/`).
4. **Independent Lifecycle**:
   - Disconnecting Calendar revokes calendar tokens without signing out of Relay.
   - Signing out of Relay revokes identity tokens without deleting local meetings or vault data.

---

## 2. Google Cloud Console Setup (1-Time Developer Setup)

To configure the official Relay Desktop Client ID in Google Cloud:

### Step 1: Create a Google Cloud Project
1. Go to [Google Cloud Console](https://console.cloud.google.com/).
2. Create a new project (e.g. `Relay Desktop App`).

### Step 2: Enable Required APIs
1. Navigate to **APIs & Services $\rightarrow$ Enabled APIs & services $\rightarrow$ + ENABLE APIS AND SERVICES**.
2. Search for and enable:
   - **Google Calendar API**

### Step 3: Configure the OAuth Consent Screen
1. Navigate to **APIs & Services $\rightarrow$ OAuth consent screen**.
2. Select User Type: **External** (or Internal for Google Workspace).
3. Fill in:
   - **App name**: `Relay`
   - **User support email**: your email
   - **Developer contact information**: your email
4. In **Scopes**:
   - Add `openid`
   - Add `https://www.googleapis.com/auth/userinfo.email`
   - Add `https://www.googleapis.com/auth/userinfo.profile`
   - Add `https://www.googleapis.com/auth/calendar.events.readonly`
5. In **Test users** (while in Testing status):
   - Add your Google account email to allow login during development.

### Step 4: Create the Desktop OAuth 2.0 Client ID
1. Navigate to **APIs & Services $\rightarrow$ Credentials $\rightarrow$ + CREATE CREDENTIALS $\rightarrow$ OAuth client ID**.
2. Select **Application type**: **Desktop App**.
3. Name: `Relay Desktop Client`.
4. Click **Create**.
5. Copy the generated **Client ID** (format: `xxxxxxxxxxxx-xxxxxxxxxxxxxxxx.apps.googleusercontent.com`).

---

## 3. Configuring the Client ID in Relay

### Production / Environment Builds
Set the environment variable at build or runtime:
```bash
# In .env or CI build environment
RELAY_GOOGLE_CLIENT_ID="xxxxxxxxxxxx-xxxxxxxxxxxxxxxx.apps.googleusercontent.com"
```

### Developer Setting Override (In-App)
In the Relay application:
1. Open **Settings $\rightarrow$ Calendar & Meetings** (or click **Google Calendar** on the Meetings page).
2. Click **Configure custom OAuth client credentials**.
3. Paste the Desktop Client ID and save.

---

## 4. Troubleshooting & FAQ

### `Error 401: invalid_client`
- **Cause**: The Client ID does not exist in Google Cloud Console or was deleted.
- **Fix**: Verify the Client ID matches the Desktop App Client ID in your Google Cloud Console.

### `Error 403: access_denied` / App not verified
- **Cause**: The Google Cloud project is in "Testing" mode and your email is not added as a Test User.
- **Fix**: Add your Google account email under **OAuth consent screen $\rightarrow$ Test users**.
