import api from './client';

export interface Tag {
  id: string;
  label: string;
  slug: string;
  created_at: string;
}

export interface TagStats {
  id: string;
  label: string;
  slug: string;
  entry_count: number;
  created_at: string;
}

export async function listTags(): Promise<Tag[]> {
  const res = await api.get('/tags');
  return res.data;
}

export async function fetchTagStats(): Promise<TagStats[]> {
  const res = await api.get('/tags/stats');
  return res.data;
}

export async function renameTag(id: string, label: string): Promise<Tag> {
  const res = await api.patch(`/tags/${id}`, { label });
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

export async function bulkTagByIds(entryIds: string[], tags: string[]): Promise<void> {
  await api.post('/entries/bulk/tag-by-ids', { entry_ids: entryIds, tags });
}

export async function bulkUntagByIds(entryIds: string[], tags: string[]): Promise<void> {
  await api.post('/entries/bulk/untag-by-ids', { entry_ids: entryIds, tags });
}

export async function bulkDeleteByIds(entryIds: string[]): Promise<void> {
  await api.post('/entries/bulk/delete-by-ids', { entry_ids: entryIds });
}

export async function bulkArchiveByIds(entryIds: string[]): Promise<void> {
  await api.post('/entries/bulk/archive-by-ids', { entry_ids: entryIds });
}
