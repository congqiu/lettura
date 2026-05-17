import { apiGet, apiPost, apiDel } from './client';
import type { components } from './schema';

export type TokenRow = components['schemas']['PersonalAccessToken'];
export type CreatedToken = components['schemas']['CreateTokenResponse'];
export type CreateTokenPayload = components['schemas']['CreateTokenRequest'];

export async function listTokens(): Promise<TokenRow[]> {
  return apiGet<TokenRow[]>('/tokens');
}

export async function createToken(body: CreateTokenPayload): Promise<CreatedToken> {
  return apiPost<CreatedToken>('/tokens', body);
}

export async function deleteToken(id: string): Promise<void> {
  await apiDel(`/tokens/${id}`);
}