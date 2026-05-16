import { lazy, Suspense } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import Layout from './components/Layout';
import ProtectedRoute from './components/ProtectedRoute';
import ErrorBoundary from './components/ErrorBoundary';
import LoginPage from './pages/LoginPage';
import RegisterPage from './pages/RegisterPage';
import ShareTargetPage from './pages/ShareTargetPage';
import EntryListPage from './pages/EntryListPage';
import EntryDetailPage from './pages/EntryDetailPage';

const MemosPage = lazy(() => import('./pages/MemosPage'));
const PagesPage = lazy(() => import('./pages/PagesPage'));
const SettingsPage = lazy(() => import('./pages/SettingsPage'));
const AuditLogsPage = lazy(() => import('./pages/AuditLogsPage'));

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      gcTime: 5 * 60_000,
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});

function App() {
  return (
    <ErrorBoundary level="app">
      <QueryClientProvider client={queryClient}>
        <BrowserRouter>
          <Suspense fallback={<div className="p-8 text-center text-muted-foreground">Loading...</div>}>
            <Routes>
              <Route path="/login" element={<LoginPage />} />
              <Route path="/register" element={<RegisterPage />} />
              <Route path="/share-target" element={<ShareTargetPage />} />
              <Route
                path="/"
                element={
                  <ProtectedRoute>
                    <Layout />
                  </ProtectedRoute>
                }
              >
                <Route index element={<EntryListPage filter="unread" />} />
                <Route path="archived" element={<EntryListPage filter="archived" />} />
                <Route path="starred" element={<EntryListPage filter="starred" />} />
                <Route path="entry/:id" element={<EntryDetailPage />} />
                <Route path="memos" element={<MemosPage />} />
                <Route path="pages" element={<PagesPage />} />
                <Route path="audit-logs" element={<AuditLogsPage />} />
                <Route path="settings" element={<SettingsPage />} />
              </Route>
              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </Suspense>
        </BrowserRouter>
      </QueryClientProvider>
    </ErrorBoundary>
  );
}

export default App;
