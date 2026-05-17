import { apiGet, apiGetWithHeaders, apiPost, apiPatch, apiDel } from './client';
import type { components } from './schema';

export type Entry = components['schemas']['Entry'];
export type EntrySummary = components['schemas']['EntrySummary'];
export type ListParams = components['schemas']['ListParams'];

export interface ListResponse {
  entries: EntrySummary[];
  next_cursor: string | null;
  has_more: boolean;
}

export async function listEntries(params: ListParams = {}): Promise<ListResponse> {
  const { data, headers } = await apiGetWithHeaders<EntrySummary[]>('/entries', params as Record<string, string | number | boolean | undefined>);
  const nextCursor = headers.get('x-next-cursor') || null;
  return {
    entries: data,
    next_cursor: nextCursor,
    has_more: nextCursor !== null,
  };
}

export async function getEntry(id: string): Promise<Entry> {
  return apiGet<Entry>(`/entries/${id}`);
}

export type CreateEntryOptions = Pick<components['schemas']['CreateEntryRequest'], 'title' | 'tag'>;

export async function createEntry(url: string, options?: CreateEntryOptions): Promise<Entry> {
  return apiPost<Entry>('/entries', { url, ...options });
}

export async function updateEntry(id: string, data: Partial<Pick<Entry, 'title' | 'content' | 'is_archived' | 'is_starred'>>): Promise<Entry> {
  return apiPatch<Entry>(`/entries/${id}`, data);
}

export async function deleteEntry(id: string): Promise<void> {
  await apiDel(`/entries/${id}`);
}

export async function refetchEntry(id: string): Promise<void> {
  await apiPost(`/entries/${id}/refetch`);
}