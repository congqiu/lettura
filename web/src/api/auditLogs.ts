import { apiGet } from './client';
import type { components } from './schema';

export type AuditAction = components['schemas']['AuditAction'];
export type AuditResourceType = components['schemas']['AuditResourceType'];
export type AuditLog = components['schemas']['AuditLog'];
export type ListAuditLogsResponse = components['schemas']['ListAuditLogsResponse'];

export interface ListAuditLogsParams {
  action?: AuditAction;
  resource_type?: AuditResourceType;
  resource_id?: string;
  status?: string;
  limit?: number;
  offset?: number;
}

export async function listAuditLogs(params: ListAuditLogsParams = {}): Promise<ListAuditLogsResponse> {
  return apiGet<ListAuditLogsResponse>('/audit-logs', params as Record<string, string | number | boolean | undefined>);
}