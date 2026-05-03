import { useEffect, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { createEntry } from '../api/entries';
import { useAuthStore } from '../store/auth';
import { Button } from '@/components/ui/button';
import { Loader2, CheckCircle2, XCircle } from 'lucide-react';

const URL_REGEX = /https?:\/\/[^\s<>"{}|\\^`\[\]]+/;

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
  const [status, setStatus] = useState<'loading' | 'success' | 'duplicate' | 'error' | 'no-url'>('loading');
  const [errorMsg, setErrorMsg] = useState('');
  const [savedEntryId, setSavedEntryId] = useState('');
  const timerRef = useRef<number>();
  const mountedRef = useRef(true);

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

    // Clean up any stale share data from sessionStorage
    sessionStorage.removeItem(SHARE_STORAGE_KEY);

    const urlParam = searchParams.get('url');
    const textParam = searchParams.get('text');
    const url = extractUrl(urlParam, textParam);

    if (!url) {
      setStatus('no-url');
      return;
    }

    createEntry(url)
      .then((entry) => {
        if (!mountedRef.current) return;
        setSavedEntryId(entry.id);
        setStatus('success');
        timerRef.current = window.setTimeout(() => navigate(`/entry/${entry.id}`), 2000);
      })
      .catch((err: any) => {
        if (!mountedRef.current) return;
        if (err.response?.status === 409) {
          const existingId = err.response?.data?.id;
          if (existingId) {
            setSavedEntryId(existingId);
            setStatus('duplicate');
            timerRef.current = window.setTimeout(() => navigate(`/entry/${existingId}`), 2000);
          } else {
            setStatus('error');
            setErrorMsg('该链接已保存，但无法定位已有文章');
          }
        } else {
          setStatus('error');
          setErrorMsg(err.response?.data?.message || '保存失败');
        }
      });
  }, [isAuthenticated, searchParams, navigate]);

  return (
    <div className="min-h-screen flex items-center justify-center bg-background p-4">
      <div className="w-full max-w-sm p-8 bg-card border border-border rounded-xl shadow-sm text-center">
        {status === 'loading' && (
          <>
            <Loader2 size={32} className="animate-spin mx-auto mb-4 text-primary" />
            <p className="text-foreground">正在保存...</p>
          </>
        )}
        {status === 'success' && (
          <>
            <CheckCircle2 size={32} className="mx-auto mb-4 text-green-500" />
            <p className="text-foreground mb-2">已保存</p>
            <p className="text-sm text-muted-foreground">正在跳转到文章...</p>
          </>
        )}
        {status === 'duplicate' && (
          <>
            <CheckCircle2 size={32} className="mx-auto mb-4 text-amber-500" />
            <p className="text-foreground mb-2">该链接已保存</p>
            {savedEntryId && (
              <Button variant="outline" size="sm" onClick={() => { clearTimeout(timerRef.current); navigate(`/entry/${savedEntryId}`); }} className="mb-2">
                查看文章
              </Button>
            )}
            <p className="text-sm text-muted-foreground">正在跳转...</p>
          </>
        )}
        {status === 'error' && (
          <>
            <XCircle size={32} className="mx-auto mb-4 text-destructive" />
            <p className="text-foreground mb-2">保存失败</p>
            <p className="text-sm text-muted-foreground mb-4">{errorMsg}</p>
            <Button onClick={() => window.location.reload()}>重试</Button>
          </>
        )}
        {status === 'no-url' && (
          <>
            <XCircle size={32} className="mx-auto mb-4 text-muted-foreground" />
            <p className="text-foreground mb-2">未检测到链接</p>
            <p className="text-sm text-muted-foreground mb-4">请从浏览器分享菜单分享一个网页链接</p>
            <Button variant="outline" onClick={() => navigate('/')}>返回首页</Button>
          </>
        )}
      </div>
    </div>
  );
}