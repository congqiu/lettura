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
        <span className="inline-flex items-center gap-1 text-xs font-medium text-warning">
          <span className="w-1.5 h-1.5 rounded-full bg-warning animate-pulse" />
          抓取中
        </span>
      );
    }
    if (entry.extract_method === 'failed') {
      return (
        <span className="inline-flex items-center gap-1 text-xs font-medium text-destructive">
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
        selected ? 'ring-2 ring-primary/30 shadow-md shadow-primary/5' : 'border-border/70',
        selectionMode ? 'flex flex-row items-start' : ''
      )}
    >
      {selectionMode && (
        <button
          onClick={(e) => { e.preventDefault(); onToggleSelect?.(); }}
          className="shrink-0 text-muted-foreground hover:text-foreground transition-colors mr-0 p-5"
        >
          {entrySelected ? (
            <CheckSquare size={20} className="text-primary" />
          ) : (
            <Square size={20} />
          )}
        </button>
      )}

      <div className="flex flex-col sm:flex-row items-start gap-4 p-5 w-full">
        {/* Thumbnail - mobile top, desktop right */}
        {entry.preview_picture && (
          <div className="order-first sm:order-last w-full sm:w-auto shrink-0">
            <Link
              to={`/entry/${entry.id}`}
              state={entryIds ? { entryIds, currentIndex: entryIndex } : undefined}
              className="block"
            >
              <div className="w-full h-40 sm:w-28 sm:h-20 rounded-lg overflow-hidden border border-border/50 bg-muted">
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

        <div className="flex-1 min-w-0 flex flex-col gap-2.5 order-last sm:order-first">
          {/* Title */}
          <Link
            to={`/entry/${entry.id}`}
            state={entryIds ? { entryIds, currentIndex: entryIndex } : undefined}
            className="block"
          >
            <h3 className="text-[17px] font-semibold text-card-foreground leading-snug line-clamp-2 group-hover:text-primary transition-colors break-words">
              {entry.title || entry.url}
            </h3>
          </Link>

          {/* Meta info */}
          <div className="flex items-center flex-wrap gap-x-2.5 gap-y-1 text-[13px] text-muted-foreground">
            {entry.domain_name && (
              <>
                <button
                  onClick={() => onDomainClick?.(entry.domain_name!)}
                  className="font-medium text-foreground/70 hover:text-primary transition-colors"
                  title={`查看 ${entry.domain_name} 的所有文章`}
                >
                  {entry.domain_name}
                </button>
                <span className="text-border">·</span>
              </>
            )}

            <span>{timeAgo(entry.created_at)}</span>

            {entry.reading_time && (
              <>
                <span className="text-border">·</span>
                <span className="flex items-center gap-1">
                  <Clock size={12} />
                  {entry.reading_time} 分钟
                </span>
              </>
            )}

            {statusBadge() && (
              <>
                <span className="text-border">·</span>
                {statusBadge()}
              </>
            )}
          </div>

          {/* Tags */}
          {entry.tags && entry.tags.length > 0 && (
            <div className="flex flex-wrap gap-1.5 mt-0.5">
              {entry.tags.slice(0, 4).map((tag) => (
                <TagBadge key={tag.id} label={tag.label} />
              ))}
              {entry.tags.length > 4 && (
                <span className="text-[11px] text-muted-foreground/70 px-1.5 py-0.5">
                  +{entry.tags.length - 4}
                </span>
              )}
            </div>
          )}

          {/* Actions */}
          <div className="flex items-center gap-1 mt-1">
            <Button
              variant="ghost"
              size="sm"
              onClick={() => toggleStar.mutate()}
              className={cn(
                'h-8 w-8 p-0 rounded-lg transition-colors',
                entry.is_starred
                  ? 'text-amber-500 hover:text-amber-600 hover:bg-amber-500/10'
                  : 'text-muted-foreground hover:text-foreground hover:bg-accent'
              )}
              title={entry.is_starred ? '取消收藏' : '收藏'}
            >
              <Star size={16} className={cn(entry.is_starred && 'fill-current')} />
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => toggleArchive.mutate()}
              className={cn(
                'h-8 w-8 p-0 rounded-lg transition-colors',
                entry.is_archived
                  ? 'text-success hover:text-success hover:bg-success/10'
                  : 'text-muted-foreground hover:text-foreground hover:bg-accent'
              )}
              title={entry.is_archived ? '取消归档' : '归档'}
            >
              <Archive size={16} className={cn(entry.is_archived && 'fill-current')} />
            </Button>
            <div className="flex-1" />
            <a
              href={entry.url}
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-1 text-[13px] text-muted-foreground hover:text-primary transition-colors px-2 py-1 rounded-lg hover:bg-accent"
              title="访问原始网页"
              onClick={(e) => e.stopPropagation()}
            >
              原文
              <ExternalLink size={12} />
            </a>
          </div>
        </div>
      </div>
    </div>
  );
}
