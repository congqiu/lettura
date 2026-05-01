import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { listAuditLogs, type AuditAction, type AuditLog } from '../api/auditLogs';
import ErrorState from '../components/ErrorState';
import EmptyState from '../components/EmptyState';
import { Badge } from '../components/ui/badge';
import { Button } from '../components/ui/button';
import { Loader2, Shield, ShieldAlert, ShieldCheck } from 'lucide-react';

const ACTION_LABELS: Record<AuditAction, string> = {
  register: '注册',
  login: '登录',
  logout: '退出',
  refresh_token: '刷新令牌',
  change_password: '修改密码',
  regenerate_feed_token: '重置 Feed Token',
  create_pat: '创建 PAT',
  delete_pat: '删除 PAT',
  create_entry: '保存文章',
  update_entry: '更新文章',
  soft_delete_entry: '删除文章',
  restore_entry: '恢复文章',
  permanent_delete_entry: '永久删除文章',
  archive_entry: '归档文章',
  unarchive_entry: '取消归档',
  star_entry: '收藏文章',
  unstar_entry: '取消收藏',
  refetch_entry: '重新抓取',
  create_tag: '创建标签',
  delete_tag: '删除标签',
  add_tag_to_entry: '添加标签',
  remove_tag_from_entry: '移除标签',
  create_annotation: '创建批注',
  update_annotation: '更新批注',
  delete_annotation: '删除批注',
  create_memo: '创建便签',
  delete_memo: '删除便签',
  promote_memo: '转化便签',
  create_tagging_rule: '创建标签规则',
  update_tagging_rule: '更新标签规则',
  delete_tagging_rule: '删除标签规则',
  create_site_rule: '创建站点规则',
  update_site_rule: '更新站点规则',
  delete_site_rule: '删除站点规则',
  import_wallabag: '导入 Wallabag',
  import_browser: '导入浏览器书签',
  export_all: '导出全部',
  create_page: '创建页面',
  update_page: '更新页面',
  delete_page: '删除页面',
  restore_page: '恢复页面',
  admin_backup: '备份',
  admin_restore: '恢复',
  admin_reindex: '重建索引',
  admin_list_users: '查看用户列表',
  bulk_tag_add: '批量添加标签',
  bulk_untag: '批量移除标签',
  bulk_archive: '批量归档',
  bulk_star: '批量收藏',
};

const PAGE_SIZE = 30;

function StatusBadge({ status }: { status: AuditLog['status'] }) {
  if (status === 'success') {
    return (
      <Badge variant="outline" className="border-green-500 text-green-600 dark:text-green-400 flex items-center gap-1">
        <ShieldCheck size={12} /> 成功
      </Badge>
    );
  }
  if (status === 'forbidden') {
    return (
      <Badge variant="outline" className="border-orange-500 text-orange-600 dark:text-orange-400 flex items-center gap-1">
        <ShieldAlert size={12} /> 拒绝
      </Badge>
    );
  }
  return (
    <Badge variant="outline" className="border-red-500 text-red-600 dark:text-red-400 flex items-center gap-1">
      <Shield size={12} /> 失败
    </Badge>
  );
}

function AuthSourceBadge({ source }: { source: 'jwt' | 'pat' }) {
  return (
    <Badge variant="secondary" className="text-[10px]">
      {source === 'jwt' ? 'JWT' : 'PAT'}
    </Badge>
  );
}

function formatTime(iso: string) {
  const d = new Date(iso);
  return d.toLocaleString('zh-CN', {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

export default function AuditLogsPage() {
  const [offset, setOffset] = useState(0);

  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['audit-logs', offset],
    queryFn: () => listAuditLogs({ limit: PAGE_SIZE, offset }),
  });

  const logs = data?.data ?? [];
  const total = data?.total ?? 0;
  const hasMore = offset + PAGE_SIZE < total;
  const hasPrev = offset > 0;

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-xl font-semibold text-foreground">操作日志</h2>
        <span className="text-sm text-muted-foreground">共 {total} 条</span>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12">
          <Loader2 size={24} className="animate-spin text-muted-foreground" />
        </div>
      ) : error ? (
        <ErrorState onRetry={() => refetch()} />
      ) : logs.length === 0 ? (
        <EmptyState icon="shield" title="暂无操作日志" description="你的操作记录将显示在这里" />
      ) : (
        <>
          <div className="space-y-2">
            {logs.map((log) => (
              <div
                key={log.id}
                className="bg-card border border-border rounded-lg p-3 flex items-center gap-3 hover:border-muted-foreground/20 transition-colors"
              >
                <div className="flex-shrink-0 w-14 text-xs text-muted-foreground text-right leading-tight">
                  {formatTime(log.created_at)}
                </div>

                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="text-sm font-medium text-foreground">
                      {ACTION_LABELS[log.action] ?? log.action}
                    </span>
                    <StatusBadge status={log.status} />
                    <AuthSourceBadge source={log.auth_source as 'jwt' | 'pat'} />
                  </div>
                  {log.resource_type && (
                    <p className="text-xs text-muted-foreground mt-0.5">
                      资源: {log.resource_type}
                      {log.resource_id ? ` (${log.resource_id.slice(0, 8)}...)` : ''}
                    </p>
                  )}
                  {log.error_message && (
                    <p className="text-xs text-red-500 mt-0.5">{log.error_message}</p>
                  )}
                </div>
              </div>
            ))}
          </div>

          <div className="flex items-center justify-between mt-6">
            <Button
              variant="outline"
              size="sm"
              onClick={() => setOffset((o) => Math.max(0, o - PAGE_SIZE))}
              disabled={!hasPrev || isLoading}
            >
              上一页
            </Button>
            <span className="text-sm text-muted-foreground">
              {offset + 1} - {Math.min(offset + PAGE_SIZE, total)} / {total}
            </span>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setOffset((o) => o + PAGE_SIZE)}
              disabled={!hasMore || isLoading}
            >
              下一页
            </Button>
          </div>
        </>
      )}
    </div>
  );
}
