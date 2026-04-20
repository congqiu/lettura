import { useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
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
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { ArrowLeft, Star, Archive, RefreshCw, Edit3, MessageSquare, Trash2, MoreHorizontal } from 'lucide-react';

export default function EntryDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const qc = useQueryClient();
  const [editing, setEditing] = useState(false);
  const [showAnnotations, setShowAnnotations] = useState(false);
  const [editingTitle, setEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState('');

  const { data: entry, isLoading, error, refetch: refetchEntryQuery } = useQuery({
    queryKey: ['entry', id],
    queryFn: () => getEntry(id!),
    enabled: !!id,
  });

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
    <div className="space-y-4 py-8">
      <Skeleton className="h-8 w-3/4" />
      <Skeleton className="h-4 w-1/2" />
      <Skeleton className="h-64 w-full" />
    </div>
  );
  if (error) return <div className="py-8"><ErrorState message="文章加载失败" onRetry={() => refetchEntryQuery()} /></div>;
  if (!entry) return <div className="py-8"><ErrorState message="文章未找到" /></div>;

  return (
    <div className="flex gap-0 -mx-4">
      <div className={`flex-1 px-4 ${showAnnotations ? 'max-w-3xl' : 'max-w-3xl mx-auto'}`}>
        <Button variant="ghost" size="sm" onClick={() => navigate(-1)} className="mb-4 -ml-2 text-muted-foreground">
          <ArrowLeft size={16} className="mr-1" /> 返回
        </Button>

        {editingTitle ? (
          <div className="flex items-center gap-2 mb-2">
            <input
              value={titleDraft}
              onChange={(e) => setTitleDraft(e.target.value)}
              className="flex-1 text-2xl font-bold px-2 py-1 border border-input rounded-lg bg-card text-card-foreground focus:outline-none focus:ring-2 focus:ring-ring"
              autoFocus
              onKeyDown={(e) => { if (e.key === 'Enter') saveTitle.mutate(titleDraft); if (e.key === 'Escape') setEditingTitle(false); }}
            />
            <Button size="sm" onClick={() => saveTitle.mutate(titleDraft)}>保存</Button>
            <Button size="sm" variant="ghost" onClick={() => setEditingTitle(false)}>取消</Button>
          </div>
        ) : (
          <h1
            className="text-2xl font-bold mb-2 cursor-pointer hover:text-primary group"
            onClick={() => { setTitleDraft(entry.title || ''); setEditingTitle(true); }}
            title="点击编辑标题"
          >
            {entry.title || '无标题'}
            <span className="text-sm font-normal text-muted-foreground ml-2 opacity-0 group-hover:opacity-100 transition-opacity">编辑</span>
          </h1>
        )}

        <div className="flex items-center gap-2 text-sm text-muted-foreground mb-4">
          {entry.domain_name && (
            <a href={entry.url} target="_blank" rel="noopener noreferrer" className="hover:text-foreground hover:underline">
              {entry.domain_name}
            </a>
          )}
          {entry.published_by && <span>作者: {entry.published_by}</span>}
          {entry.reading_time && <span>{entry.reading_time} 分钟阅读</span>}
          {entry.language && <span>{entry.language}</span>}
        </div>

        <div className="flex gap-2 mb-6 flex-wrap">
          <Button
            variant={entry.is_starred ? 'default' : 'outline'}
            size="sm"
            onClick={() => toggleStar.mutate()}
            className={entry.is_starred ? 'bg-amber-500 hover:bg-amber-600 text-white' : ''}
          >
            <Star size={14} className={`mr-1 ${entry.is_starred ? 'fill-current' : ''}`} />
            {entry.is_starred ? '已收藏' : '收藏'}
          </Button>
          <Button
            variant={entry.is_archived ? 'default' : 'outline'}
            size="sm"
            onClick={() => toggleArchive.mutate()}
            className={entry.is_archived ? 'bg-green-600 hover:bg-green-700 text-white' : ''}
          >
            <Archive size={14} className={`mr-1 ${entry.is_archived ? 'fill-current' : ''}`} />
            {entry.is_archived ? '已归档' : '归档'}
          </Button>

          <Button variant="outline" size="sm" onClick={() => refetch.mutate()} disabled={refetch.isPending}>
            <RefreshCw size={14} className={`mr-1 ${refetch.isPending ? 'animate-spin' : ''}`} />
            {refetch.isPending ? '抓取中...' : '重新抓取'}
          </Button>

          {entry.content && !editing && (
            <Button variant="outline" size="sm" onClick={() => setEditing(true)}>
              <Edit3 size={14} className="mr-1" /> 编辑内容
            </Button>
          )}

          <Button
            variant={showAnnotations ? 'default' : 'outline'}
            size="sm"
            onClick={() => setShowAnnotations(!showAnnotations)}
          >
            <MessageSquare size={14} className="mr-1" />
            批注
          </Button>

          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="icon" className="h-8 w-8">
                <MoreHorizontal size={16} />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem
                className="text-destructive focus:text-destructive"
                onClick={() => { if (confirm('确定删除这篇文章？')) remove.mutate(); }}
              >
                <Trash2 size={14} className="mr-2" /> 删除
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>

        {editing && entry.content ? (
          <ContentEditor content={entry.content} onSave={(html) => saveContent.mutate(html)} onCancel={() => setEditing(false)} />
        ) : entry.extract_method === 'pending' ? (
          <p className="text-amber-600 dark:text-amber-400">正在抓取内容...</p>
        ) : entry.extract_method === 'failed' ? (
          <p className="text-destructive">内容提取失败。
            <a href={entry.url} target="_blank" className="underline ml-1">查看原文</a>
          </p>
        ) : entry.content ? (
          <article className="prose prose-gray dark:prose-invert max-w-none" dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(entry.content) }} />
        ) : (
          <p className="text-muted-foreground">暂无内容</p>
        )}

        <Separator className="my-6" />

        <div>
          <a href={entry.url} target="_blank" rel="noopener noreferrer"
            className="text-sm text-primary hover:underline">
            查看原文 ↗ {entry.domain_name && `(${entry.domain_name})`}
          </a>
        </div>

        {id && <EntryTags entryId={id} />}
      </div>

      {showAnnotations && id && <AnnotationsSidebar entryId={id} />}
    </div>
  );
}
