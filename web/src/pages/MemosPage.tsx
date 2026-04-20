import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { listMemos, createMemo, deleteMemo, promoteMemo } from '../api/memos';
import { toast } from 'sonner';
import ErrorState from '../components/ErrorState';
import EmptyState from '../components/EmptyState';
import { Button } from '../components/ui/button';

export default function MemosPage() {
  const [content, setContent] = useState('');
  const qc = useQueryClient();

  const { data: memos = [], isLoading, error, refetch } = useQuery({
    queryKey: ['memos'],
    queryFn: listMemos,
  });

  const create = useMutation({
    mutationFn: (content: string) => createMemo(content),
    onSuccess: () => { setContent(''); qc.invalidateQueries({ queryKey: ['memos'] }); },
    onError: () => toast.error('保存失败'),
  });

  const remove = useMutation({
    mutationFn: (id: string) => deleteMemo(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['memos'] }),
    onError: () => toast.error('删除失败'),
  });

  const promote = useMutation({
    mutationFn: (id: string) => promoteMemo(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['memos'] }),
    onError: () => toast.error('转化失败'),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!content.trim()) return;
    create.mutate(content.trim());
  };

  return (
    <div>
      <h2 className="text-xl font-semibold mb-4 text-foreground">收集箱</h2>

      <form onSubmit={handleSubmit} className="mb-6">
        <textarea
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder="快速记录 — 文字、URL 或想法..."
          className="w-full px-3 py-2 border border-border rounded resize-none h-20 bg-card text-foreground placeholder-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary"
          autoFocus
        />
        <Button
          type="submit"
          className="mt-2"
          disabled={create.isPending || !content.trim()}
        >
          {create.isPending ? '保存中...' : '保存'}
        </Button>
      </form>

      {isLoading ? (
        <div className="flex justify-center py-12">
          <div className="w-5 h-5 border-2 border-muted border-t-primary rounded-full animate-spin" />
        </div>
      ) : error ? (
        <ErrorState onRetry={() => refetch()} />
      ) : memos.length === 0 ? (
        <EmptyState icon="note" title="暂无收集" description="快速记录想法、URL 或灵感" />
      ) : (
        <div className="space-y-3">
          {memos.map((memo) => (
            <div key={memo.id} className="bg-card border border-border rounded-lg p-4">
              <p className="whitespace-pre-wrap text-foreground">{memo.content}</p>
              <div className="flex items-center gap-2 mt-3 text-sm">
                <span className="text-muted-foreground">
                  {new Date(memo.created_at).toLocaleDateString('zh-CN')}
                </span>
                {memo.promoted_entry_id ? (
                  <span className="text-green-600 dark:text-green-400">已转化</span>
                ) : (
                  <Button size="sm" variant="ghost" onClick={() => promote.mutate(memo.id)}>
                    转为文章
                  </Button>
                )}
                <Button size="sm" variant="ghost" className="text-destructive hover:text-destructive" onClick={() => remove.mutate(memo.id)}>
                  删除
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
