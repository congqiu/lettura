import { useState, useRef, useEffect } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { addTagToEntry, removeTagFromEntry, fetchTagStats, type Tag, type TagStats } from '../api/tags';
import { toast } from 'sonner';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
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
    queryFn: async (): Promise<Tag[]> => {
      const res = await fetch(`/api/v1/entries/${entryId}/tags`, {
        headers: { Authorization: `Bearer ${localStorage.getItem('access_token')}` },
      });
      return res.json();
    },
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
    <div className="mt-6 pt-4 border-t border-border">
      <h4 className="text-sm font-medium text-muted-foreground mb-2">标签</h4>
      <div className="flex flex-wrap gap-2 mb-2">
        {entryTags.map((tag) => (
          <Badge key={tag.id} variant="secondary" className="flex items-center gap-1">
            {tag.label}
            <Button
              size="sm"
              variant="ghost"
              className="h-4 w-4 p-0 hover:text-destructive"
              onClick={() => removeTag.mutate(tag.id)}
            >
              &times;
            </Button>
          </Badge>
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
            className="h-8 text-sm"
            disabled={isAdding}
          />
          <Button type="submit" size="sm" disabled={!input.trim() || isAdding}>
            {isAdding ? '添加中...' : '添加'}
          </Button>
        </form>
        {showSuggestions && debouncedInput && suggestions.length > 0 && (
          <div className="absolute z-50 w-full mt-1">
            <Command className="border border-border shadow-md">
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
                      <span className="ml-auto text-xs text-muted-foreground">{tag.entry_count}</span>
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
