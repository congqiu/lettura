import { AlertTriangle, RefreshCw } from 'lucide-react';

interface Props {
  message?: string;
  onRetry?: () => void;
}

export default function ErrorState({ message = '加载失败', onRetry }: Props) {
  return (
    <div className="flex flex-col items-center justify-center py-16 text-center">
      <div className="w-12 h-12 rounded-full bg-red-100 dark:bg-red-900/30 flex items-center justify-center mb-3">
        <AlertTriangle size={24} className="text-red-500 dark:text-red-400" />
      </div>
      <p className="text-sm text-gray-600 dark:text-gray-400 mb-3">{message}</p>
      {onRetry && (
        <button
          onClick={onRetry}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm text-blue-600 dark:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-900/20 rounded-lg transition-colors"
        >
          <RefreshCw size={14} />
          重试
        </button>
      )}
    </div>
  );
}
