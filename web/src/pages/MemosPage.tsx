import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { listMemos, createMemo, deleteMemo, promoteMemo } from '../api/memos';

export default function MemosPage() {
  const [content, setContent] = useState('');
  const qc = useQueryClient();

  const { data: memos = [], isLoading } = useQuery({
    queryKey: ['memos'],
    queryFn: listMemos,
  });

  const create = useMutation({
    mutationFn: (content: string) => createMemo(content),
    onSuccess: () => { setContent(''); qc.invalidateQueries({ queryKey: ['memos'] }); },
  });

  const remove = useMutation({
    mutationFn: (id: string) => deleteMemo(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['memos'] }),
  });

  const promote = useMutation({
    mutationFn: (id: string) => promoteMemo(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['memos'] }),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!content.trim()) return;
    create.mutate(content.trim());
  };

  return (
    <div>
      <h2 className="text-xl font-semibold mb-4">收集箱</h2>

      <form onSubmit={handleSubmit} className="mb-6">
        <textarea
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder="快速记录 — 文字、URL 或想法..."
          className="w-full px-3 py-2 border border-gray-200 dark:border-gray-700 rounded resize-none h-20 bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-600 focus:outline-none focus:ring-2 focus:ring-blue-500"
          autoFocus
        />
        <button
          type="submit"
          disabled={create.isPending || !content.trim()}
          className="mt-2 px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
        >
          {create.isPending ? '保存中...' : '保存'}
        </button>
      </form>

      {isLoading ? (
        <p className="text-gray-500">加载中...</p>
      ) : memos.length === 0 ? (
        <p className="text-gray-500">暂无收集</p>
      ) : (
        <div className="space-y-3">
          {memos.map((memo) => (
            <div key={memo.id} className="bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-lg p-4">
              <p className="whitespace-pre-wrap text-gray-900 dark:text-gray-100">{memo.content}</p>
              <div className="flex items-center gap-2 mt-3 text-sm">
                <span className="text-gray-400 dark:text-gray-600">
                  {new Date(memo.created_at).toLocaleDateString('zh-CN')}
                </span>
                {memo.promoted_entry_id ? (
                  <span className="text-green-600 dark:text-green-400">已转化</span>
                ) : (
                  <button onClick={() => promote.mutate(memo.id)} className="text-blue-600 dark:text-blue-400 hover:underline">
                    转为文章
                  </button>
                )}
                <button onClick={() => remove.mutate(memo.id)} className="text-red-500 dark:text-red-400 hover:underline">
                  删除
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
