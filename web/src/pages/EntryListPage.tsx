import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useInfiniteEntries } from '../hooks/useInfiniteEntries';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Search, Loader2, Tag, Tags, Archive, Trash2, X } from 'lucide-react';
import type { EntrySummary, ListParams } from '../api/entries';
import { bulkTagByIds, bulkUntagByIds, bulkDeleteByIds, bulkArchiveByIds, fetchTagStats } from '../api/tags';
import EntryCard from '../components/EntryCard';
import AddEntryForm from '../components/AddEntryForm';
import ErrorState from '../components/ErrorState';
import EmptyState from '../components/EmptyState';
import TagBadge from '../components/TagBadge';
import { useListKeyboardNav } from '../hooks/useKeyboardShortcuts';
import { useSwipe } from '../hooks/useSwipe';
import { useIsMobile } from '../hooks/use-mobile';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { useQuery } from '@tanstack/react-query';
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandItem,
  CommandList,
} from '@/components/ui/command';
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent,
  AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { toast } from 'sonner';

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
  const [searchParams, setSearchParams] = useSearchParams();
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [bulkTagInput, setBulkTagInput] = useState('');
  const [showBulkTagSuggest, setShowBulkTagSuggest] = useState(false);
  const [bulkUntagInput, setBulkUntagInput] = useState('');
  const [showBulkUntagSuggest, setShowBulkUntagSuggest] = useState(false);
  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false);
  const qc = useQueryClient();

  const isMobile = useIsMobile();
  const [isRefreshing, setIsRefreshing] = useState(false);

  const handleRefresh = async () => {
    setIsRefreshing(true);
    await qc.invalidateQueries({ queryKey: ['entries-infinite'] });
    setIsRefreshing(false);
  };

  const { isSwiping: isPulling, ref: refreshRef } = useSwipe(
    { onSwipeDown: handleRefresh },
    { threshold: 60, direction: 'vertical' },
  );

  // Only enable pull-to-refresh when scrolled to top
  useEffect(() => {
    if (!isMobile || !refreshRef.current) return;
    const el = refreshRef.current;
    const checkScrollTop = () => {
      // Use 'manipulation' to allow vertical scroll while preventing double-tap zoom.
      // The useSwipe hook handles vertical gesture detection via touch events.
      el.style.touchAction = window.scrollY === 0 ? 'manipulation' : 'auto';
    };
    window.addEventListener('scroll', checkScrollTop, { passive: true });
    checkScrollTop();
    return () => window.removeEventListener('scroll', checkScrollTop);
  }, [isMobile]);

  const tagFilter = searchParams.get('tag') || '';
  const excludeTag = searchParams.get('exclude_tag') || '';
  const untagged = searchParams.get('untagged') === 'true';

  const params: Omit<ListParams, 'cursor'> = {};
  if (filter === 'archived') params.is_archived = true;
  if (filter === 'starred') params.is_starred = true;
  if (filter === 'unread') params.is_archived = false;
  if (search) params.search = search;
  if (domain) params.domain = domain;
  if (tagFilter) params.tag = tagFilter;
  if (excludeTag) params.exclude_tag = excludeTag;
  if (untagged) params.untagged = true;

  const {
    data,
    isLoading,
    error,
    refetch,
    fetchNextPage,
    hasNextPage,
    isFetchingNextPage,
  } = useInfiniteEntries(params);

  // Fetch tag stats for autocomplete
  const { data: tagStats = [] } = useQuery({
    queryKey: ['tags', 'stats'],
    queryFn: fetchTagStats,
    staleTime: 5 * 60 * 1000,
  });

  // Flatten pages into a single array
  const entries: EntrySummary[] = data?.pages.flatMap((page) => page.entries) ?? [];
  const entryIds = useMemo(() => entries.map(e => e.id), [entries]);

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

  const toggleSelect = (id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const clearSelection = () => {
    setSelectedIds(new Set());
    setSelectionMode(false);
  };

  const afterBulkOp = () => {
    clearSelection();
    qc.invalidateQueries({ queryKey: ['entries-infinite'] });
    qc.invalidateQueries({ queryKey: ['tags', 'stats'] });
  };

  const bulkTagMutation = useMutation({
    mutationFn: ({ ids, tags }: { ids: string[]; tags: string[] }) => bulkTagByIds(ids, tags),
    onSuccess: () => {
      toast.success(`已为 ${selectedIds.size} 篇文章添加标签`);
      afterBulkOp();
    },
    onError: () => toast.error('批量打标签失败'),
  });

  const bulkUntagMutation = useMutation({
    mutationFn: ({ ids, tags }: { ids: string[]; tags: string[] }) => bulkUntagByIds(ids, tags),
    onSuccess: () => {
      toast.success(`已从 ${selectedIds.size} 篇文章移除标签`);
      afterBulkOp();
    },
    onError: () => toast.error('批量移除标签失败'),
  });

  const bulkArchiveMutation = useMutation({
    mutationFn: (ids: string[]) => bulkArchiveByIds(ids),
    onSuccess: () => {
      toast.success(`已归档 ${selectedIds.size} 篇文章`);
      afterBulkOp();
    },
    onError: () => toast.error('批量归档失败'),
  });

  const bulkDeleteMutation = useMutation({
    mutationFn: (ids: string[]) => bulkDeleteByIds(ids),
    onSuccess: () => {
      toast.success(`已删除 ${selectedIds.size} 篇文章`);
      afterBulkOp();
    },
    onError: () => toast.error('批量删除失败'),
  });

  // Autocomplete suggestions for bulk tag
  const tagSuggestions = tagStats
    .filter((t) => t.label.toLowerCase().includes(bulkTagInput.toLowerCase()))
    .slice(0, 10);

  const untagSuggestions = tagStats
    .filter((t) => t.label.toLowerCase().includes(bulkUntagInput.toLowerCase()))
    .slice(0, 10);

  const handleBulkTag = (label?: string) => {
    const tag = label || bulkTagInput.trim();
    if (!tag || selectedIds.size === 0) return;
    bulkTagMutation.mutate({ ids: Array.from(selectedIds), tags: [tag] });
    setBulkTagInput('');
    setShowBulkTagSuggest(false);
  };

  const handleBulkUntag = (label?: string) => {
    const tag = label || bulkUntagInput.trim();
    if (!tag || selectedIds.size === 0) return;
    bulkUntagMutation.mutate({ ids: Array.from(selectedIds), tags: [tag] });
    setBulkUntagInput('');
    setShowBulkUntagSuggest(false);
  };

  return (
    <div ref={isMobile ? refreshRef : undefined}>
      {(isPulling || isRefreshing) && (
        <div className="flex items-center justify-center py-3 text-sm text-muted-foreground">
          <Loader2 size={16} className={`mr-2 ${isRefreshing ? 'animate-spin' : ''}`} />
          {isRefreshing ? '刷新中...' : '下拉刷新'}
        </div>
      )}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-3">
          <h2 className="text-2xl font-bold tracking-tight">{title}</h2>
          {domain && (
            <TagBadge
              label={domain}
              onRemove={() => setDomain('')}
            />
          )}
          {tagFilter && (
            <TagBadge
              label={tagFilter}
              onRemove={() => {
                searchParams.delete('tag');
                setSearchParams(searchParams);
              }}
            />
          )}
          {untagged && (
            <TagBadge
              label="未标签"
              clickable={false}
              onRemove={() => {
                searchParams.delete('untagged');
                setSearchParams(searchParams);
              }}
            />
          )}
        </div>
        <Button
          variant={selectionMode ? 'default' : 'outline'}
          size="sm"
          onClick={() => {
            if (selectionMode) {
              clearSelection();
            } else {
              setSelectionMode(true);
            }
          }}
        >
          {selectionMode ? '取消多选' : '多选'}
        </Button>
      </div>
      <div className="relative mb-6 sm:w-64">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
        <Input
          type="text"
          placeholder="搜索..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="pl-9 bg-card"
        />
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
            <div key={entry.id}>
              <EntryCard
                entry={entry}
                selected={i === selectedIndex || selectedIds.has(entry.id)}
                onDomainClick={(d) => setDomain(d)}
                selectionMode={selectionMode}
                entrySelected={selectedIds.has(entry.id)}
                onToggleSelect={() => toggleSelect(entry.id)}
                entryIndex={i}
                entryIds={entryIds}
              />
            </div>
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

      {/* Bulk action bar */}
      {selectionMode && selectedIds.size > 0 && (
        <div className="fixed left-0 right-0 z-50 bg-background border-t border-border shadow-lg pb-[env(safe-area-inset-bottom)]" style={{ bottom: 'var(--bottom-nav-height, 0px)' }}>
          <div className="max-w-4xl mx-auto px-4 py-3 flex items-center gap-3 flex-wrap">
            <span className="text-sm font-medium">已选 {selectedIds.size} 篇</span>

            {/* Tag input */}
            <div className="relative flex items-center gap-1">
              <div className="relative">
                <Input
                  value={bulkTagInput}
                  onChange={(e) => {
                    setBulkTagInput(e.target.value);
                    setShowBulkTagSuggest(true);
                  }}
                  onFocus={() => setShowBulkTagSuggest(true)}
                  placeholder="打标签..."
                  className="h-8 w-28 text-sm"
                />
                {showBulkTagSuggest && bulkTagInput && tagSuggestions.length > 0 && (
                  <div className="absolute bottom-full mb-1 left-0 w-48 z-50">
                    <Command className="border border-border shadow-md">
                      <CommandList>
                        <CommandEmpty>无匹配</CommandEmpty>
                        <CommandGroup>
                          {tagSuggestions.map((tag) => (
                            <CommandItem
                              key={tag.id}
                              value={tag.label}
                              onSelect={() => handleBulkTag(tag.label)}
                            >
                              {tag.label}
                            </CommandItem>
                          ))}
                        </CommandGroup>
                      </CommandList>
                    </Command>
                  </div>
                )}
              </div>
              <Button size="sm" variant="outline" onClick={() => handleBulkTag()} disabled={!bulkTagInput.trim()}>
                <Tag size={14} />
              </Button>
            </div>

            {/* Untag input */}
            <div className="relative flex items-center gap-1">
              <div className="relative">
                <Input
                  value={bulkUntagInput}
                  onChange={(e) => {
                    setBulkUntagInput(e.target.value);
                    setShowBulkUntagSuggest(true);
                  }}
                  onFocus={() => setShowBulkUntagSuggest(true)}
                  placeholder="取消标签..."
                  className="h-8 w-28 text-sm"
                />
                {showBulkUntagSuggest && bulkUntagInput && untagSuggestions.length > 0 && (
                  <div className="absolute bottom-full mb-1 left-0 w-48 z-50">
                    <Command className="border border-border shadow-md">
                      <CommandList>
                        <CommandEmpty>无匹配</CommandEmpty>
                        <CommandGroup>
                          {untagSuggestions.map((tag) => (
                            <CommandItem
                              key={tag.id}
                              value={tag.label}
                              onSelect={() => handleBulkUntag(tag.label)}
                            >
                              {tag.label}
                            </CommandItem>
                          ))}
                        </CommandGroup>
                      </CommandList>
                    </Command>
                  </div>
                )}
              </div>
              <Button size="sm" variant="outline" onClick={() => handleBulkUntag()} disabled={!bulkUntagInput.trim()}>
                <Tags size={14} />
              </Button>
            </div>

            <Button size="sm" variant="outline" onClick={() => bulkArchiveMutation.mutate(Array.from(selectedIds))}>
              <Archive size={14} className="mr-1" /> 归档
            </Button>

            <Button size="sm" variant="destructive" onClick={() => setDeleteConfirmOpen(true)}>
              <Trash2 size={14} className="mr-1" /> 删除
            </Button>

            <Button size="sm" variant="ghost" onClick={clearSelection} className="ml-auto">
              <X size={14} />
            </Button>
          </div>
        </div>
      )}

      {/* Delete confirmation dialog */}
      <AlertDialog open={deleteConfirmOpen} onOpenChange={setDeleteConfirmOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>确认删除</AlertDialogTitle>
            <AlertDialogDescription>
              确定要删除选中的 {selectedIds.size} 篇文章吗？此操作不可撤销。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={() => {
                bulkDeleteMutation.mutate(Array.from(selectedIds));
                setDeleteConfirmOpen(false);
              }}
            >
              删除
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
