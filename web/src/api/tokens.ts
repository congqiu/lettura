import api from './client';

export interface TokenRow {
  id: string;
  name: string;
  scope: 'read' | 'write';
  token_prefix: string;
  last_used_at: string | null;
  expires_at: string | null;
  created_at: string;
}

export interface CreatedToken {
  id: string;
  name: string;
  scope: 'read' | 'write';
  token: string;
}

export interface CreateTokenPayload {
  name: string;
  scope: 'read' | 'write';
  expires_in_days: number | null;
}

export async function listTokens(): Promise<TokenRow[]> {
  const res = await api.get('/tokens');
  return res.data;
}

export async function createToken(body: CreateTokenPayload): Promise<CreatedToken> {
  const res = await api.post('/tokens', body);
  return res.data;
}

export async function deleteToken(id: string): Promise<void> {
  await api.delete(`/tokens/${id}`);
}
