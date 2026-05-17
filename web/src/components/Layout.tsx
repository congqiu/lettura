import { Link, Outlet, useLocation } from 'react-router-dom';
import { SidebarProvider, SidebarInset } from '@/components/ui/sidebar';
import { AppSidebar } from './layout/AppSidebar';
import { MobileBottomNav } from './layout/MobileBottomNav';
import ErrorBoundary from './ErrorBoundary';
import NetworkStatus from './NetworkStatus';
import { Toaster } from '@/components/ui/sonner';
import { useEffect, useState } from 'react';

export default function Layout() {
  const location = useLocation();
  // Track latest pathname applied to the DOM. When it differs from the current
  // location, we render with opacity 0 for a brief frame then schedule a flip
  // back via an effect — avoiding synchronous setState inside the effect body.
  const [appliedPath, setAppliedPath] = useState(location.pathname);
  const isTransitioning = appliedPath !== location.pathname;

  useEffect(() => {
    if (appliedPath === location.pathname) return;
    const timer = setTimeout(() => setAppliedPath(location.pathname), 50);
    return () => clearTimeout(timer);
  }, [location.pathname, appliedPath]);

  return (
    <SidebarProvider>
      <AppSidebar />
      <SidebarInset className="min-h-[100dvh]">
        <NetworkStatus />
        {/* Mobile header */}
        <header className="sticky top-0 z-30 flex h-14 items-center border-b border-border/60 bg-background/80 backdrop-blur-md px-4 pt-[env(safe-area-inset-top)] lg:hidden">
          <Link to="/" className="flex items-center gap-2.5">
            <div className="w-7 h-7 rounded-lg bg-primary flex items-center justify-center">
              <span className="text-primary-foreground font-bold text-xs">L</span>
            </div>
            <span className="font-semibold text-base text-foreground select-none tracking-tight">Lettura</span>
          </Link>
        </header>
        <main className={`mx-auto w-full px-4 py-5 pb-28 sm:pb-6 lg:py-8 lg:max-w-3xl transition-opacity duration-200 ${isTransitioning ? 'opacity-0' : 'opacity-100'}`}>
          <ErrorBoundary level="page">
            <Outlet />
          </ErrorBoundary>
        </main>
      </SidebarInset>
      <MobileBottomNav />
      <Toaster
        richColors
        position="top-center"
        toastOptions={{
          className: 'text-sm font-medium',
        }}
      />
    </SidebarProvider>
  );
}
