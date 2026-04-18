import { useState, useEffect } from 'react';
import { Wifi, WifiOff } from 'lucide-react';

export default function NetworkStatus() {
  const [isOnline, setIsOnline] = useState(navigator.onLine);
  const [showRecovered, setShowRecovered] = useState(false);

  useEffect(() => {
    const handleOnline = () => {
      setIsOnline(true);
      setShowRecovered(true);
      setTimeout(() => setShowRecovered(false), 2000);
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
      className={`fixed top-0 left-0 right-0 z-[60] flex items-center justify-center gap-2 px-4 py-2.5 text-sm font-medium transition-all ${
        isOnline
          ? 'bg-green-600 text-white'
          : 'bg-red-600 text-white'
      }`}
    >
      {isOnline ? <Wifi size={16} /> : <WifiOff size={16} />}
      {isOnline ? '网络已恢复' : '网络连接已断开，请检查网络后重试'}
    </div>
  );
}
