import { Link } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { fetchTagStats } from '../api/tags';
import { Pencil } from 'lucide-react';

export default function TagsPage() {
  const { data: tagStats = [], isLoading } = useQuery({
    queryKey: ['tags', 'stats'],
    queryFn: fetchTagStats,
  });

  const tagsWithEntries = tagStats.filter((t) => t.entry_count > 0);
  const tagsWithoutEntries = tagStats.filter((t) => t.entry_count === 0);

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-2xl font-bold tracking-tight">标签</h2>
        <Link
          to="/?untagged=true"
          className="text-sm text-muted-foreground hover:text-foreground transition-colors"
        >
          查看未标签文章
        </Link>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12">
          <div className="w-5 h-5 border-2 border-muted border-t-primary rounded-full animate-spin" />
        </div>
      ) : tagsWithEntries.length === 0 ? (
        <p className="text-muted-foreground text-center py-12">暂无标签</p>
      ) : (
        <>
          <div className="flex flex-wrap gap-3">
            {tagsWithEntries.map((tag) => (
              <Link
                key={tag.id}
                to={`/?tag=${encodeURIComponent(tag.label)}`}
                className="group relative inline-flex items-center gap-1.5 rounded-full bg-secondary px-4 py-2 text-sm font-medium text-secondary-foreground hover:bg-secondary/80 transition-colors"
              >
                {tag.label}
                <span className="text-xs text-muted-foreground">({tag.entry_count})</span>
                <Link
                  to="/settings"
                  className="absolute -top-1 -right-1 hidden group-hover:flex items-center justify-center w-5 h-5 rounded-full bg-background border border-border text-muted-foreground hover:text-foreground"
                  onClick={(e) => e.stopPropagation()}
                >
                  <Pencil size={10} />
                </Link>
              </Link>
            ))}
          </div>

          {tagsWithoutEntries.length > 0 && (
            <div className="mt-8">
              <h3 className="text-sm font-medium text-muted-foreground mb-3">无文章的标签</h3>
              <div className="flex flex-wrap gap-2">
                {tagsWithoutEntries.map((tag) => (
                  <Link
                    key={tag.id}
                    to="/settings"
                    className="inline-flex items-center rounded-full bg-muted px-3 py-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
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
