// Shared types for the extension

export interface AuthTokens {
  access_token: string;
  refresh_token?: string;
}

export interface Entry {
  id: string;
  url: string;
  title: string | null;
  domain_name: string | null;
  is_archived: boolean;
  is_starred: boolean;
  created_at: string;
}

export interface Memo {
  id: string;
  content: string;
  source_url: string | null;
  created_at: string;
}

export interface ApiResponse<T> {
  data: T;
}

export interface SaveRequest {
  url: string;
  tags?: string[];
}

export interface CreateMemoRequest {
  content: string;
  source_url?: string;
}

export type BadgeStatus = 'OK' | 'DUP' | 'ERR' | '!';

export type AuthMode = 'jwt' | 'pat';
