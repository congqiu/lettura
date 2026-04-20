import { NavLink } from 'react-router-dom';
import { BookOpen, Archive, Star, StickyNote, MoreHorizontal } from 'lucide-react';
import { Sheet, SheetContent, SheetTrigger } from '@/components/ui/sheet';
import { Separator } from '@/components/ui/separator';
import { useAuthStore } from '../../store/auth';
import { useNavigate } from 'react-router-dom';
import { logout as apiLogout } from '../../api/auth';
import { useState } from 'react';

const bottomNavItems = [
  { to: '/', label: '未读', icon: BookOpen, end: true },
  { to: '/archived', label: '归档', icon: Archive, end: false },
  { to: '/starred', label: '收藏', icon: Star, end: false },
  { to: '/memos', label: '便签', icon: StickyNote, end: false },
];

export function MobileBottomNav() {
  const [sheetOpen, setSheetOpen] = useState(false);
  const { logout } = useAuthStore();
  const navigate = useNavigate();

  const handleLogout = async () => {
    const refreshToken = localStorage.getItem('refresh_token');
    if (refreshToken) {
      try { await apiLogout(refreshToken); } catch {}
    }
    logout();
    navigate('/login');
  };

  return (
    <div className="fixed bottom-0 left-0 right-0 z-40 border-t border-border bg-card lg:hidden">
      <div className="flex items-center justify-around px-2 py-1 pb-[env(safe-area-inset-bottom)]">
        {bottomNavItems.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.end}
            className={({ isActive }) =>
              `flex flex-col items-center gap-0.5 px-3 py-1.5 text-xs transition-colors ${
                isActive ? 'text-primary font-medium' : 'text-muted-foreground'
              }`
            }
          >
            <item.icon size={20} />
            <span>{item.label}</span>
          </NavLink>
        ))}

        <Sheet open={sheetOpen} onOpenChange={setSheetOpen}>
          <SheetTrigger asChild>
            <button className="flex flex-col items-center gap-0.5 px-3 py-1.5 text-xs text-muted-foreground">
              <MoreHorizontal size={20} />
              <span>更多</span>
            </button>
          </SheetTrigger>
          <SheetContent side="bottom" className="rounded-t-2xl">
            <div className="space-y-1 py-2">
              <NavLink
                to="/pages"
                onClick={() => setSheetOpen(false)}
                className="block px-4 py-3 text-sm rounded-lg hover:bg-accent"
              >
                Pages
              </NavLink>
              <NavLink
                to="/settings"
                onClick={() => setSheetOpen(false)}
                className="block px-4 py-3 text-sm rounded-lg hover:bg-accent"
              >
                设置
              </NavLink>
              <Separator className="my-2" />
              <button
                onClick={() => { setSheetOpen(false); handleLogout(); }}
                className="block w-full text-left px-4 py-3 text-sm rounded-lg hover:bg-accent text-destructive"
              >
                退出登录
              </button>
            </div>
          </SheetContent>
        </Sheet>
      </div>
    </div>
  );
}