import api from './client';

export type FetchJobStatus = 'pending' | 'running' | 'failed' | 'dead';

export interface FetchJob {
  id: string;
  entry_id: string;
  user_id: string;
  url: string;
  status: FetchJobStatus;
  priority: number;
  attempts: number;
  max_attempts: number;
  run_after: string;
  leased_until: string | null;
  leased_by: string | null;
  last_error: string | null;
  last_error_at: string | null;
  refetch_requested_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface ListFetchJobsResponse {
  items: FetchJob[];
}

export interface RetryAllDeadResponse {
  retried: number;
  remaining_dead: number;
}

// Note: api baseURL is `/api/v1`, so paths start with `/admin/...` (not `/api/v1/admin/...`).

export async function listFetchJobs(
  status?: FetchJobStatus,
  limit = 100,
): Promise<FetchJob[]> {
  const res = await api.get<ListFetchJobsResponse>('/admin/fetch-jobs', {
    params: { status, limit },
  });
  return res.data.items;
}

export async function getFetchJob(id: string): Promise<FetchJob> {
  const res = await api.get<FetchJob>(`/admin/fetch-jobs/${id}`);
  return res.data;
}

export async function retryFetchJob(id: string): Promise<void> {
  await api.post(`/admin/fetch-jobs/${id}/retry`);
}

export async function retryAllDeadFetchJobs(): Promise<RetryAllDeadResponse> {
  const res = await api.post<RetryAllDeadResponse>('/admin/fetch-jobs/retry-all-dead');
  return res.data;
}

export async function deleteFetchJob(id: string): Promise<void> {
  await api.delete(`/admin/fetch-jobs/${id}`);
}
