import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { listPages, type PageSummary } from '../api/pages';
import PageCard from '../components/PageCard';
import PageUploadModal from '../components/PageUploadModal';
import ErrorState from '../components/ErrorState';
import EmptyState from '../components/EmptyState';
import { Plus, Loader2 } from 'lucide-react';

const TABS = [
  { key: 'active', label: '活跃' },
  { key: 'disabled', label: '已禁用' },
  { key: 'deleted', label: '已删除' },
] as const;

export default function PagesPage() {
  const [tab, setTab] = useState<string>('active');
  const [uploadOpen, setUploadOpen] = useState(false);

  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['pages', tab],
    queryFn: () => listPages({ status: tab }),
  });

  return (
    <>
      <div className="flex items-center justify-between mb-4">
        <div className="flex gap-1">
          {TABS.map(t => (
            <button
              key={t.key}
              onClick={() => setTab(t.key)}
              className={`px-3 py-1.5 rounded-md text-sm transition-colors ${
                tab === t.key
                  ? 'bg-blue-100 dark:bg-blue-900/50 text-blue-700 dark:text-blue-300 font-medium'
                  : 'text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800'
              }`}
            >
              {t.label}
            </button>
          ))}
        </div>
        <button
          onClick={() => setUploadOpen(true)}
          className="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 text-white rounded-lg text-sm font-medium hover:bg-blue-700 transition-colors"
        >
          <Plus size={15} />
          上传
        </button>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12">
          <Loader2 size={24} className="animate-spin text-gray-400" />
        </div>
      ) : error ? (
        <ErrorState onRetry={() => refetch()} />
      ) : data && data.items.length > 0 ? (
        <div className="space-y-3">
          {data.items.map((page: PageSummary) => (
            <PageCard key={page.id} page={page} />
          ))}
        </div>
      ) : (
        <EmptyState icon="file" title="暂无页面" description="上传 HTML 文件创建可分享的页面" />
      )}

      <PageUploadModal open={uploadOpen} onClose={() => setUploadOpen(false)} />
    </>
  );
}
