import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { updatePage, deletePage, restorePage, type PageSummary } from '../api/pages';
import { ExternalLink, Copy, Lock, Trash2, RotateCcw, EyeOff, Pencil, Clock } from 'lucide-react';
import { toast } from 'sonner';
import { timeAgo } from '../utils/time';
import PageEditModal from './PageEditModal';
import { Button } from './ui/button';
import { cn } from '@/lib/utils';

interface PageCardProps {
  page: PageSummary;
}

function formatExpiry(expiresAt: string): { text: string; urgent: boolean } {
  const now = Date.now();
  const end = new Date(expiresAt).getTime();
  const diff = end - now;
  if (diff <= 0) return { text: '已过期', urgent: true };
  const minutes = Math.floor(diff / 60000);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);
  if (days > 0) return { text: `${days}天后到期`, urgent: days <= 1 };
  if (hours > 0) return { text: `${hours}小时后到期`, urgent: true };
  return { text: `${minutes}分钟后到期`, urgent: true };
}

export default function PageCard({ page }: PageCardProps) {
  const qc = useQueryClient();
  const [editOpen, setEditOpen] = useState(false);
  const pageUrl = `${window.location.origin}/p/${page.slug}`;

  const handleCopyLink = () => {
    navigator.clipboard.writeText(pageUrl);
    toast.success('链接已复制');
  };

  const toggleStatus = useMutation({
    mutationFn: () => updatePage(page.id, { status: page.status === 'active' ? 'disabled' : 'active' }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['pages'] });
      toast.success(page.status === 'active' ? '已禁用' : '已启用');
    },
    onError: () => toast.error('操作失败，请重试'),
  });

  const handleDelete = useMutation({
    mutationFn: () => deletePage(page.id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['pages'] });
      toast.success('已删除');
    },
    onError: () => toast.error('删除失败，请重试'),
  });

  const handleRestore = useMutation({
    mutationFn: () => restorePage(page.id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['pages'] });
      toast.success('已恢复');
    },
    onError: () => toast.error('恢复失败，请重试'),
  });

  const isDeleted = page.status === 'deleted';
  const expiry = page.expires_at ? formatExpiry(page.expires_at) : null;

  return (
    <>
      <div className="bg-card border border-border/60 rounded-xl p-4 hover:border-border transition-colors">
        <div className="flex items-start justify-between gap-3">
          <div className="flex-1 min-w-0">
            <h3 className="font-semibold text-foreground text-sm sm:text-[15px] truncate">
              {page.title}
            </h3>
            <div className="flex items-center gap-2 mt-1.5 text-[11px] text-muted-foreground flex-wrap">
              <button
                onClick={() => window.open(pageUrl, '_blank')}
                className="flex items-center gap-1 hover:text-primary font-mono text-[11px] transition-colors"
              >
                /p/{page.slug}
              </button>
              {page.has_password && <Lock size={11} className="text-warning shrink-0" />}
              <span>{page.file_count} 个文件</span>
              <span>{timeAgo(page.created_at)}</span>
              {page.status === 'disabled' && (
                <span className="text-warning font-medium">已禁用</span>
              )}
              {expiry && (
                <span className={cn(
                  'flex items-center gap-0.5',
                  expiry.urgent ? 'text-destructive' : 'text-muted-foreground'
                )}>
                  <Clock size={11} />
                  {expiry.text}
                </span>
              )}
            </div>
          </div>
          <div className="flex items-center gap-0.5 shrink-0">
            {!isDeleted && (
              <>
                <Button variant="ghost" size="icon" onClick={() => window.open(pageUrl, '_blank')} title="新窗口打开" className="h-8 w-8 rounded-lg">
                  <ExternalLink size={14} />
                </Button>
                <Button variant="ghost" size="icon" onClick={handleCopyLink} title="复制链接" className="h-8 w-8 rounded-lg">
                  <Copy size={14} />
                </Button>
                <Button variant="ghost" size="icon" onClick={() => setEditOpen(true)} title="编辑" className="h-8 w-8 rounded-lg">
                  <Pencil size={14} />
                </Button>
                <Button variant="ghost" size="icon" onClick={() => toggleStatus.mutate()} title={page.status === 'active' ? '禁用' : '启用'} className="h-8 w-8 rounded-lg">
                  <EyeOff size={14} />
                </Button>
                <Button variant="ghost" size="icon" onClick={() => handleDelete.mutate()} title="删除" className="h-8 w-8 rounded-lg hover:text-destructive">
                  <Trash2 size={14} />
                </Button>
              </>
            )}
            {isDeleted && (
              <Button variant="ghost" size="icon" onClick={() => handleRestore.mutate()} title="恢复" className="h-8 w-8 rounded-lg hover:text-success">
                <RotateCcw size={14} />
              </Button>
            )}
          </div>
        </div>
      </div>
      <PageEditModal page={page} open={editOpen} onClose={() => setEditOpen(false)} />
    </>
  );
}
