import { useState, useRef, useEffect } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { addTagToEntry, removeTagFromEntry, fetchTagStats, type Tag, type TagStats } from '../api/tags';
import { apiGet } from '../api/client';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Tag as TagIcon, X, Loader2 } from 'lucide-react';
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandItem,
  CommandList,
} from '@/components/ui/command';

export default function EntryTags({ entryId }: { entryId: string }) {
  const [input, setInput] = useState('');
  const [isAdding, setIsAdding] = useState(false);
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [debouncedInput, setDebouncedInput] = useState('');
  const qc = useQueryClient();
  const inputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Fetch entry-specific tags
  const { data: entryTags = [] } = useQuery({
    queryKey: ['entry-tags', entryId],
    queryFn: () => apiGet<Tag[]>(`/entries/${entryId}/tags`),
  });

  // Fetch all tag stats for autocomplete
  const { data: tagStats = [] } = useQuery({
    queryKey: ['tags', 'stats'],
    queryFn: fetchTagStats,
    staleTime: 5 * 60 * 1000,
  });

  // Debounce input for filtering
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedInput(input);
    }, 200);
    return () => clearTimeout(timer);
  }, [input]);

  // Close suggestions when clicking outside
  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setShowSuggestions(false);
      }
    }
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  // Compute suggestions
  const appliedLabels = new Set(entryTags.map((t) => t.label.toLowerCase()));
  const suggestions = tagStats
    .filter((t: TagStats) =>
      t.label.toLowerCase().includes(debouncedInput.toLowerCase()) &&
      !appliedLabels.has(t.label.toLowerCase())
    )
    .slice(0, 10);

  const addTag = useMutation({
    mutationFn: (label: string) => addTagToEntry(entryId, label),
    onMutate: () => { setIsAdding(true); },
    onSuccess: () => {
      setInput('');
      setShowSuggestions(false);
      qc.invalidateQueries({ queryKey: ['entry-tags', entryId] });
      qc.invalidateQueries({ queryKey: ['tags', 'stats'] });
    },
    onError: () => toast.error('添加标签失败'),
    onSettled: () => { setIsAdding(false); },
  });

  const removeTag = useMutation({
    mutationFn: (tagId: string) => removeTagFromEntry(entryId, tagId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['entry-tags', entryId] });
      qc.invalidateQueries({ queryKey: ['tags', 'stats'] });
    },
    onError: () => toast.error('删除标签失败'),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim() || isAdding) return;
    addTag.mutate(input.trim());
  };

  const handleSelect = (label: string) => {
    if (isAdding) return;
    addTag.mutate(label);
  };

  return (
    <div className="mt-8 pt-5 border-t border-border/60">
      <h4 className="text-sm font-semibold text-foreground mb-3 flex items-center gap-1.5">
        <TagIcon size={14} className="text-muted-foreground/50" />
        标签
      </h4>
      <div className="flex flex-wrap gap-2 mb-3">
        {entryTags.map((tag) => (
          <span
            key={tag.id}
            className="inline-flex items-center gap-1 text-[12px] font-medium px-2.5 py-1 rounded-full bg-secondary text-secondary-foreground"
          >
            {tag.label}
            <button
              onClick={() => removeTag.mutate(tag.id)}
              className="inline-flex items-center justify-center w-4 h-4 rounded-full hover:bg-destructive/10 hover:text-destructive transition-colors"
            >
              <X size={10} strokeWidth={2.5} />
            </button>
          </span>
        ))}
      </div>
      <div ref={containerRef} className="relative">
        <form onSubmit={handleSubmit} className="flex gap-2">
          <Input
            ref={inputRef}
            value={input}
            onChange={(e) => {
              setInput(e.target.value);
              setShowSuggestions(true);
            }}
            onFocus={() => setShowSuggestions(true)}
            placeholder="添加标签..."
            className="h-9 text-sm rounded-lg"
            disabled={isAdding}
          />
          <Button type="submit" size="sm" disabled={!input.trim() || isAdding} className="h-9 rounded-lg px-3">
            {isAdding ? (
              <Loader2 size={14} className="animate-spin" />
            ) : (
              '添加'
            )}
          </Button>
        </form>
        {showSuggestions && debouncedInput && suggestions.length > 0 && (
          <div className="absolute z-50 w-full mt-1">
            <Command className="border border-border/60 shadow-lg rounded-xl">
              <CommandList>
                <CommandEmpty>无匹配标签</CommandEmpty>
                <CommandGroup>
                  {suggestions.map((tag: TagStats) => (
                    <CommandItem
                      key={tag.id}
                      value={tag.label}
                      onSelect={() => handleSelect(tag.label)}
                    >
                      {tag.label}
                      <span className="ml-auto text-xs text-muted-foreground tabular-nums">{tag.entry_count}</span>
                    </CommandItem>
                  ))}
                </CommandGroup>
              </CommandList>
            </Command>
          </div>
        )}
      </div>
    </div>
  );
}
