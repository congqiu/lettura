import { useEffect, useState } from 'react';
import { CheckCircle, XCircle, AlertTriangle, X } from 'lucide-react';

export type ToastType = 'success' | 'error' | 'warning';

interface Toast {
  id: string;
  type: ToastType;
  message: string;
}

let listeners: ((toasts: Toast[]) => void)[] = [];
let toasts: Toast[] = [];

function emitChange() {
  listeners.forEach((l) => l([...toasts]));
}

let nextId = Date.now();
const timerMap = new Map<string, ReturnType<typeof setTimeout>>();

export function toast(type: ToastType, message: string) {
  const id = String(nextId++);
  toasts = [...toasts, { id, type, message }];
  const MAX_TOASTS = 5;
  while (toasts.length > MAX_TOASTS) {
    const oldest = toasts[0];
    const timer = timerMap.get(oldest.id);
    if (timer) { clearTimeout(timer); timerMap.delete(oldest.id); }
    toasts.shift();
  }
  emitChange();
  const timer = setTimeout(() => {
    timerMap.delete(id);
    toasts = toasts.filter((t) => t.id !== id);
    emitChange();
  }, 3000);
  timerMap.set(id, timer);
}

export default function ToastContainer() {
  const [local, setLocal] = useState<Toast[]>([]);

  useEffect(() => {
    const listener = (t: Toast[]) => setLocal(t);
    listeners.push(listener);
    return () => {
      listeners = listeners.filter((l) => l !== listener);
    };
  }, []);

  const dismiss = (id: string) => {
    const timer = timerMap.get(id);
    if (timer) {
      clearTimeout(timer);
      timerMap.delete(id);
    }
    toasts = toasts.filter((t) => t.id !== id);
    emitChange();
  };

  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2 pb-[env(safe-area-inset-bottom)]">
      {local.map((t) => (
        <div
          key={t.id}
          className={`flex items-center gap-2 px-4 py-2.5 rounded-lg shadow-lg text-sm font-medium animate-in slide-in-from-right-5 ${
            t.type === 'success'
              ? 'bg-green-50 dark:bg-green-900/80 text-green-700 dark:text-green-300 border border-green-200 dark:border-green-800'
              : t.type === 'warning'
              ? 'bg-yellow-50 dark:bg-yellow-900/80 text-yellow-700 dark:text-yellow-300 border border-yellow-200 dark:border-yellow-800'
              : 'bg-red-50 dark:bg-red-900/80 text-red-700 dark:text-red-300 border border-red-200 dark:border-red-800'
          }`}
        >
          {t.type === 'success' ? (
            <CheckCircle size={16} className="shrink-0" />
          ) : t.type === 'warning' ? (
            <AlertTriangle size={16} className="shrink-0" />
          ) : (
            <XCircle size={16} className="shrink-0" />
          )}
          <span>{t.message}</span>
          <button onClick={() => dismiss(t.id)} className="ml-1 hover:opacity-70">
            <X size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}
