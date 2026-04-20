import { useState, useRef, useCallback } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { uploadFiles, updatePage, type PageSummary } from '../api/pages';
import { X, Loader2, RefreshCw, Upload } from 'lucide-react';
import { toast } from 'sonner';
import { Dialog, DialogContent, DialogHeader, DialogTitle } from './ui/dialog';
import { Button } from './ui/button';
import { Input } from './ui/input';

interface Props {
  page: PageSummary;
  open: boolean;
  onClose: () => void;
}

const EXPIRY_OPTIONS = [
  { label: '永久', value: '' },
  { label: '1 小时', value: '1h' },
  { label: '1 天', value: '1d' },
  { label: '7 天', value: '7d' },
  { label: '30 天', value: '30d' },
];

function computeExpiry(value: string): string | undefined {
  if (!value) return undefined;
  const now = new Date();
  const map: Record<string, number> = { '1h': 3600, '1d': 86400, '7d': 604800, '30d': 2592000 };
  const seconds = map[value];
  if (!seconds) return undefined;
  return new Date(now.getTime() + seconds * 1000).toISOString();
}

export default function PageEditModal({ page, open, onClose }: Props) {
  const qc = useQueryClient();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [title, setTitle] = useState(page.title);
  const [password, setPassword] = useState(page.password || '');
  const [expiry, setExpiry] = useState('');
  const [dragOver, setDragOver] = useState(false);
  const [uploadResult, setUploadResult] = useState<{
    upload_id: string;
    html_files: string[];
    default_entry: string;
    suggested_title: string;
    file_count: number;
  } | null>(null);
  const [entryFile, setEntryFile] = useState('');

  const handleFiles = useCallback(async (fileList: FileList | File[]) => {
    const arr = Array.from(fileList);
    const hasHtml = arr.some(f => f.name.toLowerCase().endsWith('.html') || f.name.toLowerCase().endsWith('.htm'));
    if (!hasHtml) {
      toast.error('当前只支持 HTML 页面，请至少上传一个 HTML 文件');
      return;
    }
    try {
      const result = await uploadFiles(arr);
      setUploadResult(result);
      setEntryFile(result.default_entry);
    } catch {
      toast.error('上传失败');
    }
  }, []);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    if (e.dataTransfer.files.length > 0) {
      handleFiles(e.dataTransfer.files);
    }
  }, [handleFiles]);

  const clearUpload = () => {
    setUploadResult(null);
    setEntryFile('');
  };

  const saveMutation = useMutation({
    mutationFn: () => updatePage(page.id, {
      title,
      password: password || '',
      expires_at: expiry === '' ? undefined : (expiry === '__clear__' ? null : computeExpiry(expiry)),
      upload_id: uploadResult?.upload_id || undefined,
      entry_file: uploadResult ? entryFile : undefined,
    }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['pages'] });
      toast.success('已保存');
      onClose();
    },
    onError: () => {
      toast.error('保存失败');
    },
  });

  const handleExpiryChange = (value: string) => {
    if (value === '') {
      setExpiry(page.expires_at ? '__clear__' : '');
    } else {
      setExpiry(value);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onClose}>
      <DialogContent className="sm:max-w-[480px] max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>编辑页面</DialogTitle>
        </DialogHeader>

        <div className="space-y-4">
          {!uploadResult ? (
            <div
              onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
              onDragLeave={() => setDragOver(false)}
              onDrop={handleDrop}
              onClick={() => fileInputRef.current?.click()}
              className={`border-2 border-dashed rounded-xl p-5 text-center cursor-pointer transition-colors ${
                dragOver
                  ? 'border-primary bg-primary/5'
                  : 'border-border hover:border-muted-foreground'
              }`}
            >
              <Upload size={24} className="mx-auto text-muted-foreground mb-2" />
              <p className="text-sm text-muted-foreground">
                拖拽文件替换，或点击选择
              </p>
              <p className="text-xs text-muted-foreground mt-1">
                当前 {page.file_count} 个文件 · 上传新文件将完全替换
              </p>
              <input
                ref={fileInputRef}
                type="file"
                multiple
                accept=".html,.css,.js,.zip,.png,.jpg,.jpeg,.gif,.svg,.webp"
                className="hidden"
                onChange={(e) => e.target.files && handleFiles(e.target.files)}
              />
            </div>
          ) : (
            <div className="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-xl p-3">
              <div className="flex items-center justify-between">
                <p className="text-sm text-green-700 dark:text-green-400 font-medium">
                  {uploadResult.file_count} 个文件已准备替换
                </p>
                <Button type="button" variant="ghost" size="sm" onClick={clearUpload} className="text-destructive hover:text-destructive">
                  取消替换
                </Button>
              </div>
              {uploadResult.html_files.length > 1 && (
                <div className="mt-2">
                  <label className="block text-xs text-muted-foreground mb-1">入口文件</label>
                  <select
                    value={entryFile}
                    onChange={(e) => setEntryFile(e.target.value)}
                    className="w-full px-2 py-1 text-sm border border-border rounded-lg bg-background"
                  >
                    {uploadResult.html_files.map(f => (
                      <option key={f} value={f}>{f}</option>
                    ))}
                  </select>
                </div>
              )}
            </div>
          )}

          <div>
            <label className="block text-sm font-medium text-foreground mb-1">标题</label>
            <Input
              type="text"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              maxLength={500}
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-foreground mb-1">
              访问密码 {page.has_password && <span className="text-amber-500 text-xs">(已设置)</span>}
            </label>
            <div className="flex gap-2">
              <div className="relative flex-1">
                <Input
                  type="text"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder="留空则无密码"
                  className="font-mono pr-8"
                />
                {password && (
                  <button
                    type="button"
                    onClick={() => setPassword('')}
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                  >
                    <X size={14} />
                  </button>
                )}
              </div>
              <Button
                type="button"
                variant="outline"
                onClick={() => {
                  const chars = 'abcdefghijklmnopqrstuvwxyz0123456789';
                  setPassword(Array.from({ length: 8 }, () => chars[Math.floor(Math.random() * chars.length)]).join(''));
                }}
                title="自动生成密码"
              >
                <RefreshCw size={14} />
              </Button>
            </div>
          </div>

          <div>
            <label className="block text-sm font-medium text-foreground mb-1">分享有效期</label>
            <select
              value={expiry || ''}
              onChange={(e) => handleExpiryChange(e.target.value)}
              className="w-full px-3 py-2 border border-border rounded-lg bg-background text-sm"
            >
              {page.expires_at && (
                <option value="">永久（清除当前有效期）</option>
              )}
              {!page.expires_at && (
                <option value="">永久（当前）</option>
              )}
              {EXPIRY_OPTIONS.slice(1).map((opt) => (
                <option key={opt.value} value={opt.value}>{opt.label}</option>
              ))}
            </select>
            {page.expires_at && expiry === '' && (
              <p className="text-xs text-muted-foreground mt-1">
                当前有效期至 {new Date(page.expires_at).toLocaleString()}
              </p>
            )}
          </div>
        </div>

        <div className="flex justify-end gap-3">
          <Button variant="outline" onClick={onClose}>
            取消
          </Button>
          <Button
            onClick={() => saveMutation.mutate()}
            disabled={!title.trim() || saveMutation.isPending}
          >
            {saveMutation.isPending && <Loader2 size={14} className="animate-spin" />}
            保存
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}