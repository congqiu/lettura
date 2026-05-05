import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { listPages, type PageSummary } from '../api/pages';
import PageCard from '../components/PageCard';
import PageUploadModal from '../components/PageUploadModal';
import ErrorState from '../components/ErrorState';
import EmptyState from '../components/EmptyState';
import { Plus, Loader2, Globe } from 'lucide-react';
import { Button } from '../components/ui/button';
import { cn } from '@/lib/utils';

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
    <div className="animate-fade-in">
      <div className="flex items-center justify-between mb-5">
        <div className="flex items-center gap-2.5">
          <div className="w-9 h-9 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
            <Globe size={18} />
          </div>
          <div>
            <h2 className="text-xl font-bold tracking-tight text-foreground">Pages</h2>
            <p className="text-xs text-muted-foreground">分享你的 HTML 页面</p>
          </div>
        </div>
        <Button onClick={() => setUploadOpen(true)} className="rounded-lg h-9 gap-1.5">
          <Plus size={15} />
          上传
        </Button>
      </div>

      <div className="flex gap-1 overflow-x-auto flex-nowrap scrollbar-hide mb-5 pb-1">
        {TABS.map(t => (
          <button
            key={t.key}
            onClick={() => setTab(t.key)}
            className={cn(
              'px-3.5 py-1.5 rounded-lg text-sm font-medium transition-all duration-150 shrink-0',
              tab === t.key
                ? 'bg-primary/10 text-primary'
                : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'
            )}
          >
            {t.label}
          </button>
        ))}
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12">
          <Loader2 size={24} className="animate-spin text-muted-foreground/50" />
        </div>
      ) : error ? (
        <ErrorState onRetry={() => refetch()} />
      ) : data && data.items.length > 0 ? (
        <div className="space-y-3 stagger-children">
          {data.items.map((page: PageSummary) => (
            <PageCard key={page.id} page={page} />
          ))}
        </div>
      ) : (
        <EmptyState icon="file" title="暂无页面" description="上传 HTML 文件创建可分享的页面" />
      )}

      <PageUploadModal open={uploadOpen} onClose={() => setUploadOpen(false)} />
    </div>
  );
}
