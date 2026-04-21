import { Link } from 'react-router-dom';
import { Star, Archive, ExternalLink, Clock } from 'lucide-react';
import { type EntrySummary } from '../api/entries';
import { timeAgo } from '../utils/time';
import { useEntryActions } from '../hooks/useEntryActions';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

export default function EntryCard({
  entry,
  selected = false,
  onDomainClick,
}: {
  entry: EntrySummary;
  selected?: boolean;
  onDomainClick?: (domain: string) => void;
}) {
  const { toggleStar, toggleArchive } = useEntryActions(entry.id, entry);

  return (
    <div className={cn(
      'group bg-card border border-border rounded-xl p-5',
      selected ? 'ring-2 ring-primary shadow-md shadow-primary/10' : ''
    )}>
      <div className="flex flex-col sm:flex-row items-start justify-between gap-4">
        <div className="flex-1 min-w-0 flex flex-col gap-2">
          <Link to={`/entry/${entry.id}`} className="block">
            <h3 className="text-lg font-semibold text-card-foreground leading-snug line-clamp-2 hover:text-primary transition-colors break-all">
              {entry.title || entry.url}
            </h3>
          </Link>
          
          <div className="flex items-center gap-3 text-sm text-muted-foreground flex-wrap">
            {entry.domain_name && (
              <button
                onClick={() => onDomainClick?.(entry.domain_name!)}
                className="font-medium hover:text-card-foreground transition-colors"
                title={`查看 ${entry.domain_name} 的所有文章`}
              >
                {entry.domain_name}
              </button>
            )}
            <span className="w-1 h-1 rounded-full bg-border"></span>
            <span>{timeAgo(entry.created_at)}</span>
            
            {entry.reading_time && (
              <>
                <span className="w-1 h-1 rounded-full bg-border"></span>
                <span className="flex items-center gap-1">
                  <Clock size={14} />
                  {entry.reading_time} 分钟
                </span>
              </>
            )}
            
            {entry.extract_method === 'pending' && (
              <>
                <span className="w-1 h-1 rounded-full bg-border"></span>
                <span className="text-amber-600 dark:text-amber-400 font-medium animate-pulse">抓取中...</span>
              </>
            )}
            {entry.extract_method === 'failed' && (
              <>
                <span className="w-1 h-1 rounded-full bg-border"></span>
                <span className="text-destructive font-medium">提取失败</span>
              </>
            )}
          </div>
          
          <div className="flex items-center gap-2 mt-2">
            <Button
              variant="ghost"
              size="sm"
              onClick={() => toggleStar.mutate()}
              className={entry.is_starred ? 'bg-amber-50 dark:bg-amber-900/20 text-amber-500 hover:text-amber-600' : 'text-muted-foreground hover:text-card-foreground'}
            >
              <Star size={18} className={entry.is_starred ? 'fill-current' : ''} />
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => toggleArchive.mutate()}
              className={entry.is_archived ? 'bg-green-50 dark:bg-green-900/20 text-green-600 dark:text-green-400' : 'text-muted-foreground hover:text-card-foreground'}
            >
              <Archive size={18} className={entry.is_archived ? 'fill-current' : ''} />
            </Button>
          </div>
        </div>

        <div className="flex sm:flex-col items-center sm:items-end gap-4 w-full sm:w-auto">
          {entry.preview_picture && (
            <div className="w-24 h-16 sm:w-28 sm:h-20 shrink-0 rounded-lg overflow-hidden border border-border">
              <img src={entry.preview_picture} alt="" className="w-full h-full object-cover transition-transform duration-500 group-hover:scale-105" />
            </div>
          )}
          <a
            href={entry.url}
            target="_blank"
            rel="noopener noreferrer"
            className="ml-auto sm:ml-0 flex items-center gap-1 text-xs font-medium text-muted-foreground hover:text-primary transition-colors"
            title="访问原始网页"
          >
            原文 <ExternalLink size={12} />
          </a>
        </div>
      </div>
    </div>
  );
}