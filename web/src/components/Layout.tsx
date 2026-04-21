import { Outlet } from 'react-router-dom';
import { SidebarProvider, SidebarInset } from '@/components/ui/sidebar';
import { AppSidebar } from './layout/AppSidebar';
import { MobileBottomNav } from './layout/MobileBottomNav';
import ErrorBoundary from './ErrorBoundary';
import NetworkStatus from './NetworkStatus';
import { Toaster } from '@/components/ui/sonner';
import { Search } from 'lucide-react';
import { NavLink } from 'react-router-dom';

export default function Layout() {
  return (
    <SidebarProvider>
      <AppSidebar />
      <SidebarInset>
        <NetworkStatus />
        <header className="flex h-14 items-center gap-2 border-b border-border bg-card px-4 lg:hidden">
          <span className="font-bold text-lg text-primary select-none">Lettura</span>
          <div className="flex-1" />
          <NavLink to="/" className="p-2 text-muted-foreground hover:text-foreground">
            <Search size={20} />
          </NavLink>
        </header>
        <main className="mx-auto w-full px-4 py-6 pb-24 lg:pb-6 lg:max-w-3xl">
          <ErrorBoundary level="page">
            <Outlet />
          </ErrorBoundary>
        </main>
      </SidebarInset>
      <MobileBottomNav />
      <Toaster richColors position="top-center" />
    </SidebarProvider>
  );
}