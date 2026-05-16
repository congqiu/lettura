import { useState, useEffect, useRef } from 'react';
import { useParams, useNavigate, useLocation } from 'react-router-dom';
import DOMPurify from 'dompurify';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getEntry, updateEntry, deleteEntry, refetchEntry } from '../api/entries';
import ContentEditor from '../components/ContentEditor';
import AnnotationsSidebar from '../components/AnnotationsSidebar';
import ErrorState from '../components/ErrorState';
import EntryTags from '../components/EntryTags';
import { useEntryActions } from '../hooks/useEntryActions';
import { getErrorMessage } from '../utils/error';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';
import { Skeleton } from '@/components/ui/skeleton';
import { useIsMobile } from '../hooks/use-mobile';
import { useSwipe } from '../hooks/useSwipe';
import { Sheet, SheetContent } from '@/components/ui/sheet';
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { ArrowLeft, Star, Archive, RefreshCw, Edit3, MessageSquare, Trash2, MoreHorizontal, ExternalLink, Clock, Globe, WifiOff } from 'lucide-react';
import { cn } from '@/lib/utils';

export default function EntryDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const qc = useQueryClient();
  const isMobile = useIsMobile();
  const location = useLocation();
  const listContext = location.state as { entryIds?: string[]; currentIndex?: number } | null;

  const navigateToEntry = (direction: 'prev' | 'next') => {
    if (!listContext?.entryIds || listContext.currentIndex === undefined) return;
    const newIndex = direction === 'prev' ? listContext.currentIndex - 1 : listContext.currentIndex + 1;
    if (newIndex < 0 || newIndex >= listContext.entryIds.length) return;
    const newId = listContext.entryIds[newIndex];
    navigate(`/entry/${newId}`, {
      state: { entryIds: listContext.entryIds, currentIndex: newIndex },
      replace: true,
    });
  };

  // Swipe handler
  const { swipeOffset, isSwiping: isGestureActive, ref: gestureRef } = useSwipe(
    {
      onSwipeRight: () => navigateToEntry('prev'),
      onSwipeLeft: () => navigateToEntry('next'),
      onEdgeSwipeRight: () => navigate(-1),
    },
    { threshold: 100, direction: 'horizontal', edgeStart: 30 },
  );

  const [editing, setEditing] = useState(false);
  const [showAnnotations, setShowAnnotations] = useState(false);
  const [editingTitle, setEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState('');
  const articleRef = useRef<HTMLElement>(null);
  const [pendingQuote, setPendingQuote] = useState('');
  const [annotationKey, setAnnotationKey] = useState(0);
  const [selectionToolbar, setSelectionToolbar] = useState<{
    visible: boolean;
    text: string;
    x: number;
    y: number;
  }>({ visible: false, text: '', x: 0, y: 0 });

  const { data: entry, isLoading, error, refetch: refetchEntryQuery } = useQuery({
    queryKey: ['entry', id],
    queryFn: () => getEntry(id!),
    enabled: !!id,
  });

  // Inject copy buttons into code blocks
  useEffect(() => {
    const article = articleRef.current;
    if (!article) return;
    const pres = article.querySelectorAll('pre');
    const cleanups: (() => void)[] = [];

    pres.forEach((pre) => {
      if (pre.parentElement?.classList.contains('code-block-wrapper')) return;
      const wrapper = document.createElement('div');
      wrapper.className = 'code-block-wrapper relative group/code';
      pre.parentNode?.insertBefore(wrapper, pre);
      wrapper.appendChild(pre);

      const btn = document.createElement('button');
      btn.className = 'absolute top-2.5 right-2.5 p-1.5 rounded-md bg-muted/90 text-muted-foreground hover:text-foreground opacity-0 group-hover/code:opacity-100 transition-all cursor-pointer border border-border/50 shadow-sm';
      btn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>';
      btn.title = '复制代码';
      wrapper.appendChild(btn);

      btn.addEventListener('click', async () => {
        const code = pre.querySelector('code')?.textContent ?? pre.textContent ?? '';
        try {
          await navigator.clipboard.writeText(code);
          btn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>';
          btn.classList.add('text-success');
          setTimeout(() => {
            btn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>';
            btn.classList.remove('text-success');
          }, 2000);
        } catch {
          toast.error('复制失败');
        }
      });
    });

    return () => cleanups.forEach(fn => fn());
  }, [entry?.content]);

  // Show floating toolbar when text is selected in article
  useEffect(() => {
    const handleSelection = () => {
      const selection = window.getSelection();
      if (!selection || selection.isCollapsed || !articleRef.current) {
        setSelectionToolbar(prev => ({ ...prev, visible: false }));
        return;
      }

      const text = selection.toString().trim();
      if (!text || text.length < 2) {
        setSelectionToolbar(prev => ({ ...prev, visible: false }));
        return;
      }

      const range = selection.getRangeAt(0);
      if (!articleRef.current.contains(range.commonAncestorContainer)) {
        setSelectionToolbar(prev => ({ ...prev, visible: false }));
        return;
      }

      const rect = range.getBoundingClientRect();
      setSelectionToolbar({
        visible: true,
        text,
        x: rect.left + rect.width / 2,
        y: rect.top - 8,
      });
    };

    document.addEventListener('selectionchange', handleSelection);
    return () => document.removeEventListener('selectionchange', handleSelection);
  }, []);

  // Hide toolbar on click outside
  useEffect(() => {
    if (!selectionToolbar.visible) return;
    const handleClick = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (!target.closest('.selection-toolbar')) {
        setSelectionToolbar(prev => ({ ...prev, visible: false }));
      }
    };
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, [selectionToolbar.visible]);

  const invalidate = () => { qc.invalidateQueries({ queryKey: ['entry', id] }); qc.invalidateQueries({ queryKey: ['entries'] }); };

  const { toggleStar, toggleArchive } = useEntryActions(
    id!,
    { is_starred: entry?.is_starred ?? false, is_archived: entry?.is_archived ?? false },
  );
  const saveContent = useMutation({
    mutationFn: (html: string) => updateEntry(id!, { content: html }),
    onSuccess: () => { setEditing(false); invalidate(); },
    onError: () => toast.error('保存内容失败'),
  });
  const saveTitle = useMutation({
    mutationFn: (title: string) => updateEntry(id!, { title }),
    onSuccess: () => { setEditingTitle(false); invalidate(); },
    onError: () => toast.error('保存标题失败'),
  });
  const remove = useMutation({
    mutationFn: () => deleteEntry(id!),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['entries'] }); navigate('/'); },
    onError: () => toast.error('删除失败'),
  });
  const refetch = useMutation({
    mutationFn: () => refetchEntry(id!),
    onSuccess: () => { invalidate(); toast.success('已重新抓取'); },
    onError: (err: unknown) => { toast.error(getErrorMessage(err, '重新抓取失败')); },
  });

  if (isLoading) return (
    <div className="space-y-4 py-6 animate-fade-in">
      <Skeleton className="h-9 w-3/4 rounded-lg" />
      <Skeleton className="h-4 w-1/2 rounded-lg" />
      <div className="flex gap-2 mt-6">
        <Skeleton className="h-9 w-20 rounded-lg" />
        <Skeleton className="h-9 w-20 rounded-lg" />
        <Skeleton className="h-9 w-20 rounded-lg" />
      </div>
      <Skeleton className="h-64 w-full rounded-xl mt-4" />
      <Skeleton className="h-4 w-full rounded-lg" />
      <Skeleton className="h-4 w-5/6 rounded-lg" />
      <Skeleton className="h-4 w-4/6 rounded-lg" />
    </div>
  );

  if (error) return (
    <div className="py-8 animate-fade-in">
      <ErrorState message="文章加载失败" onRetry={() => refetchEntryQuery()} />
    </div>
  );

  if (!entry) return (
    <div className="py-8 animate-fade-in">
      <ErrorState message="文章未找到" />
    </div>
  );

  return (
    <div
      ref={isMobile ? gestureRef : undefined}
      className="flex gap-0 lg:-mx-4 animate-fade-in"
      style={isMobile && isGestureActive ? {
        transform: `translateX(${swipeOffset.x}px)`,
        transition: swipeOffset.x === 0 ? 'transform 0.2s ease-out' : 'none',
      } : undefined}
    >
      <div className={`flex-1 px-4 w-full overflow-hidden max-w-170 ${!showAnnotations ? 'lg:mx-auto' : ''}`}>
        {/* Back button */}
        <Button
          variant="ghost"
          size="sm"
          onClick={() => navigate(-1)}
          className="mb-5 -ml-2 text-muted-foreground hover:text-foreground rounded-lg"
        >
          <ArrowLeft size={16} className="mr-1.5" />
          返回
        </Button>

        {/* Title */}
        {editingTitle ? (
          <div className="flex items-center gap-2 mb-3 animate-fade-in">
            <input
              value={titleDraft}
              onChange={(e) => setTitleDraft(e.target.value)}
              className="flex-1 text-xl sm:text-2xl font-bold px-3 py-2 border border-input rounded-xl bg-card text-card-foreground focus:outline-none focus:ring-2 focus:ring-ring/50"
              autoFocus
              onKeyDown={(e) => { if (e.key === 'Enter') saveTitle.mutate(titleDraft); if (e.key === 'Escape') setEditingTitle(false); }}
            />
            <Button size="sm" onClick={() => saveTitle.mutate(titleDraft)} className="rounded-lg">保存</Button>
            <Button size="sm" variant="ghost" onClick={() => setEditingTitle(false)} className="rounded-lg">取消</Button>
          </div>
        ) : (
          <h1
            className="text-xl sm:text-[1.75rem] font-bold mb-3 cursor-pointer hover:text-primary group leading-tight tracking-tight"
            onClick={() => { setTitleDraft(entry.title || ''); setEditingTitle(true); }}
            title="点击编辑标题"
          >
            {entry.title || '无标题'}
            <span className="text-sm font-normal text-muted-foreground ml-2 opacity-0 group-hover:opacity-100 transition-opacity">编辑</span>
          </h1>
        )}

        {/* Meta info */}
        <div className="flex items-center flex-wrap gap-x-3 gap-y-1 text-[13px] text-muted-foreground mb-5">
          {entry.domain_name && (
            <a
              href={entry.url}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-1 hover:text-primary transition-colors"
            >
              <Globe size={12} />
              {entry.domain_name}
            </a>
          )}
          {entry.published_by && (
            <>
              <span className="text-border">·</span>
              <span>{entry.published_by}</span>
            </>
          )}
          {entry.reading_time && (
            <>
              <span className="text-border">·</span>
              <span className="inline-flex items-center gap-1">
                <Clock size={12} />
                {entry.reading_time} 分钟阅读
              </span>
            </>
          )}
          {entry.language && (
            <>
              <span className="text-border">·</span>
              <span className="uppercase">{entry.language}</span>
            </>
          )}
        </div>

        {/* Offline indicator */}
        {!navigator.onLine && (
          <div className="flex items-center gap-1.5 text-xs text-muted-foreground/80 mb-4 px-1">
            <WifiOff size={12} />
            <span>离线模式 — 显示已缓存的内容</span>
          </div>
        )}

        {/* Action toolbar */}
        <div className="flex items-center justify-between mb-6">
          <div className="flex items-center gap-0.5">
            <Button
              variant="ghost"
              size="icon"
              onClick={() => toggleStar.mutate()}
              className={cn(
                'h-9 w-9 rounded-lg transition-colors',
                entry.is_starred
                  ? 'text-amber-500 hover:text-amber-600 hover:bg-amber-500/10'
                  : 'text-muted-foreground hover:text-foreground hover:bg-accent'
              )}
              title={entry.is_starred ? '取消收藏' : '收藏'}
            >
              <Star size={18} className={cn(entry.is_starred && 'fill-current')} />
            </Button>

            <Button
              variant="ghost"
              size="icon"
              onClick={() => toggleArchive.mutate()}
              className={cn(
                'h-9 w-9 rounded-lg transition-colors',
                entry.is_archived
                  ? 'text-success hover:text-success hover:bg-success/10'
                  : 'text-muted-foreground hover:text-foreground hover:bg-accent'
              )}
              title={entry.is_archived ? '取消归档' : '归档'}
            >
              <Archive size={18} className={cn(entry.is_archived && 'fill-current')} />
            </Button>

            <Button
              variant={showAnnotations ? 'secondary' : 'ghost'}
              size="icon"
              onClick={() => setShowAnnotations(!showAnnotations)}
              className="h-9 w-9 rounded-lg"
              title="批注"
            >
              <MessageSquare size={18} />
            </Button>
          </div>

          <div className="flex items-center gap-0.5">
            <Button
              variant="ghost"
              size="sm"
              onClick={() => refetch.mutate()}
              disabled={refetch.isPending}
              className="h-8 px-2.5 rounded-lg text-caption text-muted-foreground hover:text-foreground"
              title="重新抓取内容"
            >
              <RefreshCw size={14} className={cn('mr-1.5', refetch.isPending && 'animate-spin')} />
              {refetch.isPending ? '抓取中' : '重抓'}
            </Button>

            {entry.content && !editing && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setEditing(true)}
                className="h-8 px-2.5 rounded-lg text-caption text-muted-foreground hover:text-foreground"
                title="编辑内容"
              >
                <Edit3 size={14} className="mr-1.5" />
                编辑
              </Button>
            )}

            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="ghost" size="icon" className="h-8 w-8 rounded-lg text-muted-foreground hover:text-foreground">
                  <MoreHorizontal size={16} />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="rounded-xl">
                <DropdownMenuItem
                  className="text-destructive focus:text-destructive rounded-lg cursor-pointer"
                  onClick={() => { if (confirm('确定删除这篇文章？')) remove.mutate(); }}
                >
                  <Trash2 size={14} className="mr-2" /> 删除
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>

        {/* Content */}
        {editing && entry.content ? (
          <ContentEditor content={entry.content} onSave={(html) => saveContent.mutate(html)} onCancel={() => setEditing(false)} />
        ) : entry.extract_method === 'pending' ? (
          <div className="flex items-center gap-2 text-warning py-8">
            <RefreshCw size={16} className="animate-spin" />
            <span>正在抓取内容...</span>
          </div>
        ) : entry.extract_method === 'failed' ? (
          <div className="rounded-xl bg-destructive/5 border border-destructive/10 p-5 text-destructive text-sm">
            内容提取失败。
            <a href={entry.url} target="_blank" rel="noopener noreferrer" className="underline ml-1 font-medium">查看原文</a>
          </div>
        ) : entry.content ? (
          <>
            <article
              ref={articleRef}
              className="entry-content max-w-none overflow-x-hidden"
              dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(entry.content, {
                FORBID_TAGS: ['iframe', 'form', 'input', 'textarea', 'select', 'button', 'object', 'embed', 'applet'],
                FORBID_ATTR: ['formaction', 'xlink:href', 'style'],
                ALLOWED_URI_REGEXP: /^(?:(?:(?:f|ht)tps?|mailto|tel):|[^a-z]|[a-z+.-]+(?:[^a-z]|$))/i,
              }) }}
            />

            {/* Floating selection toolbar */}
            {selectionToolbar.visible && (
              <div
                className="selection-toolbar fixed z-50 bg-card border border-border/60 shadow-lg rounded-lg px-1.5 py-1 flex items-center gap-0.5 animate-scale-in"
                style={{
                  left: selectionToolbar.x,
                  top: selectionToolbar.y,
                  transform: 'translateX(-50%) translateY(-100%)',
                }}
              >
                <Button
                  size="sm"
                  variant="ghost"
                  className="h-7 px-2 text-xs rounded-md gap-1.5"
                  onClick={() => {
                    setPendingQuote(selectionToolbar.text);
                    setAnnotationKey(prev => prev + 1);
                    setShowAnnotations(true);
                    setSelectionToolbar(prev => ({ ...prev, visible: false }));
                    window.getSelection()?.removeAllRanges();
                  }}
                >
                  <MessageSquare size={13} />
                  添加批注
                </Button>
              </div>
            )}
          </>
        ) : (
          <p className="text-muted-foreground py-8">暂无内容</p>
        )}

        <Separator className="my-8" />

        {/* Footer link */}
        <div>
          <a
            href={entry.url}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-1.5 text-sm font-medium text-primary hover:opacity-70 transition-opacity"
          >
            查看原文
            <ExternalLink size={13} />
            {entry.domain_name && (
              <span className="text-muted-foreground font-normal">({entry.domain_name})</span>
            )}
          </a>
        </div>

        {id && <EntryTags entryId={id} />}
      </div>

      {/* Annotations sidebar */}
      {isMobile ? (
        <Sheet open={showAnnotations} onOpenChange={setShowAnnotations}>
          <SheetContent side="bottom" className="h-[65dvh] rounded-t-2xl pb-[env(safe-area-inset-bottom)]">
            {id && <AnnotationsSidebar key={annotationKey} entryId={id} compact preselectedQuote={pendingQuote} />}
          </SheetContent>
        </Sheet>
      ) : (
        showAnnotations && id && <AnnotationsSidebar key={annotationKey} entryId={id} preselectedQuote={pendingQuote} />
      )}
    </div>
  );
}
