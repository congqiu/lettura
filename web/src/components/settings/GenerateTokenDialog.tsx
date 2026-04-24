import { useState, useEffect } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { toast } from 'sonner';
import { Copy, Check } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { createToken } from '@/api/tokens';
import type { CreateTokenPayload } from '@/api/tokens';

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreated?: () => void;
}

type Phase = 'form' | 'success';

const SCOPE_OPTIONS = [
  { value: 'write', label: '写入 — 完全访问' },
  { value: 'read', label: '只读 — 仅读取数据' },
] as const;

const EXPIRY_OPTIONS = [
  { value: '', label: '永不过期' },
  { value: '30', label: '30 天' },
  { value: '90', label: '90 天' },
  { value: '365', label: '1 年' },
] as const;

export default function GenerateTokenDialog({ open, onOpenChange, onCreated }: Props) {
  const queryClient = useQueryClient();

  // Form fields
  const [name, setName] = useState('');
  const [scope, setScope] = useState<'write' | 'read'>('write');
  const [expiryDays, setExpiryDays] = useState<string>('');

  // Success state
  const [phase, setPhase] = useState<Phase>('form');
  const [createdToken, setCreatedToken] = useState('');
  const [copied, setCopied] = useState(false);

  const mutation = useMutation({
    mutationFn: (payload: CreateTokenPayload) => createToken(payload),
    onSuccess: (data) => {
      setCreatedToken(data.token);
      setPhase('success');
      queryClient.invalidateQueries({ queryKey: ['tokens'] });
      onCreated?.();
    },
    onError: () => toast.error('生成令牌失败，请重试'),
  });

  // Cleanup sensitive state on unmount
  useEffect(() => {
    return () => {
      setCreatedToken('');
      setName('');
      setCopied(false);
    };
  }, []);

  function handleGenerate() {
    if (!name.trim()) return;
    mutation.mutate({
      name: name.trim(),
      scope,
      expires_in_days: expiryDays === '' ? null : Number(expiryDays),
    });
  }

  function handleCopy() {
    navigator.clipboard.writeText(createdToken).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  }

  function handleClose() {
    // Clear all state before closing
    setPhase('form');
    setName('');
    setScope('write');
    setExpiryDays('');
    setCreatedToken('');
    setCopied(false);
    onOpenChange(false);
  }

  function handleOpenChange(v: boolean) {
    if (!v) {
      handleClose();
    } else {
      onOpenChange(true);
    }
  }

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent showCloseButton={phase === 'form'}>
        {phase === 'form' ? (
          <>
            <DialogHeader>
              <DialogTitle>生成 API 令牌</DialogTitle>
            </DialogHeader>

            <div className="space-y-4 py-2">
              <div className="space-y-1.5">
                <label className="text-sm font-medium text-foreground" htmlFor="token-name">
                  名称 <span className="text-destructive">*</span>
                </label>
                <Input
                  id="token-name"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  minLength={1}
                  maxLength={64}
                  placeholder="例如：lettura-cli"
                  onKeyDown={(e) => e.key === 'Enter' && handleGenerate()}
                />
              </div>

              <div className="space-y-1.5">
                <label className="text-sm font-medium text-foreground" htmlFor="token-scope">
                  权限
                </label>
                <select
                  id="token-scope"
                  value={scope}
                  onChange={(e) => setScope(e.target.value as 'write' | 'read')}
                  className="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                >
                  {SCOPE_OPTIONS.map((o) => (
                    <option key={o.value} value={o.value}>{o.label}</option>
                  ))}
                </select>
              </div>

              <div className="space-y-1.5">
                <label className="text-sm font-medium text-foreground" htmlFor="token-expiry">
                  过期时间
                </label>
                <select
                  id="token-expiry"
                  value={expiryDays}
                  onChange={(e) => setExpiryDays(e.target.value)}
                  className="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                >
                  {EXPIRY_OPTIONS.map((o) => (
                    <option key={o.value} value={o.value}>{o.label}</option>
                  ))}
                </select>
              </div>
            </div>

            <DialogFooter>
              <Button variant="outline" onClick={handleClose} disabled={mutation.isPending}>
                取消
              </Button>
              <Button
                onClick={handleGenerate}
                disabled={!name.trim() || mutation.isPending}
              >
                {mutation.isPending ? '生成中...' : '生成'}
              </Button>
            </DialogFooter>
          </>
        ) : (
          <>
            <DialogHeader>
              <DialogTitle>令牌已创建</DialogTitle>
            </DialogHeader>

            <div className="space-y-4 py-2">
              <p className="text-sm text-amber-600 dark:text-amber-400 font-medium">
                这是唯一一次看到完整令牌的机会 —— 请立即复制保存。
              </p>

              <div className="relative">
                <pre className="block w-full rounded-md border border-input bg-muted px-3 py-2 text-sm font-mono break-all whitespace-pre-wrap pr-10">
                  {createdToken}
                </pre>
                <button
                  onClick={handleCopy}
                  className="absolute top-2 right-2 p-1 rounded hover:bg-accent transition-colors"
                  title="复制令牌"
                >
                  {copied ? (
                    <Check size={16} className="text-green-600" />
                  ) : (
                    <Copy size={16} className="text-muted-foreground" />
                  )}
                </button>
              </div>
            </div>

            <DialogFooter>
              <Button onClick={handleClose}>完成</Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
