import api from './client';

export interface Tag {
  id: string;
  label: string;
  slug: string;
  created_at: string;
}

export async function listTags(): Promise<Tag[]> {
  const res = await api.get('/tags');
  return res.data;
}

export async function addTagToEntry(entryId: string, label: string): Promise<Tag> {
  const res = await api.post(`/entries/${entryId}/tags`, { label });
  return res.data;
}

export async function removeTagFromEntry(entryId: string, tagId: string): Promise<void> {
  await api.delete(`/entries/${entryId}/tags/${tagId}`);
}

export async function deleteTag(tagId: string): Promise<void> {
  await api.delete(`/tags/${tagId}`);
}
