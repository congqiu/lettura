import { apiGet, apiPost, apiDel } from './client';
import type { components } from './schema';

export type FetchJobStatus = components['schemas']['FetchJobStatus'];
export type FetchJob = components['schemas']['FetchJobRow'];
export type RetryAllDeadResponse = components['schemas']['RetryAllResponse'];

export async function listFetchJobs(
  status?: FetchJobStatus,
  limit = 100,
): Promise<FetchJob[]> {
  const res = await apiGet<components['schemas']['ListResponse']>('/admin/fetch-jobs', { status, limit });
  return res.items;
}

export async function getFetchJob(id: string): Promise<FetchJob> {
  return apiGet<FetchJob>(`/admin/fetch-jobs/${id}`);
}

export async function retryFetchJob(id: string): Promise<void> {
  await apiPost(`/admin/fetch-jobs/${id}/retry`);
}

export async function retryAllDeadFetchJobs(): Promise<RetryAllDeadResponse> {
  return apiPost<RetryAllDeadResponse>('/admin/fetch-jobs/retry-all-dead');
}

export async function deleteFetchJob(id: string): Promise<void> {
  await apiDel(`/admin/fetch-jobs/${id}`);
}