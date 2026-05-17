import { apiGet, apiPost, apiDel } from './client';
import type { components } from './schema';

export type Memo = components['schemas']['Memo'];

export async function listMemos(): Promise<Memo[]> {
  return apiGet<Memo[]>('/memos');
}

export async function createMemo(content: string, source_url?: string): Promise<Memo> {
  return apiPost<Memo>('/memos', { content, source_url });
}

export async function deleteMemo(id: string): Promise<void> {
  await apiDel(`/memos/${id}`);
}

export async function promoteMemo(id: string): Promise<{ entry_id: string }> {
  return apiPost(`/memos/${id}/promote`);
}