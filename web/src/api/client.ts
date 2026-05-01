import axios from 'axios';

const api = axios.create({
  baseURL: '/api/v1',
});

api.interceptors.request.use((config) => {
  const token = localStorage.getItem('access_token');
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

// Refresh lock: only one refresh request at a time
let refreshPromise: Promise<string> | null = null;
let refreshFailedAt: number = 0;
const REFRESH_COOLDOWN_MS = 5000;

// Uses raw axios (not the `api` instance) to avoid triggering the 401 interceptor on refresh requests
async function doRefresh(): Promise<string> {
  const refreshToken = localStorage.getItem('refresh_token');
  if (!refreshToken) {
    throw new Error('no refresh token');
  }
  const res = await axios.post('/api/v1/auth/refresh', {
    refresh_token: refreshToken,
  });
  const { access_token, refresh_token } = res.data;
  localStorage.setItem('access_token', access_token);
  localStorage.setItem('refresh_token', refresh_token);
  return access_token;
}

api.interceptors.response.use(
  (response) => response,
  async (error) => {
    const originalRequest = error.config;
    if (error.response?.status === 401 && !originalRequest._retry) {
      // Skip token refresh for auth endpoints — they don't have tokens to refresh
      if (originalRequest.url?.includes('/auth/')) {
        return Promise.reject(error);
      }
      originalRequest._retry = true;

      // Cooldown check: if refresh failed recently, skip straight to login
      if (Date.now() - refreshFailedAt < REFRESH_COOLDOWN_MS) {
        localStorage.removeItem('access_token');
        localStorage.removeItem('refresh_token');
        window.location.href = '/login';
        return Promise.reject(error);
      }

      try {
        // If a refresh is already in progress, wait for it
        if (!refreshPromise) {
          refreshPromise = doRefresh().finally(() => {
            refreshPromise = null;
          });
        }
        const newToken = await refreshPromise;
        originalRequest.headers.Authorization = `Bearer ${newToken}`;
        return api(originalRequest);
      } catch {
        refreshFailedAt = Date.now();
        localStorage.removeItem('access_token');
        localStorage.removeItem('refresh_token');
        window.location.href = '/login';
      }
    }
    return Promise.reject(error);
  }
);

export default api;
