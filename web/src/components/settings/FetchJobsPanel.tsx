import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { toast } from 'sonner';
import { Inbox, RotateCw, Trash2 } from 'lucide-react';
import {
  listFetchJobs,
  retryFetchJob,
  retryAllDeadFetchJobs,
  deleteFetchJob,
  type FetchJob,
  type FetchJobStatus,
} from '@/api/fetchJobs';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import ConfirmDialog from '@/components/ConfirmDialog';
import { cn } from '@/lib/utils';

const STATUSES: { key: FetchJobStatus; label: string }[] = [
  { key: 'failed', label: '失败重试中' },
  { key: 'dead', label: '死信' },
  { key: 'running', label: '运行中' },
  { key: 'pending', label: '等待中' },
];

const STATUS_BADGE: Record<FetchJobStatus, 'default' | 'secondary' | 'destructive' | 'outline'> = {
  pending: 'secondary',
  running: 'default',
  failed: 'outline',
  dead: 'destructive',
};

function formatDateTime(value: string | null): string {
  if (!value) return '—';
  return new Date(value).toLocaleString();
}

export default function FetchJobsPanel() {
  const queryClient = useQueryClient();
  const [status, setStatus] = useState<FetchJobStatus>('failed');
  const [deleteId, setDeleteId] = useState<string | null>(null);

  const { data, isLoading, isError, error, refetch } = useQuery({
    queryKey: ['fetch-jobs', status],
    queryFn: () => listFetchJobs(status),
    refetchInterval: 5000,
  });

  const retryOne = useMutation({
    mutationFn: retryFetchJob,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['fetch-jobs'] });
      toast.success('已重新入队');
    },
    onError: (e: unknown) => {
      const msg = e instanceof Error ? e.message : '未知错误';
      toast.error(`重试失败：${msg}`);
    },
  });

  const retryAll = useMutation({
    mutationFn: retryAllDeadFetchJobs,
    onSuccess: (resp) => {
      queryClient.invalidateQueries({ queryKey: ['fetch-jobs'] });
      const tail =
        resp.remaining_dead > 0
          ? `（还有 ${resp.remaining_dead} 个未复活，可再次点击继续处理）`
          : '';
      toast.success(`已复活 ${resp.retried} 个死信任务${tail}`);
    },
    onError: (e: unknown) => {
      const msg = e instanceof Error ? e.message : '未知错误';
      toast.error(`批量复活失败：${msg}`);
    },
  });

  const removeJob = useMutation({
    mutationFn: deleteFetchJob,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['fetch-jobs'] });
      toast.success('任务已删除');
      setDeleteId(null);
    },
    onError: (e: unknown) => {
      const msg = e instanceof Error ? e.message : '未知错误';
      toast.error(`删除失败：${msg}`);
      setDeleteId(null);
    },
  });

  if (isError) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-center border border-dashed border-destructive/50 rounded-lg bg-destructive/5">
        <div className="w-12 h-12 rounded-full bg-destructive/10 flex items-center justify-center mb-4">
          <Inbox size={24} className="text-destructive" />
        </div>
        <h4 className="font-semibold text-base text-destructive mb-1">无法加载抓取队列</h4>
        <p className="text-sm text-muted-foreground mb-4">
          {error instanceof Error ? error.message : '请确认你拥有管理员权限。'}
        </p>
        <Button variant="outline" onClick={() => refetch()}>
          重试
        </Button>
      </div>
    );
  }

  const items = data ?? [];

  return (
    <div className="space-y-4">
      {/* Status tabs + bulk action */}
      <div className="flex flex-wrap items-center gap-2">
        <div className="flex flex-wrap gap-1.5">
          {STATUSES.map((s) => {
            const isActive = s.key === status;
            return (
              <button
                key={s.key}
                onClick={() => setStatus(s.key)}
                className={cn(
                  'px-3 py-1.5 rounded-md text-sm font-medium transition-colors',
                  isActive
                    ? 'bg-primary text-primary-foreground'
                    : 'bg-muted/50 text-muted-foreground hover:bg-muted hover:text-foreground',
                )}
              >
                {s.label}
              </button>
            );
          })}
        </div>
        {status === 'dead' && items.length > 0 && (
          <Button
            variant="destructive"
            size="sm"
            className="ml-auto"
            onClick={() => retryAll.mutate()}
            disabled={retryAll.isPending}
          >
            <RotateCw size={14} />
            复活全部死信（最多 100 条/次）
          </Button>
        )}
      </div>

      {isLoading ? (
        <p className="text-sm text-muted-foreground">加载中...</p>
      ) : items.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-12 text-center border border-dashed rounded-lg">
          <div className="w-12 h-12 rounded-full bg-secondary flex items-center justify-center mb-4">
            <Inbox size={24} className="text-muted-foreground" />
          </div>
          <h4 className="font-semibold text-base mb-1">没有「{STATUSES.find((s) => s.key === status)?.label}」状态的任务</h4>
          <p className="text-sm text-muted-foreground">列表每 5 秒自动刷新。</p>
        </div>
      ) : (
        <div className="overflow-x-auto rounded-lg border">
          <table className="w-full text-sm">
            <thead className="bg-muted/40 text-muted-foreground">
              <tr className="text-left">
                <th className="px-3 py-2 font-medium">URL</th>
                <th className="px-3 py-2 font-medium whitespace-nowrap">状态</th>
                <th className="px-3 py-2 font-medium whitespace-nowrap">尝试</th>
                <th className="px-3 py-2 font-medium">最后错误</th>
                <th className="px-3 py-2 font-medium whitespace-nowrap">更新时间</th>
                <th className="px-3 py-2 font-medium whitespace-nowrap text-right">操作</th>
              </tr>
            </thead>
            <tbody>
              {items.map((job: FetchJob) => (
                <tr key={job.id} className="border-t align-top">
                  <td className="px-3 py-2 max-w-xs">
                    <div className="truncate" title={job.url}>
                      {job.url}
                    </div>
                    <div className="mt-0.5 text-xs text-muted-foreground font-mono truncate" title={job.entry_id}>
                      entry: {job.entry_id.slice(0, 8)}…
                    </div>
                  </td>
                  <td className="px-3 py-2">
                    <Badge variant={STATUS_BADGE[job.status]}>{job.status}</Badge>
                  </td>
                  <td className="px-3 py-2 whitespace-nowrap">
                    <Badge variant="secondary">
                      {job.attempts}/{job.max_attempts}
                    </Badge>
                  </td>
                  <td className="px-3 py-2 max-w-md">
                    {job.last_error ? (
                      <div className="text-destructive truncate" title={job.last_error}>
                        {job.last_error}
                      </div>
                    ) : (
                      <span className="text-muted-foreground">—</span>
                    )}
                  </td>
                  <td className="px-3 py-2 whitespace-nowrap text-muted-foreground">
                    {formatDateTime(job.last_error_at ?? job.updated_at)}
                  </td>
                  <td className="px-3 py-2 whitespace-nowrap text-right">
                    <div className="inline-flex gap-1.5">
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => retryOne.mutate(job.id)}
                        disabled={retryOne.isPending}
                      >
                        <RotateCw size={14} />
                        重试
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => setDeleteId(job.id)}
                        disabled={removeJob.isPending}
                        aria-label="删除任务"
                      >
                        <Trash2 size={14} className="text-destructive" />
                      </Button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <ConfirmDialog
        open={deleteId !== null}
        title="删除抓取任务？"
        message="删除后该任务将从队列中移除，不再被重试。文章本身不受影响。"
        confirmText="删除"
        variant="danger"
        onConfirm={() => {
          if (deleteId) removeJob.mutate(deleteId);
        }}
        onCancel={() => setDeleteId(null)}
      />
    </div>
  );
}
