import { Link, NavLink, useNavigate, useSearchParams, useLocation } from 'react-router-dom';
import { BookOpen, Archive, Star, StickyNote, Globe, ShieldIcon, Settings, LogOut, Sun, Moon, Monitor, Tag as TagIcon, ChevronRight } from 'lucide-react';
import { useQuery } from '@tanstack/react-query';
import { useAuthStore } from '../../store/auth';
import { useThemeStore } from '../../store/theme';
import { logout as apiLogout } from '../../api/auth';
import { fetchTagStats } from '../../api/tags';
import {
  Sidebar, SidebarContent, SidebarFooter, SidebarGroup, SidebarGroupContent,
  SidebarHeader, SidebarMenu, SidebarMenuButton, SidebarMenuItem,
} from '@/components/ui/sidebar';
import { Separator } from '@/components/ui/separator';
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { cn } from '@/lib/utils';

const navItems = [
  { to: '/', label: '未读', icon: BookOpen, end: true },
  { to: '/archived', label: '归档', icon: Archive, end: false },
  { to: '/starred', label: '收藏', icon: Star, end: true },
  { to: '/memos', label: '便签', icon: StickyNote, end: false },
];

const toolItems = [
  { to: '/pages', label: 'Pages', icon: Globe, end: false },
  { to: '/audit-logs', label: '操作日志', icon: ShieldIcon, end: false },
];

export function AppSidebar() {
  const { logout } = useAuthStore();
  const navigate = useNavigate();
  const { theme, setTheme } = useThemeStore();
  const [searchParams] = useSearchParams();
  const currentTag = searchParams.get('tag') || '';
  const location = useLocation();
  const isActivePath = (to: string, end?: boolean) => {
    if (end) return location.pathname === to && !location.search;
    return location.pathname === to;
  };

  const { data: tagStats = [] } = useQuery({
    queryKey: ['tags', 'stats'],
    queryFn: fetchTagStats,
    staleTime: 5 * 60 * 1000,
  });

  const topTags = tagStats
    .filter((t) => t.entry_count > 0)
    .sort((a, b) => b.entry_count - a.entry_count)
    .slice(0, 10);

  const handleLogout = async () => {
    const refreshToken = localStorage.getItem('refresh_token');
    if (refreshToken) {
      try { await apiLogout(refreshToken); } catch {}
    }
    logout();
    navigate('/login');
  };

  const themeIcon = theme === 'dark' ? Moon : theme === 'light' ? Sun : Monitor;
  const ThemeIcon = themeIcon;

  return (
    <Sidebar className="border-r border-border/60">
      <SidebarHeader className="px-4 py-5">
        <Link to="/" className="flex items-center gap-2.5">
          <div className="w-8 h-8 rounded-xl bg-primary flex items-center justify-center shadow-sm shadow-primary/20">
            <span className="text-primary-foreground font-bold text-sm">L</span>
          </div>
          <span className="font-semibold text-lg text-foreground select-none tracking-tight">Lettura</span>
        </Link>
      </SidebarHeader>

      <SidebarContent className="px-2 overflow-x-hidden">
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu className="space-y-0.5">
              {navItems.map((item) => {
                const active = isActivePath(item.to, item.end);
                return (
                  <SidebarMenuItem key={item.to}>
                    <SidebarMenuButton asChild className="group/menu-button">
                      <NavLink
                        to={item.to}
                        end={item.end}
                        className={cn(
                          'flex items-center gap-2.5 rounded-lg px-3 py-2.5 text-sm font-medium transition-all duration-150',
                          active
                            ? 'bg-primary/15 text-primary shadow-sm hover:bg-primary/15 hover:text-primary'
                            : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'
                        )}
                      >
                        <item.icon size={17} className="shrink-0" />
                        <span>{item.label}</span>
                      </NavLink>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                );
              })}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        {topTags.length > 0 && (
          <SidebarGroup>
            <div className="px-3 mb-1.5 mt-4">
              <span className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground/70">
                常用标签
              </span>
            </div>
            <SidebarGroupContent>
              <SidebarMenu className="space-y-0.5">
                {topTags.map((tag) => (
                  <SidebarMenuItem key={tag.id}>
                    <SidebarMenuButton asChild>
                      <NavLink
                        to={`/?tag=${encodeURIComponent(tag.label)}`}
                        className={cn(
                          'flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm transition-all duration-150',
                          currentTag === tag.label
                            ? 'bg-primary/15 text-primary font-medium hover:bg-primary/15 hover:text-primary'
                            : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'
                        )}
                      >
                        <TagIcon size={14} className="shrink-0 opacity-60" />
                        <span className="truncate">{tag.label}</span>
                        <span className="ml-auto text-[11px] tabular-nums text-muted-foreground/60 bg-muted/50 px-1.5 py-0.5 rounded-full">
                          {tag.entry_count}
                        </span>
                      </NavLink>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                ))}
                {tagStats.filter((t) => t.entry_count > 0).length > 10 && (
                  <SidebarMenuItem>
                    <SidebarMenuButton asChild>
                      <NavLink
                        to="/settings"
                        className="flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-all duration-150"
                      >
                        <span className="ml-[22px]">查看全部标签</span>
                        <ChevronRight size={12} className="ml-auto opacity-50" />
                      </NavLink>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                )}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        )}

        <Separator className="mx-3 w-auto my-3 opacity-60" />

        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu className="space-y-0.5">
              {toolItems.map((item) => {
                const active = isActivePath(item.to, item.end);
                return (
                  <SidebarMenuItem key={item.to}>
                    <SidebarMenuButton asChild>
                      <NavLink
                        to={item.to}
                        end={item.end}
                        className={cn(
                          'flex items-center gap-2.5 rounded-lg px-3 py-2.5 text-sm font-medium transition-all duration-150',
                          active
                            ? 'bg-primary/15 text-primary hover:bg-primary/15 hover:text-primary'
                            : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'
                        )}
                      >
                        <item.icon size={17} className="shrink-0" />
                        <span>{item.label}</span>
                      </NavLink>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                );
              })}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>

      <SidebarFooter className="px-2 pb-4">
        <Separator className="mb-2 mx-2 w-auto opacity-60" />
        <SidebarMenu className="space-y-0.5">
          <SidebarMenuItem>
            <SidebarMenuButton asChild>
              <NavLink
                to="/settings"
                className={cn(
                  'flex items-center gap-2.5 rounded-lg px-3 py-2.5 text-sm font-medium transition-all duration-150',
                  isActivePath('/settings', false)
                    ? 'bg-primary/15 text-primary hover:bg-primary/15 hover:text-primary'
                    : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'
                )}
              >
                <Settings size={17} />
                <span>设置</span>
              </NavLink>
            </SidebarMenuButton>
          </SidebarMenuItem>
          <SidebarMenuItem>
            <DropdownMenu>
              <DropdownMenuTrigger className="flex w-full items-center gap-2.5 rounded-lg px-3 py-2.5 text-sm text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-all duration-150">
                <ThemeIcon size={17} />
                <span>主题</span>
                <span className="ml-auto text-[11px] text-muted-foreground/50">
                  {theme === 'dark' ? '深色' : theme === 'light' ? '浅色' : '跟随系统'}
                </span>
              </DropdownMenuTrigger>
              <DropdownMenuContent side="top" align="start" className="min-w-[140px]">
                <DropdownMenuItem onClick={() => setTheme('light')}>
                  <Sun size={15} className="mr-2" /> 浅色
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => setTheme('dark')}>
                  <Moon size={15} className="mr-2" /> 深色
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => setTheme('system')}>
                  <Monitor size={15} className="mr-2" /> 跟随系统
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </SidebarMenuItem>
          <SidebarMenuItem>
            <SidebarMenuButton
              onClick={handleLogout}
              className="flex items-center gap-2.5 rounded-lg px-3 py-2.5 text-sm text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-all duration-150"
            >
              <LogOut size={17} />
              <span>退出</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>
    </Sidebar>
  );
}
