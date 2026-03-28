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
  page?: number;
  per_page?: number;
  is_archived?: boolean;
  is_starred?: boolean;
  search?: string;
  domain?: string;
}

export async function listEntries(params: ListParams = {}): Promise<EntrySummary[]> {
  const res = await api.get('/entries', { params });
  return res.data;
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
