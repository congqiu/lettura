import { useState } from 'react';
import { Tag, ListChecks, Database, KeyRound, Settings } from 'lucide-react';
import TagsPanel from '@/components/settings/TagsPanel';
import RulesPanel from '@/components/settings/RulesPanel';
import DataPanel from '@/components/settings/DataPanel';
import TokensPanel from '@/components/settings/TokensPanel';
import { cn } from '@/lib/utils';

type PanelKey = 'tags' | 'rules' | 'data' | 'tokens';

const NAV_ITEMS: { key: PanelKey; label: string; icon: React.ComponentType<{ size?: number; className?: string }> }[] = [
  { key: 'tags', label: '标签管理', icon: Tag },
  { key: 'rules', label: '标签规则', icon: ListChecks },
  { key: 'data', label: '数据管理', icon: Database },
  { key: 'tokens', label: 'API 令牌', icon: KeyRound },
];

const PANELS: Record<PanelKey, React.ComponentType> = {
  tags: TagsPanel,
  rules: RulesPanel,
  data: DataPanel,
  tokens: TokensPanel,
};

export default function SettingsPage() {
  const [active, setActive] = useState<PanelKey>('tags');
  const ActivePanel = PANELS[active];

  return (
    <div className="animate-fade-in">
      {/* Page header */}
      <div className="flex items-center gap-2.5 mb-6">
        <div className="w-9 h-9 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
          <Settings size={18} />
        </div>
        <h2 className="text-xl font-bold tracking-tight text-foreground">设置</h2>
      </div>

      {/* Desktop: sidebar + content */}
      <div className="flex flex-col lg:flex-row gap-6 lg:gap-8">
        {/* Navigation */}
        <nav className="lg:w-48 shrink-0">
          {/* Mobile: horizontal scroll tabs */}
          <div className="flex lg:hidden gap-1 overflow-x-auto scrollbar-hide bg-muted/40 rounded-xl p-1.5 -mx-1">
            {NAV_ITEMS.map((item) => {
              const Icon = item.icon;
              const isActive = active === item.key;
              return (
                <button
                  key={item.key}
                  onClick={() => setActive(item.key)}
                  className={cn(
                    'flex items-center gap-1.5 shrink-0 px-3 py-2 rounded-lg text-sm font-medium transition-colors',
                    isActive
                      ? 'bg-background text-primary shadow-sm'
                      : 'text-muted-foreground hover:text-foreground hover:bg-background/60'
                  )}
                >
                  <Icon size={15} />
                  {item.label}
                </button>
              );
            })}
          </div>

          {/* Desktop: vertical sidebar */}
          <div className="hidden lg:flex flex-col gap-0.5 sticky top-4">
            {NAV_ITEMS.map((item) => {
              const Icon = item.icon;
              const isActive = active === item.key;
              return (
                <button
                  key={item.key}
                  onClick={() => setActive(item.key)}
                  className={cn(
                    'flex items-center gap-2.5 w-full px-3 py-2 rounded-lg text-sm font-medium transition-colors text-left',
                    isActive
                      ? 'bg-primary/10 text-primary'
                      : 'text-muted-foreground hover:text-foreground hover:bg-muted/50'
                  )}
                >
                  <Icon size={16} />
                  {item.label}
                </button>
              );
            })}
          </div>
        </nav>

        {/* Content area */}
        <div className="flex-1 min-w-0">
          <ActivePanel />
        </div>
      </div>
    </div>
  );
}
