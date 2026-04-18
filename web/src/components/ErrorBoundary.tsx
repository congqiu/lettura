import { Component, type ReactNode } from 'react';
import { AlertTriangle } from 'lucide-react';

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
          <div className="w-14 h-14 rounded-full bg-red-100 dark:bg-red-900/30 flex items-center justify-center mx-auto mb-4">
            <AlertTriangle size={28} className="text-red-500 dark:text-red-400" />
          </div>
          <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-2">
            页面出了点问题
          </h2>
          <p className="text-sm text-gray-500 dark:text-gray-400 mb-5">
            {this.state.error?.message || '发生了未知错误'}
          </p>
          {isAppLevel ? (
            <button
              onClick={() => window.location.reload()}
              className="px-4 py-2 bg-blue-600 text-white text-sm rounded-lg hover:bg-blue-700 transition-colors"
            >
              重新加载
            </button>
          ) : (
            <a
              href="/"
              className="inline-block px-4 py-2 bg-blue-600 text-white text-sm rounded-lg hover:bg-blue-700 transition-colors"
            >
              回到首页
            </a>
          )}
        </div>
      </div>
    );
  }
}
