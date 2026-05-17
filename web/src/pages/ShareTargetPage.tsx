import { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { createEntry } from '../api/entries';
import { useAuthStore } from '../store/auth';
import { Button } from '@/components/ui/button';
import { Loader2, CheckCircle2, XCircle, ArrowRight } from 'lucide-react';
import { cn } from '@/lib/utils';

const URL_REGEX = /https?:\/\/[^\s<>"{}|\\^`[\]]+/;

// eslint-disable-next-line react-refresh/only-export-components
export function extractUrl(urlParam: string | null, textParam: string | null): string | null {
  if (urlParam && URL_REGEX.test(urlParam)) return urlParam.match(URL_REGEX)![0];
  if (textParam) {
    const match = textParam.match(URL_REGEX);
    if (match) return match[0];
  }
  return null;
}

const SHARE_STORAGE_KEY = 'lettura_share_redirect';

export default function ShareTargetPage() {
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const { isAuthenticated } = useAuthStore();
  // Async-driven status only — the no-url case is derived from `url` below,
  // so we never need to call setState synchronously from an effect to express
  // it. Default is null which means "derive from url" (loading or no-url).
  const [asyncStatus, setAsyncStatus] = useState<'success' | 'duplicate' | 'error' | null>(null);
  const [errorMsg, setErrorMsg] = useState('');
  const [savedEntryId, setSavedEntryId] = useState('');
  const timerRef = useRef<number>(0);
  const mountedRef = useRef(true);

  // Derive the extracted URL from search params synchronously. When it's
  // missing we render the no-url state without any setState ping-pong.
  const url = useMemo(() => {
    return extractUrl(searchParams.get('url'), searchParams.get('text'));
  }, [searchParams]);

  const status: 'loading' | 'success' | 'duplicate' | 'error' | 'no-url' =
    asyncStatus ?? (url ? 'loading' : 'no-url');

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      clearTimeout(timerRef.current);
    };
  }, []);

  useEffect(() => {
    if (!isAuthenticated) {
      const currentUrl = window.location.pathname + window.location.search;
      sessionStorage.setItem(SHARE_STORAGE_KEY, currentUrl);
      navigate('/login?redirect=' + encodeURIComponent(currentUrl));
      return;
    }

    sessionStorage.removeItem(SHARE_STORAGE_KEY);

    if (!url) return;

    createEntry(url)
      .then((entry) => {
        if (!mountedRef.current) return;
        setSavedEntryId(entry.id);
        setAsyncStatus('success');
        timerRef.current = window.setTimeout(() => navigate(`/entry/${entry.id}`), 2000);
      })
      .catch((err: unknown) => {
        if (!mountedRef.current) return;
        const response = (err as { response?: { status?: number; data?: { id?: string; message?: string } } })?.response;
        if (response?.status === 409) {
          const existingId = response.data?.id;
          if (existingId) {
            setSavedEntryId(existingId);
            setAsyncStatus('duplicate');
            timerRef.current = window.setTimeout(() => navigate(`/entry/${existingId}`), 2000);
          } else {
            setAsyncStatus('error');
            setErrorMsg('该链接已保存，但无法定位已有文章');
          }
        } else {
          setAsyncStatus('error');
          setErrorMsg(response?.data?.message || '保存失败');
        }
      });
  }, [isAuthenticated, navigate, url]);

  const statusConfig = {
    loading: {
      icon: <Loader2 size={40} className="animate-spin text-primary" />,
      title: '正在保存...',
      description: '请稍候',
      color: 'text-primary',
    },
    success: {
      icon: <CheckCircle2 size={40} className="text-success" />,
      title: '已保存',
      description: '正在跳转到文章...',
      color: 'text-success',
    },
    duplicate: {
      icon: <CheckCircle2 size={40} className="text-warning" />,
      title: '该链接已保存',
      description: '正在跳转...',
      color: 'text-warning',
    },
    error: {
      icon: <XCircle size={40} className="text-destructive" />,
      title: '保存失败',
      description: errorMsg,
      color: 'text-destructive',
    },
    'no-url': {
      icon: <XCircle size={40} className="text-muted-foreground" />,
      title: '未检测到链接',
      description: '请从浏览器分享菜单分享一个网页链接',
      color: 'text-muted-foreground',
    },
  };

  const config = statusConfig[status];

  return (
    <div className="min-h-[100dvh] flex items-center justify-center bg-background p-4">
      <div className="w-full max-w-sm text-center">
        {/* Logo */}
        <div className="flex flex-col items-center mb-8">
          <div className="w-12 h-12 rounded-2xl bg-primary flex items-center justify-center shadow-lg shadow-primary/20 mb-4">
            <span className="text-primary-foreground font-bold text-xl">L</span>
          </div>
        </div>

        <div className="bg-card border border-border/80 rounded-2xl shadow-sm p-8">
          <div className="flex flex-col items-center">
            <div className="mb-4">{config.icon}</div>
            <h2 className={cn('text-lg font-semibold mb-1', config.color)}>
              {config.title}
            </h2>
            <p className="text-sm text-muted-foreground">{config.description}</p>

            {status === 'duplicate' && savedEntryId && (
              <Button
                variant="outline"
                size="sm"
                onClick={() => { clearTimeout(timerRef.current); navigate(`/entry/${savedEntryId}`); }}
                className="mt-4 rounded-lg gap-1.5"
              >
                查看文章
                <ArrowRight size={14} />
              </Button>
            )}

            {status === 'error' && (
              <Button
                onClick={() => window.location.reload()}
                className="mt-4 rounded-lg"
              >
                重试
              </Button>
            )}

            {status === 'no-url' && (
              <Button
                variant="outline"
                onClick={() => navigate('/')}
                className="mt-4 rounded-lg"
              >
                返回首页
              </Button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
