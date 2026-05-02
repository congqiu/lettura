import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import api from '../api/client';
import { fetchTagStats, renameTag, deleteTag } from '../api/tags';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import TokensPanel from '../components/settings/TokensPanel';
import { Pencil, Trash2 } from 'lucide-react';
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent,
  AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog';
import { toast } from 'sonner';

export default function SettingsPage() {
  const [importFile, setImportFile] = useState<File | null>(null);
  const [importResult, setImportResult] = useState('');
  const [editingTagId, setEditingTagId] = useState<string | null>(null);
  const [editingLabel, setEditingLabel] = useState('');
  const [deleteTarget, setDeleteTarget] = useState<{ id: string; label: string } | null>(null);
  const qc = useQueryClient();

  const { data: tagStats = [] } = useQuery({
    queryKey: ['tags', 'stats'],
    queryFn: fetchTagStats,
  });

  const importWallabag = useMutation({
    mutationFn: async (file: File) => {
      const text = await file.text();
      const data = JSON.parse(text);
      const res = await api.post('/import/wallabag', data);
      return res.data;
    },
    onSuccess: (data) => setImportResult(`导入 ${data.imported} 篇，跳过 ${data.skipped} 篇`),
    onError: () => setImportResult('导入失败'),
  });

  const exportAll = useMutation({
    mutationFn: async () => {
      const res = await api.get('/export');
      const blob = new Blob([JSON.stringify(res.data, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `lettura-export-${new Date().toISOString().slice(0, 10)}.json`;
      a.click();
      URL.revokeObjectURL(url);
    },
  });

  const renameMutation = useMutation({
    mutationFn: ({ id, label }: { id: string; label: string }) => renameTag(id, label),
    onSuccess: () => {
      setEditingTagId(null);
      qc.invalidateQueries({ queryKey: ['tags', 'stats'] });
      toast.success('标签已重命名');
    },
    onError: () => toast.error('重命名失败'),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteTag(id),
    onSuccess: () => {
      setDeleteTarget(null);
      qc.invalidateQueries({ queryKey: ['tags', 'stats'] });
      toast.success('标签已删除');
    },
    onError: () => toast.error('删除失败'),
  });

  const handleRenameKeyDown = (e: React.KeyboardEvent, tagId: string) => {
    if (e.key === 'Enter' && editingLabel.trim()) {
      renameMutation.mutate({ id: tagId, label: editingLabel.trim() });
    } else if (e.key === 'Escape') {
      setEditingTagId(null);
    }
  };

  return (
    <div className="max-w-2xl">
      <h2 className="text-xl font-semibold mb-6 text-foreground">设置</h2>

      <section className="mb-8">
        <h3 className="font-medium mb-3 text-foreground">标签管理</h3>
        {tagStats.length === 0 ? (
          <p className="text-sm text-muted-foreground">暂无标签</p>
        ) : (
          <div className="border border-border rounded-lg overflow-hidden">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border bg-muted/50">
                  <th className="text-left px-4 py-2 font-medium">标签名</th>
                  <th className="text-right px-4 py-2 font-medium">文章数</th>
                  <th className="text-right px-4 py-2 font-medium">操作</th>
                </tr>
              </thead>
              <tbody>
                {tagStats.map((tag) => (
                  <tr key={tag.id} className="border-b border-border last:border-b-0">
                    <td className="px-4 py-2">
                      {editingTagId === tag.id ? (
                        <Input
                          value={editingLabel}
                          onChange={(e) => setEditingLabel(e.target.value)}
                          onKeyDown={(e) => handleRenameKeyDown(e, tag.id)}
                          onBlur={() => setEditingTagId(null)}
                          className="h-7 text-sm"
                          autoFocus
                        />
                      ) : (
                        tag.label
                      )}
                    </td>
                    <td className="text-right px-4 py-2 text-muted-foreground">{tag.entry_count}</td>
                    <td className="text-right px-4 py-2">
                      <div className="flex items-center justify-end gap-1">
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-7 w-7 p-0"
                          onClick={() => {
                            setEditingTagId(tag.id);
                            setEditingLabel(tag.label);
                          }}
                        >
                          <Pencil size={14} />
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-7 w-7 p-0 hover:text-destructive"
                          onClick={() => setDeleteTarget({ id: tag.id, label: tag.label })}
                        >
                          <Trash2 size={14} />
                        </Button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      {/* Delete confirmation dialog */}
      <AlertDialog open={!!deleteTarget} onOpenChange={(open) => { if (!open) setDeleteTarget(null); }}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>确认删除标签</AlertDialogTitle>
            <AlertDialogDescription>
              确定要删除标签「{deleteTarget?.label}」吗？此操作将从所有文章中移除该标签，且不可撤销。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={() => deleteTarget && deleteMutation.mutate(deleteTarget.id)}
            >
              删除
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <section className="mb-8">
        <h3 className="font-medium mb-3 text-foreground">导入</h3>
        <div className="space-y-2">
          <label className="text-sm text-muted-foreground block mb-1">Wallabag JSON 导入</label>
          <div className="flex items-center gap-2">
            <Input
              type="file"
              accept=".json"
              onChange={(e) => setImportFile(e.target.files?.[0] ?? null)}
              className="text-sm"
            />
            <Button
              onClick={() => importFile && importWallabag.mutate(importFile)}
              disabled={!importFile || importWallabag.isPending}
            >
              {importWallabag.isPending ? '导入中...' : '导入'}
            </Button>
          </div>
          {importResult && <p className="text-sm text-green-600 dark:text-green-400 mt-1">{importResult}</p>}
        </div>
      </section>

      <section className="mb-8">
        <h3 className="font-medium mb-3 text-foreground">导出</h3>
        <Button
          onClick={() => exportAll.mutate()}
          disabled={exportAll.isPending}
          variant="outline"
        >
          {exportAll.isPending ? '导出中...' : '导出全部数据 (JSON)'}
        </Button>
      </section>

      <section className="mb-8">
        <h3 className="font-medium mb-3 text-foreground">API 令牌</h3>
        <p className="text-sm text-muted-foreground mb-4">
          管理用于 lettura-cli 或其他第三方客户端访问你数据的个人访问令牌。
        </p>
        <TokensPanel />
      </section>
    </div>
  );
}
