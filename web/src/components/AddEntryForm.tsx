import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { createEntry } from '../api/entries';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { toast } from 'sonner';
import { Link2, Loader2, ArrowRight } from 'lucide-react';

export default function AddEntryForm() {
  const [url, setUrl] = useState('');
  const [focused, setFocused] = useState(false);
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
      <div
        className={
          `flex gap-2 p-1.5 rounded-xl border bg-card transition-all duration-200 ` +
          (focused ? 'border-primary/40 shadow-sm shadow-primary/5 ring-2 ring-primary/10' : 'border-border/60')
        }
      >
        <div className="relative flex-1">
          <Link2 className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground/40" />
          <Input
            type="url"
            placeholder="粘贴 URL 保存文章..."
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            onFocus={() => setFocused(true)}
            onBlur={() => setFocused(false)}
            className="border-0 bg-transparent shadow-none focus-visible:ring-0 pl-9 h-10 text-[15px] placeholder:text-muted-foreground/40"
            required
          />
        </div>
        <Button
          type="submit"
          disabled={mutation.isPending || !url.trim()}
          className="h-10 px-4 rounded-lg gap-1.5 shrink-0"
        >
          {mutation.isPending ? (
            <Loader2 size={16} className="animate-spin" />
          ) : (
            <>
              保存
              <ArrowRight size={14} className="opacity-70" />
            </>
          )}
        </Button>
      </div>
    </form>
  );
}
