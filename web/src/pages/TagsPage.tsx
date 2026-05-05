import { Link } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { fetchTagStats } from '../api/tags';
import { Tag, TagIcon, ArrowRight } from 'lucide-react';
import { cn } from '@/lib/utils';

export default function TagsPage() {
  const { data: tagStats = [], isLoading } = useQuery({
    queryKey: ['tags', 'stats'],
    queryFn: fetchTagStats,
  });

  const tagsWithEntries = tagStats.filter((t) => t.entry_count > 0);
  const tagsWithoutEntries = tagStats.filter((t) => t.entry_count === 0);

  return (
    <div className="animate-fade-in">
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-2.5">
          <div className="w-9 h-9 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
            <TagIcon size={18} />
          </div>
          <div>
            <h2 className="text-xl font-bold tracking-tight text-foreground">标签</h2>
            <p className="text-xs text-muted-foreground">
              {tagsWithEntries.length} 个在用标签
            </p>
          </div>
        </div>
        <Link
          to="/?untagged=true"
          className="inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-primary transition-colors"
        >
          未标签文章
          <ArrowRight size={14} />
        </Link>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12">
          <div className="w-5 h-5 border-2 border-muted border-t-primary rounded-full animate-spin" />
        </div>
      ) : tagsWithEntries.length === 0 ? (
        <div className="text-center py-16">
          <div className="w-14 h-14 rounded-2xl bg-secondary flex items-center justify-center mx-auto mb-4">
            <Tag size={26} className="text-muted-foreground/50" />
          </div>
          <p className="text-muted-foreground text-sm">暂无标签</p>
        </div>
      ) : (
        <>
          <div className="flex flex-wrap gap-2">
            {tagsWithEntries.map((tag) => (
              <Link
                key={tag.id}
                to={`/?tag=${encodeURIComponent(tag.label)}`}
                className={cn(
                  'group inline-flex items-center gap-2 rounded-xl px-4 py-2.5 text-sm font-medium',
                  'bg-card border border-border/60 hover:border-primary/30 hover:bg-primary/5',
                  'transition-all duration-200'
                )}
              >
                <span className="text-foreground">{tag.label}</span>
                <span className="text-[11px] tabular-nums text-muted-foreground bg-muted/50 px-1.5 py-0.5 rounded-full">
                  {tag.entry_count}
                </span>
              </Link>
            ))}
          </div>

          {tagsWithoutEntries.length > 0 && (
            <div className="mt-8">
              <h3 className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground/60 mb-3">
                无文章的标签
              </h3>
              <div className="flex flex-wrap gap-2">
                {tagsWithoutEntries.map((tag) => (
                  <Link
                    key={tag.id}
                    to="/settings"
                    className="inline-flex items-center rounded-lg bg-muted/50 px-3 py-1.5 text-xs text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                  >
                    {tag.label}
                  </Link>
                ))}
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}
