import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useInfiniteEntries } from '../hooks/useInfiniteEntries';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Search, Loader2, Tag, Tags, Archive, Trash2, X, BookOpen, Star, ArrowUp, WifiOff } from 'lucide-react';
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
import { cn } from '@/lib/utils';

interface Props {
  filter?: 'unread' | 'archived' | 'starred';
}

const TITLES = { unread: '未读', archived: '归档', starred: '收藏' };
const TITLE_ICONS = { unread: BookOpen, archived: Archive, starred: Star };

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
  const [filterOpen, setFilterOpen] = useState(false);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const [domain, setDomain] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [searchParams] = useSearchParams();
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
  const [showScrollTop, setShowScrollTop] = useState(false);

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
      el.style.touchAction = window.scrollY === 0 ? 'manipulation' : 'auto';
    };
    window.addEventListener('scroll', checkScrollTop, { passive: true });
    checkScrollTop();
    return () => window.removeEventListener('scroll', checkScrollTop);
  }, [isMobile]);

  // Show scroll-to-top button when scrolled down
  useEffect(() => {
    const handleScroll = () => setShowScrollTop(window.scrollY > 400);
    window.addEventListener('scroll', handleScroll, { passive: true });
    return () => window.removeEventListener('scroll', handleScroll);
  }, []);

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

  let title = TITLES[filter || 'unread'];
  let TitleIcon = TITLE_ICONS[filter || 'unread'];

  if (tagFilter) {
    title = tagFilter;
    TitleIcon = Tag;
  } else if (untagged) {
    title = '未标签文章';
    TitleIcon = Tags;
  } else if (excludeTag) {
    title = `排除「${excludeTag}」`;
    TitleIcon = Tag;
  }

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
    <div ref={isMobile ? refreshRef : undefined} className="animate-fade-in">
      {/* Pull to refresh indicator */}
      {(isPulling || isRefreshing) && (
        <div className="flex items-center justify-center py-3 text-sm text-muted-foreground">
          <Loader2 size={16} className={cn('mr-2', isRefreshing && 'animate-spin')} />
          {isRefreshing ? '刷新中...' : '下拉刷新'}
        </div>
      )}

      {/* Header */}
      <div className="flex items-center justify-between mb-5">
        <div className="flex items-center gap-2.5">
          <div className={cn(
            'w-9 h-9 rounded-xl flex items-center justify-center',
            filter === 'starred' && 'bg-amber-500/10 text-amber-500',
            filter === 'archived' && 'bg-success/10 text-success',
            (!filter || filter === 'unread') && 'bg-primary/10 text-primary',
          )}>
            <TitleIcon size={18} />
          </div>
          <div>
            <h2 className="text-xl font-bold tracking-tight text-foreground">{title}</h2>
            {entries.length > 0 && !isLoading && (
              <p className="text-xs text-muted-foreground">{entries.length} 篇文章</p>
            )}
          </div>
        </div>

        <div className="flex items-center gap-1.5">
          <Button
            variant={filterOpen || search || domain ? 'default' : 'ghost'}
            size="sm"
            onClick={() => setFilterOpen(!filterOpen)}
            className="h-8 w-8 p-0 rounded-lg"
          >
            <Search size={15} />
          </Button>
          <Button
            variant={selectionMode ? 'default' : 'ghost'}
            size="sm"
            onClick={() => {
              if (selectionMode) {
                clearSelection();
              } else {
                setSelectionMode(true);
              }
            }}
            className="h-8 px-3 rounded-lg text-[13px]"
          >
            {selectionMode ? '取消' : '多选'}
          </Button>
        </div>
      </div>

      {/* Offline indicator */}
      {!navigator.onLine && (
        <div className="flex items-center gap-1.5 text-xs text-muted-foreground/80 mb-4 px-1">
          <WifiOff size={12} />
          <span>离线模式 — 显示已缓存的内容</span>
        </div>
      )}

      {/* Filter panel */}
      {(filterOpen || search || domain) && (
        <div className="mb-4 p-3 bg-card border border-border/50 rounded-xl animate-fade-in-down space-y-2.5">
          <div className="relative">
            <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground/40" />
            <Input
              ref={searchInputRef}
              type="text"
              placeholder="搜索文章标题、内容..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="pl-8 pr-8 h-9 text-sm bg-muted/30 border-border/40 rounded-lg focus-visible:ring-primary/20"
            />
            {search && (
              <button
                onClick={() => {
                  setSearch('');
                  searchInputRef.current?.focus();
                }}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground/40 hover:text-foreground transition-colors"
              >
                <X size={12} />
              </button>
            )}
          </div>
          {domain && (
            <div className="flex items-center gap-2 flex-wrap">
              <TagBadge label={domain} onRemove={() => setDomain('')} />
            </div>
          )}
        </div>
      )}

      {/* Active domain filter */}
      {domain && (
        <div className="flex items-center gap-2 mb-4 flex-wrap">
          <TagBadge label={domain} onRemove={() => setDomain('')} />
        </div>
      )}

      {/* Add entry form */}
      {!filter || filter === 'unread' ? <AddEntryForm /> : null}

      {/* Content states */}
      {isLoading ? (
        <div className="space-y-3">
          {[1, 2, 3].map((i) => (
            <div key={i} className="bg-card border border-border/50 rounded-xl p-5 animate-pulse">
              <div className="h-5 bg-muted rounded w-3/4 mb-3" />
              <div className="h-4 bg-muted rounded w-1/2 mb-4" />
              <div className="h-4 bg-muted rounded w-24" />
            </div>
          ))}
        </div>
      ) : error ? (
        <ErrorState onRetry={() => refetch()} />
      ) : entries.length === 0 ? (
        <EmptyState
          icon="book"
          title={`暂无${title}文章`}
          description={!filter || filter === 'unread' ? '粘贴 URL 保存你的第一篇文章' : undefined}
        />
      ) : (
        <div className="space-y-3 stagger-children">
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
            <div className="flex justify-center py-5">
              <Loader2 className="h-5 w-5 animate-spin text-muted-foreground/50" />
            </div>
          )}

          {/* End of list indicator */}
          {!hasNextPage && entries.length > 0 && (
            <div className="text-center text-muted-foreground/50 text-sm py-6">
              已加载全部文章
            </div>
          )}
        </div>
      )}

      {/* Bulk action bar */}
      {selectionMode && selectedIds.size > 0 && (
        <div className="fixed left-0 right-0 bottom-0 z-50 bg-background/95 backdrop-blur-xl border-t border-border/60 shadow-[0_-4px_20px_rgba(0,0,0,0.08)] pb-[env(safe-area-inset-bottom)] animate-slide-in-right">
          <div className="max-w-4xl mx-auto px-3 sm:px-4 py-2.5 flex items-center gap-2">
            <span className="text-sm font-semibold shrink-0 tabular-nums">
              {selectedIds.size}
            </span>

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
                  placeholder="添加标签..."
                  className="h-8 w-28 sm:w-36 text-sm rounded-lg"
                />
                {showBulkTagSuggest && bulkTagInput && tagSuggestions.length > 0 && (
                  <div className="absolute bottom-full mb-1 left-0 w-48 z-50">
                    <Command className="border border-border/60 shadow-lg rounded-xl">
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
              <Button size="icon" variant="outline" onClick={() => handleBulkTag()} disabled={!bulkTagInput.trim()} className="h-8 w-8 rounded-lg">
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
                  placeholder="移除标签..."
                  className="h-8 w-28 sm:w-36 text-sm rounded-lg"
                />
                {showBulkUntagSuggest && bulkUntagInput && untagSuggestions.length > 0 && (
                  <div className="absolute bottom-full mb-1 left-0 w-48 z-50">
                    <Command className="border border-border/60 shadow-lg rounded-xl">
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
              <Button size="icon" variant="outline" onClick={() => handleBulkUntag()} disabled={!bulkUntagInput.trim()} className="h-8 w-8 rounded-lg">
                <Tags size={14} />
              </Button>
            </div>

            <div className="flex-1" />

            <Button size="icon" variant="outline" onClick={() => bulkArchiveMutation.mutate(Array.from(selectedIds))} className="h-8 w-8 rounded-lg">
              <Archive size={15} />
            </Button>

            <Button size="icon" variant="destructive" onClick={() => setDeleteConfirmOpen(true)} className="h-8 w-8 rounded-lg">
              <Trash2 size={15} />
            </Button>

            <Button size="icon" variant="ghost" onClick={clearSelection} className="h-8 w-8 rounded-lg">
              <X size={15} />
            </Button>
          </div>
        </div>
      )}

      {/* Scroll to top */}
      {showScrollTop && (
        <button
          onClick={() => window.scrollTo({ top: 0, behavior: 'smooth' })}
          className="fixed right-5 bottom-6 z-40 h-10 w-10 rounded-full bg-card border border-border/60 shadow-lg flex items-center justify-center text-muted-foreground hover:text-foreground hover:border-border transition-all animate-fade-in"
        >
          <ArrowUp size={18} />
        </button>
      )}

      {/* Delete confirmation dialog */}
      <AlertDialog open={deleteConfirmOpen} onOpenChange={setDeleteConfirmOpen}>
        <AlertDialogContent className="rounded-2xl">
          <AlertDialogHeader>
            <AlertDialogTitle>确认删除</AlertDialogTitle>
            <AlertDialogDescription>
              确定要删除选中的 {selectedIds.size} 篇文章吗？此操作不可撤销。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel className="rounded-lg">取消</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              className="rounded-lg"
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
