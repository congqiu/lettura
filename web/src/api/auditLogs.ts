import api from './client';

export type AuditAction =
  | 'register' | 'login' | 'logout' | 'refresh_token' | 'change_password'
  | 'regenerate_feed_token' | 'create_pat' | 'delete_pat'
  | 'create_entry' | 'update_entry' | 'soft_delete_entry' | 'restore_entry'
  | 'permanent_delete_entry' | 'archive_entry' | 'unarchive_entry'
  | 'star_entry' | 'unstar_entry' | 'refetch_entry'
  | 'create_tag' | 'delete_tag' | 'add_tag_to_entry' | 'remove_tag_from_entry'
  | 'create_annotation' | 'update_annotation' | 'delete_annotation'
  | 'create_memo' | 'delete_memo' | 'promote_memo'
  | 'create_tagging_rule' | 'update_tagging_rule' | 'delete_tagging_rule'
  | 'create_site_rule' | 'update_site_rule' | 'delete_site_rule'
  | 'import_wallabag' | 'import_browser' | 'export_all'
  | 'create_page' | 'update_page' | 'delete_page' | 'restore_page'
  | 'admin_backup' | 'admin_restore' | 'admin_reindex' | 'admin_list_users'
  | 'bulk_tag_add' | 'bulk_untag' | 'bulk_archive' | 'bulk_star';

export type AuditResourceType =
  | 'user' | 'entry' | 'tag' | 'annotation' | 'memo'
  | 'tagging_rule' | 'site_rule' | 'page' | 'pat' | 'system';

export interface AuditLog {
  id: string;
  user_id: string | null;
  auth_source: 'jwt' | 'pat';
  action: AuditAction;
  resource_type: AuditResourceType | null;
  resource_id: string | null;
  status: 'success' | 'failure' | 'forbidden';
  details: Record<string, unknown>;
  error_message: string | null;
  ip_address: string | null;
  user_agent: string | null;
  request_id: string | null;
  created_at: string;
}

export interface ListAuditLogsResponse {
  data: AuditLog[];
  total: number;
  limit: number;
  offset: number;
}

export interface ListAuditLogsParams {
  action?: AuditAction;
  resource_type?: AuditResourceType;
  resource_id?: string;
  status?: string;
  limit?: number;
  offset?: number;
}

export async function listAuditLogs(params: ListAuditLogsParams = {}): Promise<ListAuditLogsResponse> {
  const res = await api.get('/audit-logs', { params });
  return res.data;
}
