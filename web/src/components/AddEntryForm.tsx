import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { createEntry } from '../api/entries';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { toast } from 'sonner';

export default function AddEntryForm() {
  const [url, setUrl] = useState('');
  const qc = useQueryClient();

  const mutation = useMutation({
    mutationFn: (url: string) => createEntry(url),
    onSuccess: () => {
      setUrl('');
      qc.invalidateQueries({ queryKey: ['entries'] });
      toast.success('文章已保存');
    },
    onError: (err: any) => {
      toast.error(err.response?.data?.message || '保存失败');
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!url.trim()) return;
    mutation.mutate(url.trim());
  };

  return (
    <form onSubmit={handleSubmit} className="mb-6">
      <div className="flex gap-2">
        <Input
          type="url"
          placeholder="粘贴 URL 保存文章..."
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          className="flex-1 h-10 bg-card"
          required
        />
        <Button type="submit" disabled={mutation.isPending}>
          {mutation.isPending ? '保存中...' : '保存'}
        </Button>
      </div>
    </form>
  );
}