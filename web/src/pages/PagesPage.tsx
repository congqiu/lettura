import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { listPages, type PageSummary } from '../api/pages';
import PageCard from '../components/PageCard';
import PageUploadModal from '../components/PageUploadModal';
import ErrorState from '../components/ErrorState';
import EmptyState from '../components/EmptyState';
import { Plus, Loader2 } from 'lucide-react';
import { Button } from '../components/ui/button';

const TABS = [
  { key: 'all', label: '全部' },
  { key: 'active', label: '活跃' },
  { key: 'expired', label: '已过期' },
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
        <div className="flex gap-1 overflow-x-auto flex-nowrap scrollbar-hide">
          {TABS.map(t => (
            <button
              key={t.key}
              onClick={() => setTab(t.key)}
              className={`px-3 py-1.5 rounded-md text-sm transition-colors ${
                tab === t.key
                  ? 'bg-primary/10 text-primary font-medium'
                  : 'text-muted-foreground hover:bg-muted'
              }`}
            >
              {t.label}
            </button>
          ))}
        </div>
        <Button onClick={() => setUploadOpen(true)}>
          <Plus size={15} />
          上传
        </Button>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12">
          <Loader2 size={24} className="animate-spin text-muted-foreground" />
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
