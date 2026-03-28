import api from './client';

export interface AuthResponse {
  access_token: string;
  refresh_token: string;
  token_type: string;
  expires_in: number;
}

export async function register(username: string, email: string, password: string): Promise<AuthResponse> {
  const res = await api.post('/auth/register', { username, email, password });
  return res.data;
}

export async function login(email: string, password: string): Promise<AuthResponse> {
  const res = await api.post('/auth/login', { email, password });
  return res.data;
}

export async function logout(refreshToken: string): Promise<void> {
  await api.post('/auth/logout', { refresh_token: refreshToken });
}
