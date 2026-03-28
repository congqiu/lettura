import { Link } from 'react-router-dom';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { updateEntry, type EntrySummary } from '../api/entries';

function timeAgo(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 60) return `${mins}分钟前`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}小时前`;
  const days = Math.floor(hrs / 24);
  return `${days}天前`;
}

export default function EntryCard({
  entry,
  selected = false,
  onDomainClick,
}: {
  entry: EntrySummary;
  selected?: boolean;
  onDomainClick?: (domain: string) => void;
}) {
  const qc = useQueryClient();

  const toggleStar = useMutation({
    mutationFn: () => updateEntry(entry.id, { is_starred: !entry.is_starred }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['entries'] }),
  });

  const toggleArchive = useMutation({
    mutationFn: () => updateEntry(entry.id, { is_archived: !entry.is_archived }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['entries'] }),
  });

  return (
    <div className={`bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-lg p-4 hover:shadow-sm dark:hover:shadow-gray-900/50 transition-all ${selected ? 'ring-2 ring-blue-500' : ''}`}>
      <div className="flex items-start justify-between gap-3">
        <div className="flex-1 min-w-0">
          <Link to={`/entry/${entry.id}`} className="block">
            <h3 className="font-medium text-gray-900 dark:text-gray-100 truncate hover:text-blue-600 dark:hover:text-blue-400">
              {entry.title || entry.url}
            </h3>
          </Link>
          <div className="flex items-center gap-2 mt-1 text-sm text-gray-500 dark:text-gray-500 flex-wrap">
            {entry.domain_name && (
              <button
                onClick={() => onDomainClick?.(entry.domain_name!)}
                className="hover:text-blue-600 dark:hover:text-blue-400 hover:underline"
                title={`查看 ${entry.domain_name} 的所有文章`}
              >
                {entry.domain_name}
              </button>
            )}
            {entry.reading_time && <span>{entry.reading_time} 分钟</span>}
            <span>{timeAgo(entry.created_at)}</span>
            {entry.extract_method === 'pending' && (
              <span className="text-yellow-600 dark:text-yellow-500">抓取中...</span>
            )}
            {entry.extract_method === 'failed' && (
              <span className="text-red-500">提取失败</span>
            )}
          </div>
        </div>
        <div className="flex flex-col items-end gap-2 flex-shrink-0">
          {entry.preview_picture && (
            <img src={entry.preview_picture} alt="" className="w-16 h-16 object-cover rounded" />
          )}
          <a
            href={entry.url}
            target="_blank"
            rel="noopener noreferrer"
            className="text-xs text-gray-400 dark:text-gray-600 hover:text-blue-500 dark:hover:text-blue-400"
            title="访问原始网页"
          >
            原文 ↗
          </a>
        </div>
      </div>
      <div className="flex gap-2 mt-3">
        <button
          onClick={() => toggleStar.mutate()}
          className={`text-sm px-2 py-1 rounded transition-colors ${entry.is_starred ? 'bg-yellow-100 dark:bg-yellow-900/30 text-yellow-700 dark:text-yellow-400' : 'text-gray-500 dark:text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-800'}`}
        >
          {entry.is_starred ? '已收藏' : '收藏'}
        </button>
        <button
          onClick={() => toggleArchive.mutate()}
          className={`text-sm px-2 py-1 rounded transition-colors ${entry.is_archived ? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400' : 'text-gray-500 dark:text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-800'}`}
        >
          {entry.is_archived ? '已归档' : '归档'}
        </button>
      </div>
    </div>
  );
}
