# Extension Token Authentication Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add PAT (Personal Access Token) authentication as a first-class alternative to email/password login in the browser extension, with tab-based UI switching.

**Architecture:** Two auth modes (JWT and PAT) stored with an `auth_mode` flag. `authenticatedRequest` branches on the mode: JWT uses access/refresh token flow, PAT uses the stored token directly with no refresh. Login UI uses tabs to switch between password and token forms. `connectWithToken` validates the token via `GET /api/v1/auth/me` before storing.

**Tech Stack:** TypeScript, Chrome Extension Manifest V3, Vite, Chrome Storage API

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `extension/src/shared/types.ts` | Modify | Add `AuthMode` type |
| `extension/src/shared/storage.ts` | Modify | Add `pat_token`/`auth_mode` to clear logic |
| `extension/src/shared/api.ts` | Modify | Add `connectWithToken()`, update `authenticatedRequest` for PAT mode, update `login` to clear PAT data |
| `extension/src/popup/index.html` | Modify | Add tab bar and token login form |
| `extension/src/popup/styles.css` | Modify | Add tab styles |
| `extension/src/popup/index.ts` | Modify | Tab switching, token login flow, updated init/logout, token prefix display |

---

### Task 1: Add AuthMode type and update storage

**Files:**
- Modify: `extension/src/shared/types.ts`
- Modify: `extension/src/shared/storage.ts`

- [ ] **Step 1: Add AuthMode type to types.ts**

Append to `extension/src/shared/types.ts`:

```typescript
export type AuthMode = 'jwt' | 'pat';
```

- [ ] **Step 2: Update clearAllStorage in storage.ts**

Replace the `clearAllStorage` function in `extension/src/shared/storage.ts`:

```typescript
export async function clearAllStorage(): Promise<void> {
  return new Promise((resolve) => {
    chrome.storage.local.remove(['server_url', 'refresh_token', 'pat_token', 'auth_mode'], () => {
      chrome.storage.session.remove(['access_token'], resolve);
    });
  });
}
```

- [ ] **Step 3: Build and verify**

Run: `cd /home/cc/workspace/lettura/extension && pnpm build`
Expected: Build succeeds with no errors.

- [ ] **Step 4: Commit**

```bash
git add extension/src/shared/types.ts extension/src/shared/storage.ts
git commit -m "feat(extension): add AuthMode type and update storage for PAT support"
```

---

### Task 2: Add connectWithToken and update api.ts for PAT mode

**Files:**
- Modify: `extension/src/shared/api.ts`

- [ ] **Step 1: Add connectWithToken function**

```typescript
export async function connectWithToken(
  serverUrl: string,
  token: string
): Promise<void> {
  const normalized = normalizeUrl(serverUrl);

  // Validate server URL uses HTTPS
  if (!normalized.startsWith('https://') && !normalized.startsWith('http://localhost') && !normalized.startsWith('http://127.0.0.1')) {
    throw new Error('Server URL must use HTTPS (or http://localhost for development).');
  }

  // Validate token format
  if (!token.startsWith('lta_')) {
    throw new Error('Token must start with lta_');
  }

  // Validate token by calling /api/v1/auth/me
  const resp = await fetch(`${normalized}/api/v1/auth/me`, {
    method: 'GET',
    headers: {
      'Authorization': `Bearer ${token}`,
    },
  });

  if (resp.status === 401) {
    throw new Error('Token is invalid or expired.');
  }
  if (resp.status === 403) {
    throw new Error('Token has read-only scope. Use a token with write scope.');
  }
  if (!resp.ok) {
    throw new Error(`Verification failed (${resp.status})`);
  }

  // Clear any existing JWT tokens before storing PAT data
  await chrome.storage.session.remove(['access_token']);
  await chrome.storage.local.remove(['refresh_token']);

  // Store PAT data
  await setLocalStorage({
    server_url: normalized,
    pat_token: token,
    auth_mode: 'pat',
  });
}
```

- [ ] **Step 2: Update login function to clear PAT data**

In the `login` function, before the `return resp.json();` line, add:

```typescript
  // Clear any existing PAT data before storing JWT tokens
  await chrome.storage.local.remove(['pat_token', 'auth_mode']);
```

- [ ] **Step 3: Update authenticatedRequest for PAT mode**

Replace the `authenticatedRequest` function:

```typescript
export async function authenticatedRequest<T>(
  method: string,
  path: string,
  body?: unknown
): Promise<Response> {
  const { auth_mode } = await getLocalStorage(['auth_mode']);

  if (auth_mode === 'pat') {
    const { pat_token } = await getLocalStorage(['pat_token']);
    if (!pat_token) {
      throw new Error('Not authenticated. Please login via the popup.');
    }

    const resp = await apiRequest<T>({ method, path, body, accessToken: pat_token });

    if (resp.status === 401) {
      await clearAllStorage();
      throw new Error('Token expired or invalid. Please login again.');
    }
    if (resp.status === 403) {
      const errData = await resp.json().catch(() => null);
      const msg = errData?.message ?? 'Token has read-only scope. Use a write-scope token.';
      throw new Error(msg);
    }

    return resp;
  }

  // JWT mode (existing logic)
  let token = await getAccessToken();
  if (!token) {
    token = await refreshToken();
    if (!token) {
      throw new Error('Not authenticated. Please login via the popup.');
    }
  }

  let resp = await apiRequest<T>({ method, path, body, accessToken: token });

  if (resp.status === 401) {
    const newToken = await refreshToken();
    if (!newToken) {
      throw new Error('Session expired. Please login again via the popup.');
    }
    resp = await apiRequest<T>({ method, path, body, accessToken: newToken });
  }

  return resp;
}
```

- [ ] **Step 4: Build and verify**

Run: `cd /home/cc/workspace/lettura/extension && pnpm build`
Expected: Build succeeds with no errors.

- [ ] **Step 5: Commit**

```bash
git add extension/src/shared/api.ts
git commit -m "feat(extension): add connectWithToken and PAT mode in authenticatedRequest"
```

---

### Task 3: Add tab UI and token login form to popup HTML

**Files:**
- Modify: `extension/src/popup/index.html`

- [ ] **Step 1: Replace login-section with tab-based layout**

Replace the entire `login-section` div in `extension/src/popup/index.html` with:

```html
    <!-- Login form -->
    <div id="login-section" class="section hidden">
      <h1 class="title">Lettura</h1>
      <p class="subtitle">Save articles for later</p>

      <div class="tabs">
        <button id="tab-password" class="tab active">Password</button>
        <button id="tab-token" class="tab">Token</button>
      </div>

      <!-- Password login form -->
      <div id="form-password" class="tab-content">
        <div class="form-group">
          <label for="server-url">Server URL</label>
          <input type="url" id="server-url" placeholder="https://your-lettura-instance.com" />
        </div>

        <div class="form-group">
          <label for="email">Email</label>
          <input type="email" id="email" placeholder="you@example.com" />
        </div>

        <div class="form-group">
          <label for="password">Password</label>
          <input type="password" id="password" placeholder="••••••••" />
        </div>

        <div id="login-error" class="message error hidden"></div>

        <button id="login-btn" class="btn btn-primary">Login</button>
      </div>

      <!-- Token login form -->
      <div id="form-token" class="tab-content hidden">
        <div class="form-group">
          <label for="token-server-url">Server URL</label>
          <input type="url" id="token-server-url" placeholder="https://your-lettura-instance.com" />
        </div>

        <div class="form-group">
          <label for="token-input">Token</label>
          <input type="password" id="token-input" placeholder="lta_..." />
        </div>

        <div id="token-error" class="message error hidden"></div>

        <button id="token-btn" class="btn btn-primary">Connect</button>
      </div>
    </div>
```

- [ ] **Step 2: Update main-section to show token prefix**

Replace the `main-section` div with:

```html
    <!-- Main section (after login) -->
    <div id="main-section" class="section hidden">
      <div class="header">
        <h1 class="title">Lettura</h1>
        <div class="header-badges">
          <span id="server-info" class="server-badge"></span>
          <span id="token-badge" class="token-badge hidden"></span>
        </div>
      </div>

      <button id="save-btn" class="btn btn-primary btn-block">Save this page</button>
      <div id="save-status" class="message hidden"></div>

      <button id="logout-btn" class="btn btn-secondary btn-block">Logout</button>
    </div>
```

- [ ] **Step 3: Build and verify**

Run: `cd /home/cc/workspace/lettura/extension && pnpm build`
Expected: Build succeeds (HTML is just a template, no runtime check needed yet).

- [ ] **Step 4: Commit**

```bash
git add extension/src/popup/index.html
git commit -m "feat(extension): add tab-based login UI with token form"
```

---

### Task 4: Add tab and token badge styles

**Files:**
- Modify: `extension/src/popup/styles.css`

- [ ] **Step 1: Add tab styles**

Append to `extension/src/popup/styles.css`:

```css
.tabs {
  display: flex;
  gap: 0;
  border-bottom: 1px solid #d1d5db;
  margin-bottom: 4px;
}

.tab {
  flex: 1;
  padding: 8px 0;
  border: none;
  background: transparent;
  font-size: 13px;
  font-weight: 500;
  color: #6b7280;
  cursor: pointer;
  border-bottom: 2px solid transparent;
  transition: color 0.2s, border-color 0.2s;
}

.tab:hover {
  color: #1a1a2e;
}

.tab.active {
  color: #4361ee;
  border-bottom-color: #4361ee;
}

.tab-content {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.header-badges {
  display: flex;
  gap: 6px;
  align-items: center;
  overflow: hidden;
}

.token-badge {
  font-size: 11px;
  color: #4361ee;
  background: #eef2ff;
  padding: 2px 8px;
  border-radius: 4px;
  max-width: 100px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
```

- [ ] **Step 2: Build and verify**

Run: `cd /home/cc/workspace/lettura/extension && pnpm build`
Expected: Build succeeds.

- [ ] **Step 3: Commit**

```bash
git add extension/src/popup/styles.css
git commit -m "feat(extension): add tab and token badge styles"
```

---

### Task 5: Update popup TypeScript for tab switching, token login, and PAT init

**Files:**
- Modify: `extension/src/popup/index.ts`

This is the largest task. The popup script needs: tab switching logic, token login flow, updated init to handle PAT mode, token prefix display, and updated logout.

- [ ] **Step 1: Update imports**

Replace the imports at the top of `extension/src/popup/index.ts`:

```typescript
import { login, refreshToken, saveEntry, connectWithToken } from '../shared/api';
import { getLocalStorage, setLocalStorage, getSessionStorage, setSessionStorage, clearAllStorage } from '../shared/storage';
```

- [ ] **Step 2: Add new DOM element references**

After the existing DOM element declarations, add:

```typescript
// Tab elements
const tabPassword = document.getElementById('tab-password') as HTMLButtonElement;
const tabToken = document.getElementById('tab-token') as HTMLButtonElement;
const formPassword = document.getElementById('form-password')!;
const formToken = document.getElementById('form-token')!;

// Token form elements
const tokenServerUrlInput = document.getElementById('token-server-url') as HTMLInputElement;
const tokenInput = document.getElementById('token-input') as HTMLInputElement;
const tokenBtn = document.getElementById('token-btn') as HTMLButtonElement;
const tokenError = document.getElementById('token-error')!;

// Token badge in main section
const tokenBadge = document.getElementById('token-badge')!;
```

- [ ] **Step 3: Add tab switching logic**

After the `hideMessage` function, add:

```typescript
// --- Tab switching ---

function switchTab(tab: 'password' | 'token'): void {
  if (tab === 'password') {
    tabPassword.classList.add('active');
    tabToken.classList.remove('active');
    formPassword.classList.remove('hidden');
    formToken.classList.add('hidden');
    // Sync server URL from token form to password form
    if (tokenServerUrlInput.value) {
      serverUrlInput.value = tokenServerUrlInput.value;
    }
  } else {
    tabToken.classList.add('active');
    tabPassword.classList.remove('active');
    formToken.classList.remove('hidden');
    formPassword.classList.add('hidden');
    // Sync server URL from password form to token form
    if (serverUrlInput.value) {
      tokenServerUrlInput.value = serverUrlInput.value;
    }
  }
}

tabPassword.addEventListener('click', () => switchTab('password'));
tabToken.addEventListener('click', () => switchTab('token'));
```

- [ ] **Step 4: Add token login action**

After the `doLogin` function, add:

```typescript
async function doTokenLogin(): Promise<void> {
  hideMessage(tokenError);
  const serverUrl = tokenServerUrlInput.value.trim();
  const token = tokenInput.value.trim();

  if (!serverUrl) {
    showMessage(tokenError, 'Please enter the server URL.', 'error');
    return;
  }
  if (!serverUrl.startsWith('https://') && !serverUrl.startsWith('http://localhost') && !serverUrl.startsWith('http://127.0.0.1')) {
    showMessage(tokenError, 'Server URL must use HTTPS (or http://localhost for development).', 'error');
    return;
  }
  if (!token) {
    showMessage(tokenError, 'Please enter your token.', 'error');
    return;
  }
  if (!token.startsWith('lta_')) {
    showMessage(tokenError, 'Token must start with lta_.', 'error');
    return;
  }

  tokenBtn.disabled = true;
  tokenBtn.textContent = 'Connecting...';

  try {
    await connectWithToken(serverUrl, token);
    showMainSection(serverUrl, token);
  } catch (err) {
    showMessage(
      tokenError,
      err instanceof Error ? err.message : 'Connection failed',
      'error'
    );
  } finally {
    tokenBtn.disabled = false;
    tokenBtn.textContent = 'Connect';
  }
}
```

- [ ] **Step 5: Update showMainSection to accept optional token**

Replace the `showMainSection` function:

```typescript
function showMainSection(serverUrl: string, patToken?: string): void {
  serverInfo.textContent = serverUrl;
  hideMessage(saveStatus);

  if (patToken) {
    const prefix = patToken.substring(0, 12);
    tokenBadge.textContent = `${prefix}...`;
    tokenBadge.classList.remove('hidden');
  } else {
    tokenBadge.classList.add('hidden');
  }

  showSection(mainSection);
}
```

- [ ] **Step 6: Update doLogin to pass server URL to showMainSection**

In the `doLogin` function, change:

```typescript
    showMainSection(serverUrl);
```

(This line already exists and passes `serverUrl`, so no change needed — just verify it's there.)

- [ ] **Step 7: Update doLogout to reset tab state**

Replace the `doLogout` function:

```typescript
async function doLogout(): Promise<void> {
  await clearAllStorage();
  showSection(loginSection);
  switchTab('password');
  serverUrlInput.value = '';
  emailInput.value = '';
  passwordInput.value = '';
  tokenServerUrlInput.value = '';
  tokenInput.value = '';
  hideMessage(loginError);
  hideMessage(tokenError);
  hideMessage(saveStatus);
}
```

- [ ] **Step 8: Update init function for PAT mode**

Replace the `init` function:

```typescript
async function init(): Promise<void> {
  showSection(loadingSection);

  const { server_url, refresh_token, pat_token, auth_mode } = await getLocalStorage(['server_url', 'refresh_token', 'pat_token', 'auth_mode']);
  const { access_token } = await getSessionStorage(['access_token']);

  if (auth_mode === 'pat' && pat_token && server_url) {
    showMainSection(server_url, pat_token);
  } else if ((access_token || refresh_token) && server_url) {
    // JWT mode — try to ensure we have a valid access token
    if (!access_token && refresh_token) {
      const newToken = await refreshToken();
      if (!newToken) {
        serverUrlInput.value = server_url ?? '';
        tokenServerUrlInput.value = server_url ?? '';
        showSection(loginSection);
        return;
      }
    }
    showMainSection(server_url);
  } else {
    serverUrlInput.value = server_url ?? '';
    tokenServerUrlInput.value = server_url ?? '';
    showSection(loginSection);
  }
}
```

- [ ] **Step 9: Update event listeners**

Replace the event listeners section:

```typescript
// --- Event listeners ---

loginBtn.addEventListener('click', doLogin);
tokenBtn.addEventListener('click', doTokenLogin);
saveBtn.addEventListener('click', doSave);
logoutBtn.addEventListener('click', doLogout);

// Allow Enter key to submit login form
[serverUrlInput, emailInput, passwordInput].forEach((input) => {
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      doLogin();
    }
  });
});

// Allow Enter key to submit token form
[tokenServerUrlInput, tokenInput].forEach((input) => {
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      doTokenLogin();
    }
  });
});
```

- [ ] **Step 10: Build and verify**

Run: `cd /home/cc/workspace/lettura/extension && pnpm build`
Expected: Build succeeds with no errors.

- [ ] **Step 11: Commit**

```bash
git add extension/src/popup/index.ts
git commit -m "feat(extension): implement tab switching, token login, and PAT mode init"
```

---

### Task 6: Manual testing and final verification

**Files:** None (testing only)

- [ ] **Step 1: Build the extension**

Run: `cd /home/cc/workspace/lettura/extension && pnpm build`

- [ ] **Step 2: Load extension in Chrome**

1. Open `chrome://extensions/`
2. Enable Developer Mode
3. Click "Load unpacked"
4. Select `extension/dist/`

- [ ] **Step 3: Test Password tab login**

1. Click extension icon
2. Verify two tabs visible: "Password" and "Token"
3. Verify Password tab is active by default
4. Enter server URL, email, password
5. Click Login
6. Verify main section appears with server badge

- [ ] **Step 4: Test Token tab login**

1. Logout
2. Click Token tab
3. Verify form switches to show Server URL + Token input
4. Verify Server URL from Password tab is preserved
5. Enter server URL and a valid `lta_` token
6. Click Connect
7. Verify main section appears with server badge AND token prefix badge

- [ ] **Step 5: Test Token validation**

1. Logout
2. Click Token tab
3. Enter an invalid token (not starting with `lta_`)
4. Verify error message "Token must start with lta_."
5. Enter a valid-format but incorrect token
6. Verify error message about invalid/expired token

- [ ] **Step 6: Test right-click save with PAT**

1. While logged in via Token tab
2. Right-click on a link → "Save to Lettura"
3. Verify badge shows "OK" (green) or "DUP" (orange)

- [ ] **Step 7: Test tab sync**

1. Enter server URL in Password tab
2. Switch to Token tab
3. Verify server URL is synced to Token tab's input

- [ ] **Step 8: Bump version and commit**

Update version in `extension/src/manifest.json` and `extension/scripts/postbuild.mjs` from `1.1.0` to `1.2.0`, and in `extension/package.json` from `1.1.0` to `1.2.0`.

```bash
git add extension/src/manifest.json extension/scripts/postbuild.mjs extension/package.json
git commit -m "chore(extension): bump version to 1.2.0 for token auth support"
```
