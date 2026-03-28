import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { createEntry } from '../api/entries';

export default function AddEntryForm() {
  const [url, setUrl] = useState('');
  const [error, setError] = useState('');
  const qc = useQueryClient();

  const mutation = useMutation({
    mutationFn: (url: string) => createEntry(url),
    onSuccess: () => {
      setUrl('');
      setError('');
      qc.invalidateQueries({ queryKey: ['entries'] });
    },
    onError: (err: any) => {
      setError(err.response?.data?.message || '保存失败');
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!url.trim()) return;
    mutation.mutate(url.trim());
  };

  return (
    <form onSubmit={handleSubmit} className="flex gap-2 mb-6">
      <input
        type="url"
        placeholder="粘贴 URL 保存文章..."
        value={url}
        onChange={(e) => setUrl(e.target.value)}
        className="flex-1 px-3 py-2 border border-gray-200 dark:border-gray-700 rounded bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-600 focus:outline-none focus:ring-2 focus:ring-blue-500"
        required
      />
      <button
        type="submit"
        disabled={mutation.isPending}
        className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
      >
        {mutation.isPending ? '保存中...' : '保存'}
      </button>
      {error && <span className="text-red-500 text-sm self-center">{error}</span>}
    </form>
  );
}
