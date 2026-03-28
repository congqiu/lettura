import { Component, type ReactNode } from 'react';

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
        <div className="text-center max-w-md">
          <h2 className="text-xl font-semibold text-gray-900 dark:text-gray-100 mb-2">
            出了点问题
          </h2>
          <p className="text-gray-600 dark:text-gray-400 mb-4 text-sm">
            {this.state.error?.message || '发生了未知错误'}
          </p>
          {isAppLevel ? (
            <button
              onClick={() => window.location.reload()}
              className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 transition-colors text-sm"
            >
              重新加载页面
            </button>
          ) : (
            <a
              href="/"
              className="inline-block px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 transition-colors text-sm"
            >
              回到首页
            </a>
          )}
        </div>
      </div>
    );
  }
}
