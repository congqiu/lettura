import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { listEntries, type ListParams } from '../api/entries';
import EntryCard from '../components/EntryCard';
import AddEntryForm from '../components/AddEntryForm';
import { useListKeyboardNav } from '../hooks/useKeyboardShortcuts';

interface Props {
  filter?: 'unread' | 'archived' | 'starred';
}

const TITLES = { unread: '未读', archived: '归档', starred: '收藏' };

export default function EntryListPage({ filter }: Props) {
  const [search, setSearch] = useState('');
  const [domain, setDomain] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);

  const params: ListParams = {};
  if (filter === 'archived') params.is_archived = true;
  if (filter === 'starred') params.is_starred = true;
  if (filter === 'unread') params.is_archived = false;
  if (search) params.search = search;
  if (domain) params.domain = domain;

  const { data: entries = [], isLoading } = useQuery({
    queryKey: ['entries', filter, search, domain],
    queryFn: () => listEntries(params),
  });

  useListKeyboardNav(entries, selectedIndex, setSelectedIndex);

  const title = TITLES[filter || 'unread'];

  return (
    <div>
      <div className="flex items-center justify-between mb-4 flex-wrap gap-2">
        <div className="flex items-center gap-2">
          <h2 className="text-xl font-semibold">{title}</h2>
          {domain && (
            <span className="text-sm bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 px-2 py-0.5 rounded flex items-center gap-1">
              {domain}
              <button onClick={() => setDomain('')} className="hover:text-red-500 font-bold">&times;</button>
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          <input
            type="text"
            placeholder="搜索..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="px-3 py-1.5 border border-gray-200 dark:border-gray-700 rounded text-sm w-64 bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-600 focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
          <span className="text-xs text-gray-400 dark:text-gray-600 hidden sm:inline">j/k 导航</span>
        </div>
      </div>

      {!filter || filter === 'unread' ? <AddEntryForm /> : null}

      {isLoading ? (
        <p className="text-gray-500">加载中...</p>
      ) : entries.length === 0 ? (
        <p className="text-gray-500">暂无文章</p>
      ) : (
        <div className="space-y-3">
          {entries.map((entry, i) => (
            <EntryCard
              key={entry.id}
              entry={entry}
              selected={i === selectedIndex}
              onDomainClick={(d) => setDomain(d)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
