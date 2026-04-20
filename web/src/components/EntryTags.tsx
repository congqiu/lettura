import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import api from '../api/client';
import { addTagToEntry, removeTagFromEntry, type Tag } from '../api/tags';
import { toast } from 'sonner';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

export default function EntryTags({ entryId }: { entryId: string }) {
  const [input, setInput] = useState('');
  const qc = useQueryClient();

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
    onError: () => toast.error('添加标签失败'),
  });

  const removeTag = useMutation({
    mutationFn: (tagId: string) => removeTagFromEntry(entryId, tagId),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['tags'] }),
    onError: () => toast.error('删除标签失败'),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim()) return;
    addTag.mutate(input.trim());
  };

  return (
    <div className="mt-6 pt-4 border-t border-border">
      <h4 className="text-sm font-medium text-muted-foreground mb-2">标签</h4>
      <div className="flex flex-wrap gap-2 mb-2">
        {allTags.map((tag) => (
            <Badge key={tag.id} variant="secondary" className="flex items-center gap-1">
              {tag.label}
              <Button size="sm" variant="ghost" className="h-4 w-4 p-0 hover:text-destructive" onClick={() => removeTag.mutate(tag.id)}>&times;</Button>
            </Badge>
        ))}
      </div>
      <form onSubmit={handleSubmit} className="flex gap-2">
        <Input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="添加标签..."
          className="h-8 text-sm"
        />
        <Button type="submit" size="sm" disabled={!input.trim()}>添加</Button>
      </form>
    </div>
  );
}