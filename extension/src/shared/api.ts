// API client with race-safe token refresh

import type { AuthTokens, ApiResponse, Entry, Memo, SaveRequest, CreateMemoRequest } from './types';
import { getLocalStorage, setLocalStorage, getSessionStorage, setSessionStorage, clearAllStorage } from './storage';

function normalizeUrl(url: string): string {
  return url.replace(/\/+$/, '');
}

async function getAccessToken(): Promise<string | null> {
  const { access_token } = await getSessionStorage(['access_token']);
  return access_token ?? null;
}

// Race-safe token refresh using promise caching
interface RefreshState {
  promise: Promise<string | null> | null;
}

const refreshState: RefreshState = { promise: null };

async function doRefresh(): Promise<string | null> {
  const { refresh_token } = await getLocalStorage(['refresh_token']);
  if (!refresh_token) {
    return null;
  }

  try {
    const { server_url } = await getLocalStorage(['server_url']);
    if (!server_url) {
      return null;
    }

    const resp = await fetch(`${normalizeUrl(server_url)}/api/v1/auth/refresh`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ refresh_token }),
    });

    if (!resp.ok) {
      await clearAllStorage();
      return null;
    }

    const data: AuthTokens = await resp.json();
    await setSessionStorage({ access_token: data.access_token });
    if (data.refresh_token) {
      await setLocalStorage({ refresh_token: data.refresh_token });
    }
    return data.access_token;
  } catch (err) {
    console.error('[Lettura] Token refresh failed:', err);
    await clearAllStorage();
    return null;
  }
}

/**
 * Refresh the access token. If a refresh is already in progress,
 * returns the existing promise to prevent race conditions.
 */
export async function refreshToken(): Promise<string | null> {
  // If refresh is already in progress, reuse the promise
  if (refreshState.promise) {
    return refreshState.promise;
  }

  refreshState.promise = doRefresh();
  try {
    return await refreshState.promise;
  } finally {
    refreshState.promise = null;
  }
}

interface ApiRequestOptions {
  method: string;
  path: string;
  body?: unknown;
  accessToken?: string;
}

async function apiRequest<T>(options: ApiRequestOptions): Promise<Response> {
  const { server_url } = await getLocalStorage(['server_url']);
  if (!server_url) {
    throw new Error('Server URL not configured');
  }

  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (options.accessToken) {
    headers['Authorization'] = `Bearer ${options.accessToken}`;
  }

  const resp = await fetch(`${normalizeUrl(server_url)}${options.path}`, {
    method: options.method,
    headers,
    body: options.body ? JSON.stringify(options.body) : undefined,
  });

  return resp;
}

/**
 * Make an authenticated API request with automatic token refresh on 401.
 */
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

// Convenience API methods

export async function saveEntry(url: string): Promise<Response> {
  return authenticatedRequest<Entry>('POST', '/api/v1/entries', { url });
}

export async function createMemo(content: string, sourceUrl?: string): Promise<Response> {
  return authenticatedRequest<Memo>('POST', '/api/v1/memos', {
    content,
    source_url: sourceUrl,
  });
}

export async function login(
  serverUrl: string,
  email: string,
  password: string
): Promise<AuthTokens> {
  const resp = await fetch(`${normalizeUrl(serverUrl)}/api/v1/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, password }),
  });

  if (!resp.ok) {
    const errData = await resp.json().catch(() => ({ message: 'Login failed' }));
    throw new Error(errData.message || `Login failed (${resp.status})`);
  }

  // Clear any existing PAT data before storing JWT tokens
  await chrome.storage.local.remove(['pat_token', 'auth_mode']);

  return resp.json();
}

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
