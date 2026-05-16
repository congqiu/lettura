import { NavLink, useLocation } from 'react-router-dom';
import { BookOpen, Star, StickyNote, Globe, MoreHorizontal, Archive } from 'lucide-react';
import { Sheet, SheetContent, SheetTrigger } from '@/components/ui/sheet';
import { Separator } from '@/components/ui/separator';
import { useAuthStore } from '../../store/auth';
import { logout as apiLogout } from '../../api/auth';
import { useNavigate } from 'react-router-dom';
import { useState, useLayoutEffect, useRef } from 'react';
import { cn } from '@/lib/utils';

const bottomNavItems = [
  { to: '/', label: '未读', icon: BookOpen, end: true },
  { to: '/starred', label: '收藏', icon: Star, end: true },
  { to: '/archived', label: '归档', icon: Archive, end: true },
  { to: '/memos', label: '便签', icon: StickyNote, end: false },
];

export function MobileBottomNav() {
  const [sheetOpen, setSheetOpen] = useState(false);
  const { logout } = useAuthStore();
  const navigate = useNavigate();
  const navRef = useRef<HTMLDivElement>(null);
  const location = useLocation();

  useLayoutEffect(() => {
    const el = navRef.current;
    if (el) {
      document.documentElement.style.setProperty('--bottom-nav-height', `${el.offsetHeight}px`);
    }
  }, []);

  const handleLogout = async () => {
    const refreshToken = localStorage.getItem('refresh_token');
    if (refreshToken) {
      try { await apiLogout(refreshToken); } catch { /* ignore logout failure */ }
    }
    logout();
    navigate('/login');
  };

  const moreItems = [
    { to: '/pages', label: 'Pages', icon: Globe },
    { to: '/tags', label: '标签', icon: Star },
    { to: '/audit-logs', label: '操作日志', icon: Archive },
  ];

  const isActiveRoute = (to: string, end?: boolean) => {
    if (end) return location.pathname === to;
    return location.pathname.startsWith(to);
  };

  return (
    <div
      ref={navRef}
      className="fixed bottom-0 inset-x-0 z-40 border-t border-border/60 bg-background/85 backdrop-blur-xl lg:hidden"
    >
      <div className="flex items-center justify-around px-2 py-1.5 pb-[env(safe-area-inset-bottom)] box-border">
        {bottomNavItems.map((item) => {
          const active = isActiveRoute(item.to, item.end);
          return (
            <NavLink
              key={item.to}
              to={item.to}
              end={item.end}
              className={cn(
                'flex flex-col items-center gap-0.5 px-3 py-1.5 text-[11px] transition-all duration-200 shrink-0 rounded-xl min-w-[56px]',
                active
                  ? 'text-primary font-medium'
                  : 'text-muted-foreground'
              )}
            >
              <div className={cn(
                'p-1 rounded-lg transition-all duration-200',
                active && 'bg-primary/10'
              )}>
                <item.icon size={20} strokeWidth={active ? 2.2 : 1.8} />
              </div>
              <span>{item.label}</span>
            </NavLink>
          );
        })}

        <Sheet open={sheetOpen} onOpenChange={setSheetOpen}>
          <SheetTrigger asChild>
            <button className="flex flex-col items-center gap-0.5 px-3 py-1.5 text-[11px] text-muted-foreground shrink-0 rounded-xl min-w-[56px] transition-colors active:text-foreground">
              <div className="p-1 rounded-lg">
                <MoreHorizontal size={20} strokeWidth={1.8} />
              </div>
              <span>更多</span>
            </button>
          </SheetTrigger>
          <SheetContent side="bottom" className="rounded-t-2xl border-t border-border/60 px-0 pb-[env(safe-area-inset-bottom)]">
            <div className="px-4 pt-2 pb-1">
              <div className="mx-auto w-10 h-1 rounded-full bg-border mb-4" />
            </div>
            <div className="space-y-0.5 px-2">
              {moreItems.map((item) => (
                <NavLink
                  key={item.to}
                  to={item.to}
                  onClick={() => setSheetOpen(false)}
                  className={({ isActive }) => cn(
                    'flex items-center gap-3 px-4 py-3 text-sm rounded-xl transition-colors',
                    isActive
                      ? 'bg-primary/10 text-primary font-medium'
                      : 'text-foreground hover:bg-accent'
                  )}
                >
                  <item.icon size={18} className="opacity-60" />
                  {item.label}
                </NavLink>
              ))}
              <Separator className="my-2" />
              <NavLink
                to="/settings"
                onClick={() => setSheetOpen(false)}
                className="flex items-center gap-3 px-4 py-3 text-sm rounded-xl text-foreground hover:bg-accent transition-colors"
              >
                <SettingsIcon size={18} className="opacity-60" />
                设置
              </NavLink>
              <button
                onClick={() => { setSheetOpen(false); handleLogout(); }}
                className="flex items-center gap-3 w-full text-left px-4 py-3 text-sm rounded-xl hover:bg-accent text-destructive transition-colors"
              >
                <LogOutIcon size={18} />
                退出登录
              </button>
            </div>
          </SheetContent>
        </Sheet>
      </div>
    </div>
  );
}

function SettingsIcon({ size, className }: { size?: number; className?: string }) {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className={className}>
      <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
      <circle cx="12" cy="12" r="3" />
    </svg>
  );
}

function LogOutIcon({ size, className }: { size?: number; className?: string }) {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className={className}>
      <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
      <polyline points="16 17 21 12 16 7" />
      <line x1="21" x2="9" y1="12" y2="12" />
    </svg>
  );
}
