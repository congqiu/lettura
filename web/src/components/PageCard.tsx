import { useMutation, useQueryClient } from '@tanstack/react-query';
import { updatePage, deletePage, restorePage, type PageSummary } from '../api/pages';
import { ExternalLink, Copy, Lock, Trash2, RotateCcw, EyeOff } from 'lucide-react';
import { toast } from './Toast';

function timeAgo(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 60) return `${mins}分钟前`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}小时前`;
  const days = Math.floor(hrs / 24);
  return `${days}天前`;
}

export default function PageCard({ page }: { page: PageSummary }) {
  const qc = useQueryClient();
  const pageUrl = `${window.location.origin}/p/${page.slug}`;

  const toggleStatus = useMutation({
    mutationFn: () => updatePage(page.id, { status: page.status === 'active' ? 'disabled' : 'active' }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['pages'] });
      toast('success', page.status === 'active' ? '已禁用' : '已启用');
    },
  });

  const handleDelete = useMutation({
    mutationFn: () => deletePage(page.id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['pages'] });
      toast('success', '已删除');
    },
  });

  const handleRestore = useMutation({
    mutationFn: () => restorePage(page.id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['pages'] });
      toast('success', '已恢复');
    },
  });

  const copyLink = () => {
    navigator.clipboard.writeText(pageUrl);
    toast('success', '链接已复制');
  };

  const isDeleted = page.status === 'deleted';

  return (
    <div className="bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-xl p-4 hover:shadow-sm transition-all">
      <div className="flex items-start justify-between gap-3">
        <div className="flex-1 min-w-0">
          <h3 className="font-semibold text-gray-900 dark:text-gray-100 text-sm sm:text-base truncate">
            {page.title}
          </h3>
          <div className="flex items-center gap-2 mt-1 text-xs text-gray-500 dark:text-gray-500 flex-wrap">
            <button
              onClick={() => window.open(pageUrl, '_blank')}
              className="flex items-center gap-1 hover:text-blue-600 dark:hover:text-blue-400 font-mono"
            >
              /p/{page.slug}
            </button>
            {page.has_password && <Lock size={11} className="text-yellow-500 shrink-0" />}
            <span>{page.file_count} 个文件</span>
            <span>{timeAgo(page.created_at)}</span>
            {page.status === 'disabled' && (
              <span className="text-yellow-600 dark:text-yellow-500">已禁用</span>
            )}
          </div>
        </div>
        <div className="flex items-center gap-0.5 shrink-0">
          {!isDeleted && (
            <>
              <a
                href={pageUrl}
                target="_blank"
                rel="noopener noreferrer"
                className="p-2 text-gray-400 dark:text-gray-600 hover:text-gray-600 dark:hover:text-gray-300 rounded-md transition-colors"
                title="新窗口打开"
              >
                <ExternalLink size={15} />
              </a>
              <button
                onClick={copyLink}
                className="p-2 text-gray-400 dark:text-gray-600 hover:text-gray-600 dark:hover:text-gray-300 rounded-md transition-colors"
                title="复制链接"
              >
                <Copy size={15} />
              </button>
              <button
                onClick={() => toggleStatus.mutate()}
                className="p-2 text-gray-400 dark:text-gray-600 hover:text-gray-600 dark:hover:text-gray-300 rounded-md transition-colors"
                title={page.status === 'active' ? '禁用' : '启用'}
              >
                <EyeOff size={15} />
              </button>
              <button
                onClick={() => handleDelete.mutate()}
                className="p-2 text-gray-400 dark:text-gray-600 hover:text-red-500 rounded-md transition-colors"
                title="删除"
              >
                <Trash2 size={15} />
              </button>
            </>
          )}
          {isDeleted && (
            <button
              onClick={() => handleRestore.mutate()}
              className="p-2 text-gray-400 dark:text-gray-600 hover:text-green-500 rounded-md transition-colors"
              title="恢复"
            >
              <RotateCcw size={15} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
