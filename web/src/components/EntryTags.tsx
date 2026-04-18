import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import api from '../api/client';
import { addTagToEntry, removeTagFromEntry, type Tag } from '../api/tags';
import { toast } from './Toast';

export default function EntryTags({ entryId }: { entryId: string }) {
  const [input, setInput] = useState('');
  const qc = useQueryClient();

  // Fetch tags for this entry via a custom query (no dedicated API yet, use list + filter)
  // Actually we have GET /api/entries/:id which doesn't return tags
  // We need to query entry_tags - for now use the tags list and let user add
  const { data: allTags = [] } = useQuery({
    queryKey: ['tags'],
    queryFn: async () => {
      const res = await api.get('/tags');
      return res.data as Tag[];
    },
  });

  const addTag = useMutation({
    mutationFn: (label: string) => addTagToEntry(entryId, label),
    onSuccess: () => { setInput(''); qc.invalidateQueries({ queryKey: ['tags'] }); },
    onError: () => toast('error', '添加标签失败'),
  });

  const removeTag = useMutation({
    mutationFn: (tagId: string) => removeTagFromEntry(entryId, tagId),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['tags'] }),
    onError: () => toast('error', '删除标签失败'),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim()) return;
    addTag.mutate(input.trim());
  };

  return (
    <div className="mt-6 pt-4 border-t border-gray-200 dark:border-gray-800">
      <h4 className="text-sm font-medium text-gray-600 dark:text-gray-400 mb-2">标签</h4>
      <div className="flex flex-wrap gap-2 mb-2">
        {allTags.map((tag) => (
          <span key={tag.id} className="inline-flex items-center gap-1 text-xs bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 px-2 py-1 rounded">
            {tag.label}
            <button onClick={() => removeTag.mutate(tag.id)} className="hover:text-red-500 font-bold">&times;</button>
          </span>
        ))}
      </div>
      <form onSubmit={handleSubmit} className="flex gap-2">
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="添加标签..."
          className="px-2 py-1 text-sm border border-gray-200 dark:border-gray-700 rounded bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-600 focus:outline-none focus:ring-1 focus:ring-blue-500"
        />
        <button type="submit" disabled={!input.trim()} className="text-xs px-2 py-1 bg-blue-600 text-white rounded disabled:opacity-50">添加</button>
      </form>
    </div>
  );
}
