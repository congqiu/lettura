import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { listMemos, createMemo, deleteMemo, promoteMemo } from '../api/memos';
import { toast } from 'sonner';
import ErrorState from '../components/ErrorState';
import EmptyState from '../components/EmptyState';
import { Button } from '../components/ui/button';
import { Loader2, StickyNote, Trash2, ArrowUpRight } from 'lucide-react';
import { cn } from '@/lib/utils';

export default function MemosPage() {
  const [content, setContent] = useState('');
  const [focused, setFocused] = useState(false);
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
    <div className="animate-fade-in">
      <div className="flex items-center gap-2.5 mb-5">
        <div className="w-9 h-9 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
          <StickyNote size={18} />
        </div>
        <div>
          <h2 className="text-xl font-bold tracking-tight text-foreground">便签</h2>
          <p className="text-xs text-muted-foreground">快速记录想法、链接和灵感</p>
        </div>
      </div>

      <form onSubmit={handleSubmit} className="mb-6">
        <div
          className={cn(
            'rounded-xl border bg-card transition-all duration-200 overflow-hidden',
            focused ? 'border-primary/40 shadow-sm shadow-primary/5 ring-2 ring-primary/10' : 'border-border/60'
          )}
        >
          <textarea
            value={content}
            onChange={(e) => setContent(e.target.value)}
            onFocus={() => setFocused(true)}
            onBlur={() => setFocused(false)}
            placeholder="记录此刻的想法、URL 或灵感..."
            className="w-full px-4 py-3 bg-transparent text-foreground placeholder:text-muted-foreground/40 focus:outline-none resize-none min-h-[100px] text-[15px] leading-relaxed"
            autoFocus
          />
          <div className="flex items-center justify-between px-3 py-2 border-t border-border/40 bg-muted/20">
            <span className="text-xs text-muted-foreground/60">
              {content.length > 0 ? `${content.length} 字` : '支持 Markdown'}
            </span>
            <Button
              type="submit"
              size="sm"
              disabled={create.isPending || !content.trim()}
              className="rounded-lg h-8"
            >
              {create.isPending ? (
                <Loader2 size={14} className="animate-spin" />
              ) : (
                '保存'
              )}
            </Button>
          </div>
        </div>
      </form>

      {isLoading ? (
        <div className="space-y-3">
          {[1, 2, 3].map((i) => (
            <div key={i} className="bg-card border border-border/50 rounded-xl p-5 animate-pulse">
              <div className="h-4 bg-muted rounded w-full mb-2" />
              <div className="h-4 bg-muted rounded w-4/5" />
            </div>
          ))}
        </div>
      ) : error ? (
        <ErrorState onRetry={() => refetch()} />
      ) : memos.length === 0 ? (
        <EmptyState icon="note" title="暂无收集" description="快速记录想法、URL 或灵感" />
      ) : (
        <div className="space-y-3 stagger-children">
          {memos.map((memo) => (
            <div
              key={memo.id}
              className="bg-card border border-border/60 rounded-xl p-5 hover:border-border transition-colors"
            >
              <p className="whitespace-pre-wrap text-foreground text-[15px] leading-relaxed">{memo.content}</p>
              <div className="flex items-center gap-3 mt-4 pt-3 border-t border-border/40">
                <span className="text-[12px] text-muted-foreground">
                  {new Date(memo.created_at).toLocaleDateString('zh-CN', {
                    month: 'short',
                    day: 'numeric',
                    hour: '2-digit',
                    minute: '2-digit',
                  })}
                </span>
                <div className="flex-1" />
                {memo.promoted_entry_id ? (
                  <span className="inline-flex items-center gap-1 text-[12px] font-medium text-success">
                    <ArrowUpRight size={12} />
                    已转为文章
                  </span>
                ) : (
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => promote.mutate(memo.id)}
                    className="h-7 text-[12px] px-2 rounded-lg text-muted-foreground hover:text-foreground"
                  >
                    转为文章
                  </Button>
                )}
                <Button
                  size="sm"
                  variant="ghost"
                  className="h-7 w-7 p-0 rounded-lg text-muted-foreground hover:text-destructive hover:bg-destructive/5"
                  onClick={() => remove.mutate(memo.id)}
                >
                  <Trash2 size={14} />
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
