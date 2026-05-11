import api from './client';

export interface PageSummary {
  id: string;
  slug: string;
  title: string;
  description: string | null;
  has_password: boolean;
  status: string;
  file_count: number;
  expires_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface Page {
  id: string;
  slug: string;
  user_id: string;
  title: string;
  description: string | null;
  entry_file: string;
  has_password: boolean;
  status: string;
  file_count: number;
  expires_at: string | null;
  deleted_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface PageListResponse {
  items: PageSummary[];
  total: number;
  page: number;
  limit: number;
}

export interface UploadResponse {
  upload_id: string;
  html_files: string[];
  default_entry: string;
  suggested_title: string;
  file_count: number;
}

export async function uploadFiles(files: File[]): Promise<UploadResponse> {
  const formData = new FormData();
  files.forEach(f => formData.append('files', f));
  const res = await api.post('/pages/upload', formData, {
    headers: { 'Content-Type': 'multipart/form-data' },
  });
  return res.data;
}

export async function createPage(data: {
  upload_id: string;
  entry_file: string;
  title: string;
  description?: string;
  password?: string;
  expires_at?: string;
}): Promise<Page> {
  const res = await api.post('/pages', data);
  return res.data;
}

export async function listPages(params?: {
  status?: string;
  page?: number;
  limit?: number;
}): Promise<PageListResponse> {
  const res = await api.get('/pages', { params });
  return res.data;
}

export async function updatePage(
  id: string,
  data: {
    title?: string;
    description?: string;
    password?: string | null;
    status?: string;
    expires_at?: string | null;
    upload_id?: string;
    entry_file?: string;
  }
): Promise<Page> {
  const res = await api.patch(`/pages/${id}`, data);
  return res.data;
}

export async function deletePage(id: string): Promise<void> {
  await api.delete(`/pages/${id}`);
}

export async function restorePage(id: string): Promise<void> {
  await api.post(`/pages/${id}/restore`);
}
