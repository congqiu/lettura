import { Link } from 'react-router-dom';
import { Star, Archive, ExternalLink, Clock, CheckSquare, Square } from 'lucide-react';
import { type EntrySummary } from '../api/entries';
import { timeAgo } from '../utils/time';
import { useEntryActions } from '../hooks/useEntryActions';
import TagBadge from '@/components/TagBadge';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

export default function EntryCard({
  entry,
  selected = false,
  onDomainClick,
  selectionMode,
  entrySelected,
  onToggleSelect,
  entryIndex,
  entryIds,
}: {
  entry: EntrySummary;
  selected?: boolean;
  onDomainClick?: (domain: string) => void;
  selectionMode?: boolean;
  entrySelected?: boolean;
  onToggleSelect?: () => void;
  entryIndex?: number;
  entryIds?: string[];
}) {
  const { toggleStar, toggleArchive } = useEntryActions(entry.id, entry);

  const statusBadge = () => {
    if (entry.extract_method === 'pending') {
      return (
        <span className="inline-flex items-center gap-1 text-label font-medium text-warning">
          <span className="w-1.5 h-1.5 rounded-full bg-warning animate-pulse-soft" />
          抓取中
        </span>
      );
    }
    if (entry.extract_method === 'failed') {
      return (
        <span className="inline-flex items-center gap-1 text-label font-medium text-destructive">
          <span className="w-1.5 h-1.5 rounded-full bg-destructive" />
          提取失败
        </span>
      );
    }
    return null;
  };

  return (
    <div
      className={cn(
        'group bg-card border rounded-xl overflow-hidden card-hover',
        selected ? 'ring-2 ring-primary/30 shadow-md shadow-primary/5' : 'border-border/60',
        selectionMode ? 'flex flex-row items-start' : ''
      )}
    >
      {selectionMode && (
        <button
          onClick={(e) => { e.preventDefault(); onToggleSelect?.(); }}
          className="shrink-0 text-muted-foreground hover:text-foreground transition-colors p-4 sm:p-5"
        >
          {entrySelected ? (
            <CheckSquare size={20} className="text-primary" />
          ) : (
            <Square size={20} />
          )}
        </button>
      )}

      <div className="flex flex-row items-start gap-3 sm:gap-4 p-4 sm:p-5 w-full">
        {/* Thumbnail - hidden on mobile, right side on desktop */}
        {entry.preview_picture && (
          <div className="hidden sm:block order-last shrink-0">
            <Link
              to={`/entry/${entry.id}`}
              state={entryIds ? { entryIds, currentIndex: entryIndex } : undefined}
              className="block"
            >
              <div className="w-24 h-16 rounded-lg overflow-hidden border border-border/40 bg-muted">
                <img
                  src={entry.preview_picture}
                  alt=""
                  className="w-full h-full object-cover transition-transform duration-500 group-hover:scale-105"
                  loading="lazy"
                />
              </div>
            </Link>
          </div>
        )}

        <div className="flex-1 min-w-0 flex flex-col gap-2">
          {/* Title */}
          <Link
            to={`/entry/${entry.id}`}
            state={entryIds ? { entryIds, currentIndex: entryIndex } : undefined}
            className="block"
          >
            <h3 className="text-title text-card-foreground line-clamp-2 group-hover:text-primary transition-colors break-words">
              {entry.title || entry.url}
            </h3>
          </Link>

          {/* Meta info */}
          <div className="flex items-center flex-wrap gap-x-3 gap-y-1 text-caption text-muted-foreground/80">
            {entry.domain_name && (
              <button
                onClick={() => onDomainClick?.(entry.domain_name!)}
                className="font-medium text-foreground/60 hover:text-primary transition-colors"
                title={`查看 ${entry.domain_name} 的所有文章`}
              >
                {entry.domain_name}
              </button>
            )}

            <span>{timeAgo(entry.created_at)}</span>

            {entry.reading_time && (
              <span className="inline-flex items-center gap-1">
                <Clock size={11} />
                {entry.reading_time} 分钟
              </span>
            )}

            {statusBadge()}
          </div>

          {/* Tags + Actions row */}
          <div className="flex items-end justify-between gap-3 mt-0.5">
            {/* Tags */}
            {entry.tags && entry.tags.length > 0 && (
              <div className="flex flex-wrap gap-1.5 min-w-0">
                {entry.tags.slice(0, 3).map((tag) => (
                  <TagBadge key={tag.id} label={tag.label} />
                ))}
                {entry.tags.length > 3 && (
                  <span className="text-label text-muted-foreground/60 px-1 py-0.5">
                    +{entry.tags.length - 3}
                  </span>
                )}
              </div>
            )}

            {/* Actions */}
            <div className="flex items-center gap-0.5 shrink-0">
              <Button
                variant="ghost"
                size="icon"
                onClick={() => toggleStar.mutate()}
                className={cn(
                  'h-7 w-7 rounded-md transition-colors',
                  entry.is_starred
                    ? 'text-amber-500 hover:text-amber-600 hover:bg-amber-500/10'
                    : 'text-muted-foreground/60 hover:text-foreground hover:bg-accent'
                )}
                title={entry.is_starred ? '取消收藏' : '收藏'}
              >
                <Star size={15} className={cn(entry.is_starred && 'fill-current')} />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => toggleArchive.mutate()}
                className={cn(
                  'h-7 w-7 rounded-md transition-colors',
                  entry.is_archived
                    ? 'text-success hover:text-success hover:bg-success/10'
                    : 'text-muted-foreground/60 hover:text-foreground hover:bg-accent'
                )}
                title={entry.is_archived ? '取消归档' : '归档'}
              >
                <Archive size={15} className={cn(entry.is_archived && 'fill-current')} />
              </Button>
              <a
                href={entry.url}
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex items-center justify-center h-7 w-7 rounded-md text-muted-foreground/60 hover:text-foreground hover:bg-accent transition-colors"
                title="访问原始网页"
                onClick={(e) => e.stopPropagation()}
              >
                <ExternalLink size={15} />
              </a>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
