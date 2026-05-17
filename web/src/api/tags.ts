import { apiGet, apiPost, apiPatch, apiDel } from './client';
import type { components } from './schema';

export type Tag = components['schemas']['Tag'];
export type TagStats = components['schemas']['TagStats'];

export async function listTags(): Promise<Tag[]> {
  return apiGet<Tag[]>('/tags');
}

export async function fetchTagStats(): Promise<TagStats[]> {
  return apiGet<TagStats[]>('/tags/stats');
}

export async function renameTag(id: string, label: string): Promise<Tag> {
  return apiPatch<Tag>(`/tags/${id}`, { label });
}

export async function addTagToEntry(entryId: string, label: string): Promise<Tag> {
  return apiPost<Tag>(`/entries/${entryId}/tags`, { label });
}

export async function removeTagFromEntry(entryId: string, tagId: string): Promise<void> {
  await apiDel(`/entries/${entryId}/tags/${tagId}`);
}

export async function deleteTag(tagId: string): Promise<void> {
  await apiDel(`/tags/${tagId}`);
}

export async function bulkTagByIds(entryIds: string[], tags: string[]): Promise<void> {
  await apiPost('/entries/bulk/tag-by-ids', { entry_ids: entryIds, tags });
}

export async function bulkUntagByIds(entryIds: string[], tags: string[]): Promise<void> {
  await apiPost('/entries/bulk/untag-by-ids', { entry_ids: entryIds, tags });
}

export async function bulkDeleteByIds(entryIds: string[]): Promise<void> {
  await apiPost('/entries/bulk/delete-by-ids', { entry_ids: entryIds });
}

export async function bulkArchiveByIds(entryIds: string[]): Promise<void> {
  await apiPost('/entries/bulk/archive-by-ids', { entry_ids: entryIds });
}