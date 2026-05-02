import api from './client';

export interface Entry {
  id: string;
  url: string;
  given_url: string;
  title: string | null;
  content: string | null;
  text_content: string | null;
  content_type: string;
  extract_method: string;
  language: string | null;
  reading_time: number | null;
  preview_picture: string | null;
  domain_name: string | null;
  published_by: string | null;
  is_archived: boolean;
  is_starred: boolean;
  created_at: string;
}

export interface EntrySummary {
  id: string;
  url: string;
  title: string | null;
  content_type: string;
  extract_method: string;
  language: string | null;
  reading_time: number | null;
  preview_picture: string | null;
  domain_name: string | null;
  is_archived: boolean;
  is_starred: boolean;
  created_at: string;
}

export interface ListParams {
  cursor?: string;
  page?: number;
  per_page?: number;
  is_archived?: boolean;
  is_starred?: boolean;
  search?: string;
  domain?: string;
  tags?: string[];
}

export interface ListResponse {
  entries: EntrySummary[];
  next_cursor: string | null;
  has_more: boolean;
}

export async function listEntries(params: ListParams = {}): Promise<ListResponse> {
  const res = await api.get('/entries', { params });
  // Extract cursor from response header
  const nextCursor = res.headers['x-next-cursor'] || null;
  return {
    entries: res.data,
    next_cursor: nextCursor,
    has_more: nextCursor !== null,
  };
}

export async function getEntry(id: string): Promise<Entry> {
  const res = await api.get(`/entries/${id}`);
  return res.data;
}

export async function createEntry(url: string): Promise<Entry> {
  const res = await api.post('/entries', { url });
  return res.data;
}

export async function updateEntry(id: string, data: Partial<Pick<Entry, 'title' | 'content' | 'is_archived' | 'is_starred'>>): Promise<Entry> {
  const res = await api.patch(`/entries/${id}`, data);
  return res.data;
}

export async function deleteEntry(id: string): Promise<void> {
  await api.delete(`/entries/${id}`);
}

export async function refetchEntry(id: string): Promise<void> {
  await api.post(`/entries/${id}/refetch`);
}
