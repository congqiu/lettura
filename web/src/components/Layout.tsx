import { useState } from 'react';
import { Outlet, NavLink, useNavigate } from 'react-router-dom';
import { useAuthStore } from '../store/auth';
import { logout as apiLogout } from '../api/auth';
import ErrorBoundary from './ErrorBoundary';
import ThemeToggle from './ThemeToggle';
import NetworkStatus from './NetworkStatus';
import { MobileNavButton, MobileDrawer, DesktopNav } from './MobileNav';

export default function Layout() {
  const { logout } = useAuthStore();
  const navigate = useNavigate();
  const [drawerOpen, setDrawerOpen] = useState(false);

  const handleLogout = async () => {
    const refreshToken = localStorage.getItem('refresh_token');
    if (refreshToken) {
      try { await apiLogout(refreshToken); } catch {}
    }
    logout();
    navigate('/login');
  };

  const linkClass = ({ isActive }: { isActive: boolean }) =>
    `px-3 py-2 rounded text-sm transition-colors ${
      isActive
        ? 'bg-blue-100 dark:bg-blue-900/50 text-blue-700 dark:text-blue-300 font-medium'
        : 'text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800'
    }`;

  return (
    <div className="min-h-screen bg-gray-50 dark:bg-gray-950 text-gray-900 dark:text-gray-100 transition-colors">
      <NetworkStatus />
      <header className="bg-white dark:bg-gray-900 border-b border-gray-200 dark:border-gray-800 sticky top-0 z-10">
        <div className="max-w-6xl mx-auto px-4 h-14 flex items-center justify-between">
          <div className="flex items-center gap-1">
            <MobileNavButton onClick={() => setDrawerOpen(true)} />
            <span className="font-bold text-lg mr-4">Lettura</span>
            <DesktopNav />
          </div>
          <div className="flex items-center gap-2">
            <ThemeToggle />
            <NavLink to="/settings" className={`${linkClass} hidden md:inline-flex`}>设置</NavLink>
            <button onClick={handleLogout} className="hidden md:block px-3 py-2 text-sm text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800 rounded transition-colors">
              退出
            </button>
          </div>
        </div>
      </header>
      <MobileDrawer open={drawerOpen} onClose={() => setDrawerOpen(false)} />
      <main className="max-w-6xl mx-auto px-4 py-6">
        <ErrorBoundary level="page">
          <Outlet />
        </ErrorBoundary>
      </main>
    </div>
  );
}
