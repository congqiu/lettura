import { useState, useEffect, useRef, useCallback } from 'react';
import { useInfiniteEntries } from '../hooks/useInfiniteEntries';
import { Search, Loader2 } from 'lucide-react';
import type { EntrySummary, ListParams } from '../api/entries';
import EntryCard from '../components/EntryCard';
import AddEntryForm from '../components/AddEntryForm';
import ErrorState from '../components/ErrorState';
import EmptyState from '../components/EmptyState';
import { useListKeyboardNav } from '../hooks/useKeyboardShortcuts';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';

interface Props {
  filter?: 'unread' | 'archived' | 'starred';
}

const TITLES = { unread: '未读', archived: '归档', starred: '收藏' };

// Intersection Observer hook for infinite scroll
function useIntersectionObserver(
  callback: () => void,
  options: { threshold?: number; rootMargin?: string } = {}
) {
  const targetRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const target = targetRef.current;
    if (!target) return;

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          callback();
        }
      },
      {
        threshold: options.threshold ?? 0,
        rootMargin: options.rootMargin ?? '200px',
      }
    );

    observer.observe(target);
    return () => observer.disconnect();
  }, [callback, options.threshold, options.rootMargin]);

  return targetRef;
}

export default function EntryListPage({ filter }: Props) {
  const [search, setSearch] = useState('');
  const [domain, setDomain] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);

  const params: Omit<ListParams, 'cursor'> = {};
  if (filter === 'archived') params.is_archived = true;
  if (filter === 'starred') params.is_starred = true;
  if (filter === 'unread') params.is_archived = false;
  if (search) params.search = search;
  if (domain) params.domain = domain;

  const {
    data,
    isLoading,
    error,
    refetch,
    fetchNextPage,
    hasNextPage,
    isFetchingNextPage,
  } = useInfiniteEntries(params);

  // Flatten pages into a single array
  const entries: EntrySummary[] = data?.pages.flatMap((page) => page.entries) ?? [];

  // Callback for intersection observer
  const loadMore = useCallback(() => {
    if (hasNextPage && !isFetchingNextPage) {
      fetchNextPage();
    }
  }, [hasNextPage, isFetchingNextPage, fetchNextPage]);

  // Sentinel element for infinite scroll
  const sentinelRef = useIntersectionObserver(loadMore);

  useListKeyboardNav(entries, selectedIndex, setSelectedIndex);

  const title = TITLES[filter || 'unread'];

  return (
    <div>
      <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between mb-6 gap-4">
        <div className="flex items-center gap-3">
          <h2 className="text-2xl font-bold tracking-tight">{title}</h2>
          {domain && (
            <Badge variant="secondary" className="flex items-center gap-1.5">
              {domain}
              <button onClick={() => setDomain('')} className="hover:text-destructive font-bold transition-colors">&times;</button>
            </Badge>
          )}
        </div>
        <div className="flex items-center gap-3 w-full sm:w-auto">
          <div className="relative w-full sm:w-64">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
            <Input
              type="text"
              placeholder="搜索..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="pl-9 bg-card"
            />
          </div>
        </div>
      </div>

      {!filter || filter === 'unread' ? <AddEntryForm /> : null}

      {isLoading ? (
        <div className="bg-card border border-border rounded-xl p-12 flex justify-center">
          <div className="w-5 h-5 border-2 border-muted border-t-primary rounded-full animate-spin" />
        </div>
      ) : error ? (
        <ErrorState onRetry={() => refetch()} />
      ) : entries.length === 0 ? (
        <EmptyState icon="book" title="暂无文章" description="粘贴 URL 保存你的第一篇文章" />
      ) : (
        <div className="space-y-4">
          {entries.map((entry, i) => (
            <EntryCard
              key={entry.id}
              entry={entry}
              selected={i === selectedIndex}
              onDomainClick={(d) => setDomain(d)}
            />
          ))}

          {/* Sentinel for infinite scroll */}
          <div ref={sentinelRef} className="h-4" />

          {/* Loading indicator */}
          {isFetchingNextPage && (
            <div className="flex justify-center py-4">
              <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
            </div>
          )}

          {/* End of list indicator */}
          {!hasNextPage && entries.length > 0 && (
            <div className="text-center text-muted-foreground text-sm py-4">
              已加载全部文章
            </div>
          )}
        </div>
      )}
    </div>
  );
}
