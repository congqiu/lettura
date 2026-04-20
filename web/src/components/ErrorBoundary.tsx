import { Component, type ReactNode } from 'react';
import { AlertTriangle } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface Props {
  children: ReactNode;
  /** 'app' = top-level (full reload), 'page' = page-level (navigate home) */
  level?: 'app' | 'page';
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export default class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error('ErrorBoundary caught:', error, info.componentStack);
  }

  render() {
    if (!this.state.hasError) {
      return this.props.children;
    }

    const isAppLevel = this.props.level === 'app';

    return (
      <div className="min-h-[300px] flex items-center justify-center p-8">
        <div className="text-center max-w-sm">
          <div className="w-14 h-14 rounded-full bg-destructive/10 dark:bg-destructive/20 flex items-center justify-center mx-auto mb-4">
            <AlertTriangle size={28} className="text-destructive" />
          </div>
          <h2 className="text-lg font-semibold text-foreground mb-2">
            页面出了点问题
          </h2>
          <p className="text-sm text-muted-foreground mb-5">
            {this.state.error?.message || '发生了未知错误'}
          </p>
          {isAppLevel ? (
            <Button onClick={() => window.location.reload()}>
              重新加载
            </Button>
          ) : (
            <a
              href="/"
              className="inline-block px-4 py-2 bg-primary text-primary-foreground text-sm rounded-lg hover:bg-primary/90 transition-colors"
            >
              回到首页
            </a>
          )}
        </div>
      </div>
    );
  }
}
