import { apiPost } from './client';
import type { components } from './schema';

export type AuthResponse = components['schemas']['AuthResponse'];

export async function register(username: string, email: string, password: string): Promise<AuthResponse> {
  return apiPost('/auth/register', { username, email, password });
}

export async function login(email: string, password: string): Promise<AuthResponse> {
  return apiPost('/auth/login', { email, password });
}

export async function logout(refreshToken: string): Promise<void> {
  await apiPost('/auth/logout', { refresh_token: refreshToken });
}