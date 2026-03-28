import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  listAnnotations, createAnnotation, updateAnnotation, deleteAnnotation,
  type Annotation,
} from '../api/annotations';

interface Props { entryId: string; }

export default function AnnotationsSidebar({ entryId }: Props) {
  const [newQuote, setNewQuote] = useState('');
  const [newText, setNewText] = useState('');
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editText, setEditText] = useState('');
  const qc = useQueryClient();

  const { data: annotations = [] } = useQuery({
    queryKey: ['annotations', entryId],
    queryFn: () => listAnnotations(entryId),
  });

  const create = useMutation({
    mutationFn: () => createAnnotation(entryId, newQuote, newText),
    onSuccess: () => { setNewQuote(''); setNewText(''); qc.invalidateQueries({ queryKey: ['annotations', entryId] }); },
  });

  const update = useMutation({
    mutationFn: ({ id, text }: { id: string; text: string }) => updateAnnotation(id, text),
    onSuccess: () => { setEditingId(null); qc.invalidateQueries({ queryKey: ['annotations', entryId] }); },
  });

  const remove = useMutation({
    mutationFn: (id: string) => deleteAnnotation(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['annotations', entryId] }),
  });

  const handleCaptureSelection = () => {
    const selection = window.getSelection();
    if (selection && selection.toString().trim()) {
      setNewQuote(selection.toString().trim());
    }
  };

  return (
    <div className="w-80 border-l border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-4 overflow-y-auto">
      <h3 className="font-medium mb-4">批注</h3>

      <div className="mb-4 p-3 bg-white dark:bg-gray-800 rounded border border-gray-200 dark:border-gray-700">
        <button onClick={handleCaptureSelection} className="text-xs text-blue-600 dark:text-blue-400 hover:underline mb-2 block">
          捕获选中文字
        </button>
        {newQuote && (
          <blockquote className="text-sm text-gray-600 dark:text-gray-400 border-l-2 border-blue-300 dark:border-blue-600 pl-2 mb-2 italic">
            {newQuote}
          </blockquote>
        )}
        <textarea
          value={newText}
          onChange={(e) => setNewText(e.target.value)}
          placeholder="添加笔记..."
          className="w-full text-sm px-2 py-1 border border-gray-200 dark:border-gray-600 rounded resize-none h-16 bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
        />
        <button onClick={() => create.mutate()} disabled={!newQuote || create.isPending}
          className="mt-1 text-xs px-3 py-1 bg-blue-600 text-white rounded disabled:opacity-50">
          添加
        </button>
      </div>

      {annotations.length === 0 ? (
        <p className="text-sm text-gray-400 dark:text-gray-600">暂无批注。选中文字后点击"捕获选中文字"。</p>
      ) : (
        <div className="space-y-3">
          {annotations.map((ann: Annotation) => (
            <div key={ann.id} className={`p-3 bg-white dark:bg-gray-800 rounded border ${ann.is_orphaned ? 'border-yellow-300 dark:border-yellow-700' : 'border-gray-200 dark:border-gray-700'}`}>
              {ann.is_orphaned && <span className="text-xs text-yellow-600 dark:text-yellow-500 block mb-1">已失效（内容被编辑）</span>}
              <blockquote className="text-sm text-gray-600 dark:text-gray-400 border-l-2 border-gray-300 dark:border-gray-600 pl-2 mb-2 italic">{ann.quote}</blockquote>
              {editingId === ann.id ? (
                <div>
                  <textarea value={editText} onChange={(e) => setEditText(e.target.value)}
                    className="w-full text-sm px-2 py-1 border border-gray-200 dark:border-gray-600 rounded resize-none h-12 bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100" />
                  <div className="flex gap-1 mt-1">
                    <button onClick={() => update.mutate({ id: ann.id, text: editText })} className="text-xs px-2 py-1 bg-blue-600 text-white rounded">保存</button>
                    <button onClick={() => setEditingId(null)} className="text-xs px-2 py-1 text-gray-500 dark:text-gray-400">取消</button>
                  </div>
                </div>
              ) : (
                <>
                  {ann.text && <p className="text-sm mb-2 text-gray-900 dark:text-gray-100">{ann.text}</p>}
                  <div className="flex gap-2 text-xs">
                    <button onClick={() => { setEditingId(ann.id); setEditText(ann.text); }} className="text-blue-600 dark:text-blue-400 hover:underline">编辑</button>
                    <button onClick={() => remove.mutate(ann.id)} className="text-red-500 dark:text-red-400 hover:underline">删除</button>
                  </div>
                </>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
