import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { toast } from 'sonner';
import { KeyRound } from 'lucide-react';
import { Button } from '@/components/ui/button';
import ConfirmDialog from '@/components/ConfirmDialog';
import GenerateTokenDialog from './GenerateTokenDialog';
import { listTokens, deleteToken } from '@/api/tokens';

function formatDate(value: string | null | undefined): string {
  if (!value) return '—';
  return new Date(value).toLocaleString();
}

function formatExpiry(value: string | null | undefined): string {
  if (!value) return '永不过期';
  return new Date(value).toLocaleString();
}

function formatScope(scope: string | null | undefined): string {
  if (!scope) return '—';
  return scope === 'write' ? '读写' : '只读';
}

export default function TokensPanel() {
  const queryClient = useQueryClient();
  const [generateOpen, setGenerateOpen] = useState(false);
  const [revokeId, setRevokeId] = useState<string | null>(null);

  const { data: tokens, isLoading, isError, error, refetch } = useQuery({
    queryKey: ['tokens'],
    queryFn: listTokens,
  });

  const revokeMutation = useMutation({
    mutationFn: deleteToken,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tokens'] });
      toast.success('令牌已撤销');
      setRevokeId(null);
    },
    onError: () => {
      toast.error('撤销失败，请重试');
      setRevokeId(null);
    },
  });

  function handleRevokeConfirm() {
    if (revokeId) revokeMutation.mutate(revokeId);
  }

  if (isLoading) {
    return <p className="text-sm text-muted-foreground">加载中...</p>;
  }

  if (isError) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-center border border-dashed border-destructive/50 rounded-lg bg-destructive/5">
        <div className="w-12 h-12 rounded-full bg-destructive/10 flex items-center justify-center mb-4">
          <KeyRound size={24} className="text-destructive" />
        </div>
        <h4 className="font-semibold text-base text-destructive mb-1">加载令牌列表失败</h4>
        <p className="text-sm text-muted-foreground mb-4">
          {error?.message || '无法连接到服务器，请稍后重试'}
        </p>
        <Button variant="outline" onClick={() => refetch()}>
          重试
        </Button>
      </div>
    );
  }

  const isEmpty = !tokens || tokens.length === 0;

  return (
    <div className="space-y-4">
      {isEmpty ? (
        <div className="flex flex-col items-center justify-center py-12 text-center border border-dashed rounded-lg">
          <div className="w-12 h-12 rounded-full bg-secondary flex items-center justify-center mb-4">
            <KeyRound size={24} className="text-muted-foreground" />
          </div>
          <h4 className="font-semibold text-base mb-1">暂无 API 令牌</h4>
          <p className="text-sm text-muted-foreground mb-4">
            生成令牌以供 lettura-cli 或其他客户端访问你的数据
          </p>
          <Button variant="outline" onClick={() => setGenerateOpen(true)}>
            生成令牌
          </Button>
        </div>
      ) : (
        <>
          <div className="flex justify-end">
            <Button variant="outline" size="sm" onClick={() => setGenerateOpen(true)}>
              生成令牌
            </Button>
          </div>

          <div className="overflow-x-auto rounded-md border">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b bg-muted/50 text-muted-foreground">
                  <th scope="col" className="px-4 py-2.5 text-left font-medium">名称</th>
                  <th scope="col" className="px-4 py-2.5 text-left font-medium">前缀</th>
                  <th scope="col" className="px-4 py-2.5 text-left font-medium">权限</th>
                  <th scope="col" className="px-4 py-2.5 text-left font-medium">最后使用</th>
                  <th scope="col" className="px-4 py-2.5 text-left font-medium">过期时间</th>
                  <th scope="col" className="px-4 py-2.5 text-left font-medium">操作</th>
                </tr>
              </thead>
              <tbody>
                {tokens.map((token) => (
                  <tr key={token.id} className="border-b last:border-0 hover:bg-muted/30 transition-colors">
                    <td className="px-4 py-2.5 font-medium">{token.name}</td>
                    <td className="px-4 py-2.5 font-mono text-xs text-muted-foreground">
                      {token.token_prefix}…
                    </td>
                    <td className="px-4 py-2.5">{formatScope(token.scope)}</td>
                    <td className="px-4 py-2.5 text-muted-foreground">{formatDate(token.last_used_at)}</td>
                    <td className="px-4 py-2.5 text-muted-foreground">{formatExpiry(token.expires_at)}</td>
                    <td className="px-4 py-2.5">
                      <Button
                        variant="ghost"
                        size="sm"
                        className="text-destructive hover:text-destructive hover:bg-destructive/10"
                        onClick={() => setRevokeId(token.id)}
                      >
                        撤销
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}

      <GenerateTokenDialog
        open={generateOpen}
        onOpenChange={setGenerateOpen}
      />

      <ConfirmDialog
        open={revokeId !== null}
        title="撤销令牌"
        message="撤销此令牌？使用该令牌的会话将立即失效。"
        confirmText="撤销"
        variant="danger"
        onConfirm={handleRevokeConfirm}
        onCancel={() => setRevokeId(null)}
      />
    </div>
  );
}
