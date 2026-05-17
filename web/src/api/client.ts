// Fetch-based API client with 401 auto-refresh, replacing axios.
// Exposes apiGet / apiPost / apiPatch / apiDel helpers that return typed data.

const BASE = '/api/v1';

// ---------------------------------------------------------------------------
// Token helpers
// ---------------------------------------------------------------------------

function getAccessToken(): string | null {
  return localStorage.getItem('access_token');
}

function setTokens(access: string, refresh: string) {
  localStorage.setItem('access_token', access);
  localStorage.setItem('refresh_token', refresh);
}

function clearTokens() {
  localStorage.removeItem('access_token');
  localStorage.removeItem('refresh_token');
}

// ---------------------------------------------------------------------------
// Refresh lock — only one refresh at a time
// ---------------------------------------------------------------------------

let refreshPromise: Promise<string> | null = null;
let refreshFailedAt = 0;
const REFRESH_COOLDOWN_MS = 5000;

async function doRefresh(): Promise<string> {
  const refreshToken = localStorage.getItem('refresh_token');
  if (!refreshToken) throw new Error('no refresh token');

  const res = await fetch(`${BASE}/auth/refresh`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ refresh_token: refreshToken }),
  });
  if (!res.ok) throw new Error(`refresh failed: ${res.status}`);

  const data = await res.json();
  setTokens(data.access_token, data.refresh_token);
  return data.access_token as string;
}

// ---------------------------------------------------------------------------
// Core fetch wrapper
// ---------------------------------------------------------------------------

interface FetchOptions extends Omit<RequestInit, 'method'> {
  params?: Record<string, string | number | boolean | undefined>;
}

function buildURL(path: string, params?: Record<string, string | number | boolean | undefined>): string {
  const url = new URL(`${BASE}${path}`, window.location.origin);
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      if (v !== undefined) url.searchParams.set(k, String(v));
    }
  }
  return url.toString();
}

function authHeaders(): Record<string, string> {
  const token = getAccessToken();
  return token ? { Authorization: `Bearer ${token}` } : {};
}

async function apiFetch<T>(path: string, method: string, options: FetchOptions = {}): Promise<T> {
  const { params, ...rest } = options;
  const url = buildURL(path, params);

  const headers: Record<string, string> = {
    ...authHeaders(),
    ...(rest.headers as Record<string, string> | undefined),
  };
  if (rest.body && !headers['Content-Type']) {
    headers['Content-Type'] = 'application/json';
  }

  let res = await fetch(url, { ...rest, method, headers });

  // 401 → try refresh once
  if (res.status === 401 && !path.includes('/auth/')) {
    // Cooldown check
    if (Date.now() - refreshFailedAt < REFRESH_COOLDOWN_MS) {
      clearTokens();
      window.location.href = '/login';
      throw new Error('session expired');
    }

    try {
      if (!refreshPromise) {
        refreshPromise = doRefresh().finally(() => { refreshPromise = null; });
      }
      const newToken = await refreshPromise;
      headers.Authorization = `Bearer ${newToken}`;
      res = await fetch(url, { ...rest, method, headers });
    } catch {
      refreshFailedAt = Date.now();
      clearTokens();
      window.location.href = '/login';
      throw new Error('session expired');
    }
  }

  // Handle non-OK responses
  if (!res.ok) {
    let body: unknown;
    try { body = await res.json(); } catch { /* not JSON */ }
    const err = new ApiError(res.status, body);
    throw err;
  }

  // 204 No Content
  if (res.status === 204) return undefined as T;

  const contentType = res.headers.get('content-type') ?? '';
  if (contentType.includes('application/json')) {
    return res.json() as Promise<T>;
  }
  return res.text() as unknown as Promise<T>;
}

// ---------------------------------------------------------------------------
// Public helpers
// ---------------------------------------------------------------------------

export class ApiError extends Error {
  status: number;
  body: unknown;
  constructor(status: number, body: unknown) {
    super(`API error ${status}`);
    this.status = status;
    this.body = body;
  }
}

export function isApiError(err: unknown): err is ApiError {
  return err instanceof ApiError;
}

export async function apiGet<T>(path: string, params?: Record<string, string | number | boolean | undefined>): Promise<T> {
  return apiFetch<T>(path, 'GET', { params });
}

/** GET that also returns response headers (e.g. for x-next-cursor pagination). */
export async function apiGetWithHeaders<T>(path: string, params?: Record<string, string | number | boolean | undefined>): Promise<{ data: T; headers: Headers }> {
  const { params: p } = { params };
  const url = buildURL(path, p);
  const headers: Record<string, string> = { ...authHeaders() };

  let res = await fetch(url, { method: 'GET', headers });

  if (res.status === 401 && !path.includes('/auth/')) {
    if (Date.now() - refreshFailedAt < REFRESH_COOLDOWN_MS) {
      clearTokens();
      window.location.href = '/login';
      throw new Error('session expired');
    }
    try {
      if (!refreshPromise) {
        refreshPromise = doRefresh().finally(() => { refreshPromise = null; });
      }
      const newToken = await refreshPromise;
      headers.Authorization = `Bearer ${newToken}`;
      res = await fetch(url, { method: 'GET', headers });
    } catch {
      refreshFailedAt = Date.now();
      clearTokens();
      window.location.href = '/login';
      throw new Error('session expired');
    }
  }

  if (!res.ok) {
    let body: unknown;
    try { body = await res.json(); } catch { /* not JSON */ }
    throw new ApiError(res.status, body);
  }

  const data = (res.headers.get('content-type') ?? '').includes('application/json')
    ? await res.json() as T
    : await res.text() as unknown as T;
  return { data, headers: res.headers };
}

export async function apiPost<T>(path: string, body?: unknown, extraHeaders?: Record<string, string>): Promise<T> {
  return apiFetch<T>(path, 'POST', {
    body: body !== undefined ? JSON.stringify(body) : undefined,
    headers: extraHeaders,
  });
}

export async function apiPostRaw<T>(path: string, body: string | FormData, extraHeaders?: Record<string, string>): Promise<T> {
  return apiFetch<T>(path, 'POST', { body, headers: extraHeaders });
}

export async function apiPatch<T>(path: string, body?: unknown): Promise<T> {
  return apiFetch<T>(path, 'PATCH', { body: body !== undefined ? JSON.stringify(body) : undefined });
}

export async function apiDel<T = void>(path: string): Promise<T> {
  return apiFetch<T>(path, 'DELETE');
}

// Legacy default export for gradual migration
const api = { get: apiGet, post: apiPost, patch: apiPatch, delete: apiDel };
export default api;