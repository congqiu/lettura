import { apiGet, apiPost, apiPostRaw, apiPatch, apiDel } from './client';
import type { components } from './schema';

export type PageSummary = components['schemas']['PageSummaryResponse'];
export type Page = components['schemas']['PageResponse'];
export type PageListResponse = components['schemas']['PageListResponse'];
export type UploadResponse = components['schemas']['UploadResponse'];
export type CreatePageRequest = components['schemas']['CreatePageRequest'];
export type UpdatePageRequest = components['schemas']['UpdatePageRequest'];

export async function uploadFiles(files: File[]): Promise<UploadResponse> {
  const formData = new FormData();
  files.forEach(f => formData.append('files', f));
  return apiPostRaw<UploadResponse>('/pages/upload', formData);
}

export async function createPage(data: CreatePageRequest): Promise<Page> {
  return apiPost<Page>('/pages', data);
}

export interface ListPagesParams {
  status?: string;
  page?: number;
  limit?: number;
}

export async function listPages(params?: ListPagesParams): Promise<PageListResponse> {
  return apiGet<PageListResponse>('/pages', params as Record<string, string | number | boolean | undefined>);
}

export async function updatePage(
  id: string,
  data: Partial<UpdatePageRequest>
): Promise<Page> {
  return apiPatch<Page>(`/pages/${id}`, data);
}

export async function deletePage(id: string): Promise<void> {
  await apiDel(`/pages/${id}`);
}

export async function restorePage(id: string): Promise<void> {
  await apiPost(`/pages/${id}/restore`);
}