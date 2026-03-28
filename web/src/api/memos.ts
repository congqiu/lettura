import api from './client';

export interface Memo {
  id: string;
  content: string;
  source_url: string | null;
  promoted_entry_id: string | null;
  created_at: string;
}

export async function listMemos(): Promise<Memo[]> {
  const res = await api.get('/memos');
  return res.data;
}

export async function createMemo(content: string, source_url?: string): Promise<Memo> {
  const res = await api.post('/memos', { content, source_url });
  return res.data;
}

export async function deleteMemo(id: string): Promise<void> {
  await api.delete(`/memos/${id}`);
}

export async function promoteMemo(id: string): Promise<{ entry_id: string }> {
  const res = await api.post(`/memos/${id}/promote`);
  return res.data;
}
