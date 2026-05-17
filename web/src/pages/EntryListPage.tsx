import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { Loader2, WifiOff, ArrowUp } from 'lucide-react';
import type { EntrySummary } from '../api/entries';
import { fetchTagStats } from '../api/tags';
import EntryCard from '../components/EntryCard';
import ErrorState from '../components/ErrorState';
import EmptyState from '../components/EmptyState';
import { useListKeyboardNav } from '../hooks/useKeyboardShortcuts';
import { useSwipe } from '../hooks/useSwipe';
import { useIsMobile } from '../hooks/use-mobile';
import { useEntryListFilters } from '../hooks/useEntryListFilters';
import { useEntrySelection } from '../hooks/useEntrySelection';
import { useInfiniteEntries } from '../hooks/useInfiniteEntries';
import { useQuery } from '@tanstack/react-query';
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent,
  AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { cn } from '@/lib/utils';

function useIntersectionObserver(cb: () => void, opts: { threshold?: number; rootMargin?: string } = {}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const obs = new IntersectionObserver(
      ([e]) => { if (e.isIntersecting) cb(); },
      { threshold: opts.threshold ?? 0, rootMargin: opts.rootMargin ?? '200px' },
    );
    obs.observe(el);
    return () => obs.disconnect();
  }, [cb, opts.threshold, opts.rootMargin]);
  return ref;
}

const TITLES: Record<string, string> = { unread: '未读', archived: '归档', starred: '收藏' };

export default function EntryListPage({ filter }: { filter?: 'unread' | 'archived' | 'starred' }) {
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [showScrollTop, setShowScrollTop] = useState(false);
  const isMobile = useIsMobile();
  const qc = useQueryClient();

  const { params, tagFilter, excludeTag, untagged, titleKey } = useEntryListFilters({ filter });
  const sel = useEntrySelection();

  const handleRefresh = async () => {
    setIsRefreshing(true);
    await qc.invalidateQueries({ queryKey: ['entries-infinite'] });
    setIsRefreshing(false);
  };
  const { isSwiping: isPulling, ref: refreshRef } = useSwipe(
    { onSwipeDown: handleRefresh }, { threshold: 60, direction: 'vertical' },
  );

  useEffect(() => {
    if (!isMobile || !refreshRef.current) return;
    const el = refreshRef.current;
    const check = () => { el.style.touchAction = window.scrollY === 0 ? 'manipulation' : 'auto'; };
    window.addEventListener('scroll', check, { passive: true });
    check();
    return () => window.removeEventListener('scroll', check);
  }, [isMobile]);

  useEffect(() => {
    const h = () => setShowScrollTop(window.scrollY > 400);
    window.addEventListener('scroll', h, { passive: true });
    return () => window.removeEventListener('scroll', h);
  }, []);

  const { data, isLoading, error, refetch, fetchNextPage, hasNextPage, isFetchingNextPage } = useInfiniteEntries(params);
  useQuery({ queryKey: ['tags', 'stats'], queryFn: fetchTagStats, staleTime: 5 * 60_000 });
  const entries: EntrySummary[] = data?.pages.flatMap((p) => p.entries) ?? [];
  const entryIds = useMemo(() => entries.map((e) => e.id), [entries]);

  const loadMore = useCallback(() => { if (hasNextPage && !isFetchingNextPage) fetchNextPage(); }, [hasNextPage, isFetchingNextPage, fetchNextPage]);
  const sentinelRef = useIntersectionObserver(loadMore);
  useListKeyboardNav(entries, selectedIndex, setSelectedIndex);

  let title = TITLES[titleKey] || titleKey;
  if (tagFilter) title = tagFilter;
  else if (untagged) title = '未标签文章';
  else if (excludeTag) title = `排除「${excludeTag}」`;

  return (
    <div ref={isMobile ? refreshRef : undefined} className="animate-fade-in">
      {(isPulling || isRefreshing) && (
        <div className="flex items-center justify-center py-3 text-sm text-muted-foreground">
          <Loader2 size={16} className={cn('mr-2', isRefreshing && 'animate-spin')} />
          {isRefreshing ? '刷新中...' : '下拉刷新'}
        </div>
      )}
      {!navigator.onLine && (
        <div className="flex items-center gap-1.5 text-xs text-muted-foreground/80 mb-4 px-1">
          <WifiOff size={12} /> <span>离线模式 — 显示已缓存的内容</span>
        </div>
      )}

      {sel.selectionMode && sel.selectedIds.size > 0 && (
        <div className="fixed left-0 right-0 bottom-0 z-50 bg-background/95 backdrop-blur-xl border-t border-border/60 shadow-[0_-4px_20px_rgba(0,0,0,0.08)] pb-[env(safe-area-inset-bottom)] animate-slide-in-right">
          <div className="max-w-4xl mx-auto px-3 sm:px-4 py-2.5 flex items-center gap-2">
            <span className="text-sm font-semibold shrink-0 tabular-nums">{sel.selectedIds.size}</span>
            <div className="flex-1" />
            <button onClick={() => sel.bulkArchiveMutation.mutate(Array.from(sel.selectedIds))} className="p-1.5 rounded-lg hover:bg-muted text-muted-foreground">归档</button>
            <button onClick={() => sel.setDeleteConfirmOpen(true)} className="p-1.5 rounded-lg hover:bg-muted text-destructive">删除</button>
            <button onClick={sel.clearSelection} className="p-1.5 rounded-lg hover:bg-muted text-muted-foreground">✕</button>
          </div>
        </div>
      )}

      {isLoading ? (
        <div className="space-y-3">
          {[1, 2, 3].map((i) => (
            <div key={i} className="bg-card border border-border/50 rounded-xl p-5 animate-pulse">
              <div className="h-5 bg-muted rounded w-3/4 mb-3" /><div className="h-4 bg-muted rounded w-1/2 mb-4" /><div className="h-4 bg-muted rounded w-24" />
            </div>
          ))}
        </div>
      ) : error ? (
        <ErrorState onRetry={() => refetch()} />
      ) : entries.length === 0 ? (
        <EmptyState icon="book" title={`暂无${title}文章`} description={!filter || filter === 'unread' ? '粘贴 URL 保存你的第一篇文章' : undefined} />
      ) : (
        <div className="space-y-3 stagger-children">
          {entries.map((entry, i) => (
            <EntryCard key={entry.id} entry={entry} selected={i === selectedIndex || sel.selectedIds.has(entry.id)}
              selectionMode={sel.selectionMode} entrySelected={sel.selectedIds.has(entry.id)}
              onToggleSelect={() => sel.toggleSelect(entry.id)} entryIndex={i} entryIds={entryIds} />
          ))}
          <div ref={sentinelRef} className="h-4" />
          {isFetchingNextPage && <div className="flex justify-center py-5"><Loader2 className="h-5 w-5 animate-spin text-muted-foreground/50" /></div>}
          {!hasNextPage && entries.length > 0 && <div className="text-center text-muted-foreground/50 text-sm py-6">已加载全部文章</div>}
        </div>
      )}

      {showScrollTop && (
        <button onClick={() => window.scrollTo({ top: 0, behavior: 'smooth' })}
          className="fixed right-5 bottom-6 z-40 h-10 w-10 rounded-full bg-card border border-border/60 shadow-lg flex items-center justify-center text-muted-foreground hover:text-foreground hover:border-border transition-all animate-fade-in">
          <ArrowUp size={18} />
        </button>
      )}

      <AlertDialog open={sel.deleteConfirmOpen} onOpenChange={sel.setDeleteConfirmOpen}>
        <AlertDialogContent className="rounded-2xl">
          <AlertDialogHeader>
            <AlertDialogTitle>确认删除</AlertDialogTitle>
            <AlertDialogDescription>确定要删除选中的 {sel.selectedIds.size} 篇文章吗？此操作不可撤销。</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel className="rounded-lg">取消</AlertDialogCancel>
            <AlertDialogAction variant="destructive" className="rounded-lg"
              onClick={() => { sel.bulkDeleteMutation.mutate(Array.from(sel.selectedIds)); sel.setDeleteConfirmOpen(false); }}>删除</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}