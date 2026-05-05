# Extension Token Authentication Design

## Overview

Add Personal Access Token (PAT) authentication to the Lettura browser extension as a first-class alternative to email/password login. Users can choose between two authentication methods via a tab-based UI.

## Motivation

- PAT tokens (`lta_` prefix) are long-lived credentials that don't require refresh flows, making them simpler and more reliable for browser extensions.
- Users who already have a PAT (e.g., created for CLI use) can reuse it without entering credentials.
- PAT authentication avoids storing refresh tokens and the complexity of token refresh cycles.

## UI Design

### Login Screen

Two tabs at the top of the login form:

- **Password** tab (default): Server URL + Email + Password + Login button (existing flow)
- **Token** tab: Server URL + Token input + Connect button

Tab switching preserves the Server URL field value. Active tab has visual indicator (bottom border or background color).

### Token Input

- Type: `password` (masked input)
- Placeholder: `lta_...`
- Validation: must start with `lta_` prefix before submission

### Main Section (PAT mode)

When authenticated via PAT, the server badge additionally shows the token prefix (first 12 characters, e.g. `lta_abcdefgh...`) so users can identify which token is active.

### Mode Switching

Logout is the only way to switch authentication modes. There is no in-session mode switch — users must logout first, then choose a different tab.

## Storage Changes

### New Keys

| Key | Storage | Purpose |
|-----|---------|---------|
| `pat_token` | `chrome.storage.local` | Plaintext PAT token (persistent — PATs are long-lived by design) |
| `auth_mode` | `chrome.storage.local` | `'jwt'` or `'pat'` — identifies current authentication method |

### Existing Keys (unchanged)

| Key | Storage | Purpose |
|-----|---------|---------|
| `server_url` | `chrome.storage.local` | Server URL |
| `refresh_token` | `chrome.storage.local` | JWT refresh token (only set after password login) |
| `access_token` | `chrome.storage.session` | JWT access token (only set after password login) |

### Invariants

- When `auth_mode === 'pat'`: `pat_token` is set; `access_token` and `refresh_token` are not set.
- When `auth_mode === 'jwt'`: `access_token` and/or `refresh_token` are set; `pat_token` is not set.
- Logout clears all keys regardless of mode.
- Switching modes requires logout first — `connectWithToken` clears JWT keys, `login` clears PAT keys.

## API Client Changes

### `connectWithToken` Function

```typescript
export async function connectWithToken(serverUrl: string, token: string): Promise<void>
```

1. Validates token starts with `lta_`
2. Validates server URL uses HTTPS (same rule as password login: HTTPS required, localhost/127.0.0.1 allowed for development)
3. Makes a test call to `GET /api/v1/auth/me` with the token to verify it is valid and has write scope
4. If the test call fails (401/403/network error): throws an error, does NOT store anything
5. If the test call succeeds: clears any existing JWT tokens (`access_token`, `refresh_token`), then stores `server_url`, `pat_token`, `auth_mode` = `'pat'`

The test call serves three purposes:
- Validates the token is not expired or revoked
- Confirms the token has write scope (the extension's core features — save entry, create memo — all use POST)
- Provides immediate feedback instead of silent failure on first use

### `login` Function (existing, updated)

After successful login, clear any existing PAT data (`pat_token`, `auth_mode`) before storing JWT tokens. This maintains the invariant that only one auth mode's data exists at a time.

### `authenticatedRequest` Logic

```
1. Read auth_mode from storage
2. If auth_mode === 'pat':
   a. Read pat_token from storage
   b. Send request with Authorization: Bearer <pat_token>
   c. On 401: clear all storage, throw "Token expired or invalid"
   d. On 403 with scope message: throw "Token has read-only scope. Use a write-scope token."
   e. No refresh attempt — PATs cannot be refreshed
3. If auth_mode === 'jwt':
   a. Existing flow: access_token + 401 auto-refresh via refresh_token
```

### Background Script Behavior

The background service worker's `saveEntry` and `createMemo` functions already use `authenticatedRequest`, which will be updated to handle both modes. No additional changes needed in the background script.

Note: In PAT mode, a 401 from a background request (right-click save) will clear all storage. The user will see an "ERR" badge, and the next time they open the popup they will be prompted to log in again. This is different from JWT mode where a 401 triggers an automatic refresh attempt.

## Popup Init Logic

```
1. Read auth_mode, server_url from storage
2. If auth_mode === 'pat' && pat_token exists:
   → Show main section with token prefix in badge
3. If auth_mode === 'jwt' && (access_token || refresh_token):
   → Existing JWT init flow (try refresh if needed)
4. Otherwise:
   → Show login section, default to Password tab
   → Pre-fill server_url if available
```

## Logout

Clear all storage keys (`server_url`, `refresh_token`, `pat_token`, `auth_mode` from local; `access_token` from session). Reset UI to login section with Password tab active.

## Files to Modify

| File | Changes |
|------|---------|
| `extension/src/popup/index.html` | Add tab bar, token input form |
| `extension/src/popup/styles.css` | Tab styles |
| `extension/src/popup/index.ts` | Tab switching, token login flow, updated init/logout |
| `extension/src/shared/api.ts` | `connectWithToken()`, updated `authenticatedRequest` with PAT mode, updated `login` to clear PAT data |
| `extension/src/shared/storage.ts` | Add `pat_token` and `auth_mode` to clear logic |
| `extension/src/shared/types.ts` | Add `AuthMode` type |
