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
import { toast } from '../components/Toast';

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
  const saveContent = useMutation({ mutationFn: (html: string) => updateEntry(id!, { content: html }), onSuccess: () => { setEditing(false); invalidate(); }, onError: () => toast('error', '保存内容失败') });
  const saveTitle = useMutation({
    mutationFn: (title: string) => updateEntry(id!, { title }),
    onSuccess: () => { setEditingTitle(false); invalidate(); },
    onError: () => toast('error', '保存标题失败'),
  });
  const remove = useMutation({ mutationFn: () => deleteEntry(id!), onSuccess: () => { qc.invalidateQueries({ queryKey: ['entries'] }); navigate('/'); }, onError: () => toast('error', '删除失败') });
  const refetch = useMutation({
    mutationFn: () => refetchEntry(id!),
    onSuccess: () => { invalidate(); toast('success', '已重新抓取'); },
    onError: (err: unknown) => {
      toast('error', getErrorMessage(err, '重新抓取失败'));
    },
  });

  if (isLoading) return (
    <div className="flex justify-center py-16">
      <div className="w-6 h-6 border-2 border-gray-300 dark:border-gray-600 border-t-blue-500 rounded-full animate-spin" />
    </div>
  );
  if (error) return <div className="py-8"><ErrorState message="文章加载失败" onRetry={() => refetchEntryQuery()} /></div>;
  if (!entry) return <div className="py-8"><ErrorState message="文章未找到" /></div>;

  const btnBase = "text-sm px-3 py-1.5 rounded border transition-colors";
  const btnNormal = `${btnBase} border-gray-200 dark:border-gray-700 hover:bg-gray-100 dark:hover:bg-gray-800`;

  return (
    <div className="flex gap-0 -mx-4">
      <div className={`flex-1 px-4 ${showAnnotations ? 'max-w-3xl' : 'max-w-3xl mx-auto'}`}>
        <button onClick={() => navigate(-1)} className="text-sm text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 mb-4">
          &larr; 返回
        </button>

        {editingTitle ? (
          <div className="flex items-center gap-2 mb-2">
            <input
              value={titleDraft}
              onChange={(e) => setTitleDraft(e.target.value)}
              className="flex-1 text-2xl font-bold px-2 py-1 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
              autoFocus
              onKeyDown={(e) => { if (e.key === 'Enter') saveTitle.mutate(titleDraft); if (e.key === 'Escape') setEditingTitle(false); }}
            />
            <button onClick={() => saveTitle.mutate(titleDraft)} className="text-sm px-3 py-1 bg-blue-600 text-white rounded">保存</button>
            <button onClick={() => setEditingTitle(false)} className="text-sm px-3 py-1 text-gray-500 dark:text-gray-400">取消</button>
          </div>
        ) : (
          <h1
            className="text-2xl font-bold mb-2 cursor-pointer hover:text-blue-600 dark:hover:text-blue-400 group"
            onClick={() => { setTitleDraft(entry.title || ''); setEditingTitle(true); }}
            title="点击编辑标题"
          >
            {entry.title || '无标题'}
            <span className="text-sm font-normal text-gray-400 dark:text-gray-600 ml-2 opacity-0 group-hover:opacity-100 transition-opacity">编辑</span>
          </h1>
        )}

        <div className="flex items-center gap-3 text-sm text-gray-500 dark:text-gray-500 mb-4">
          {entry.domain_name && (
            <a href={entry.url} target="_blank" rel="noopener noreferrer" className="hover:underline hover:text-gray-700 dark:hover:text-gray-300">
              {entry.domain_name}
            </a>
          )}
          {entry.published_by && <span>作者: {entry.published_by}</span>}
          {entry.reading_time && <span>{entry.reading_time} 分钟阅读</span>}
          {entry.language && <span>{entry.language}</span>}
        </div>

        <div className="flex gap-2 mb-6 flex-wrap">
          <button onClick={() => toggleStar.mutate()}
            className={`${btnBase} ${entry.is_starred ? 'bg-yellow-100 dark:bg-yellow-900/30 border-yellow-300 dark:border-yellow-800 text-yellow-700 dark:text-yellow-400' : btnNormal}`}>
            {entry.is_starred ? '已收藏' : '收藏'}
          </button>
          <button onClick={() => toggleArchive.mutate()}
            className={`${btnBase} ${entry.is_archived ? 'bg-green-100 dark:bg-green-900/30 border-green-300 dark:border-green-800 text-green-700 dark:text-green-400' : btnNormal}`}>
            {entry.is_archived ? '已归档' : '归档'}
          </button>
          {entry.content && !editing && (
            <button onClick={() => setEditing(true)} className={btnNormal}>编辑内容</button>
          )}
          <button onClick={() => setShowAnnotations(!showAnnotations)}
            className={`${btnBase} ${showAnnotations ? 'bg-purple-100 dark:bg-purple-900/30 border-purple-300 dark:border-purple-800 text-purple-700 dark:text-purple-400' : btnNormal}`}>
            批注
          </button>
          <button onClick={() => { if (confirm('确定删除这篇文章？')) remove.mutate(); }}
            className={`${btnBase} border-gray-200 dark:border-gray-700 text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20`}>
            删除
          </button>
        </div>

        {editing && entry.content ? (
          <ContentEditor content={entry.content} onSave={(html) => saveContent.mutate(html)} onCancel={() => setEditing(false)} />
        ) : entry.extract_method === 'pending' ? (
          <p className="text-yellow-600 dark:text-yellow-500">正在抓取内容...</p>
        ) : entry.extract_method === 'failed' ? (
          <p className="text-red-500">内容提取失败。
            <button onClick={() => refetch.mutate()} disabled={refetch.isPending} className="underline ml-1">
              {refetch.isPending ? '抓取中...' : '重新抓取'}
            </button>
            <a href={entry.url} target="_blank" className="underline ml-1">查看原文</a>
          </p>
        ) : entry.content ? (
          <article className="prose prose-gray dark:prose-invert max-w-none" dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(entry.content) }} />
        ) : (
          <p className="text-gray-500">暂无内容</p>
        )}

        {/* 原文链接 */}
        <div className="mt-6 pt-4 border-t border-gray-200 dark:border-gray-800">
          <a href={entry.url} target="_blank" rel="noopener noreferrer"
            className="text-sm text-blue-600 dark:text-blue-400 hover:underline">
            查看原文 ↗ {entry.domain_name && `(${entry.domain_name})`}
          </a>
        </div>

        {/* 标签 */}
        {id && <EntryTags entryId={id} />}
      </div>

      {showAnnotations && id && <AnnotationsSidebar entryId={id} />}
    </div>
  );
}
