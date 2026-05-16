import { useState, useEffect } from 'react';
import { WifiOff, CheckCircle2 } from 'lucide-react';
import { cn } from '@/lib/utils';
import { getOfflineQueue, removeFromOfflineQueue } from '../utils/offlineQueue';
import { createEntry } from '../api/entries';
import { toast } from 'sonner';

export default function NetworkStatus() {
  const [isOnline, setIsOnline] = useState(navigator.onLine);
  const [showRecovered, setShowRecovered] = useState(false);

  useEffect(() => {
    const handleOnline = () => {
      setIsOnline(true);
      setShowRecovered(true);
      setTimeout(() => setShowRecovered(false), 2000);

      // Process offline save queue
      const queue = getOfflineQueue();
      if (queue.length > 0) {
        toast.info(`正在同步 ${queue.length} 个离线保存...`);
        queue.forEach(async (item) => {
          try {
            await createEntry(item.url);
            removeFromOfflineQueue(item.url);
          } catch {
            // Keep in queue if it fails again
          }
        });
      }
    };
    const handleOffline = () => {
      setIsOnline(false);
      setShowRecovered(false);
    };
    window.addEventListener('online', handleOnline);
    window.addEventListener('offline', handleOffline);
    return () => {
      window.removeEventListener('online', handleOnline);
      window.removeEventListener('offline', handleOffline);
    };
  }, []);

  if (isOnline && !showRecovered) return null;

  return (
    <div
      className={cn(
        'fixed top-0 left-0 right-0 z-[60] flex items-center justify-center gap-2 px-4 py-2.5 text-sm font-medium transition-all animate-fade-in',
        isOnline
          ? 'bg-success/90 text-success-foreground backdrop-blur-sm'
          : 'bg-destructive/90 text-destructive-foreground backdrop-blur-sm'
      )}
    >
      {isOnline ? <CheckCircle2 size={16} /> : <WifiOff size={16} />}
      {isOnline ? '网络已恢复' : '网络连接已断开，显示已缓存的内容'}
    </div>
  );
}
