import { useState } from 'react';
import { useMutation } from '@tanstack/react-query';
import api from '@/api/client';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Upload, Download, FileJson } from 'lucide-react';

export default function DataPanel() {
  const [importFile, setImportFile] = useState<File | null>(null);
  const [importResult, setImportResult] = useState('');

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

  return (
    <div className="animate-fade-in space-y-8">
      {/* Import */}
      <section>
        <div className="flex items-center gap-2 mb-4">
          <Upload size={17} className="text-muted-foreground/60" />
          <h3 className="text-title font-semibold">导入</h3>
        </div>
        <div className="bg-card border border-border/60 rounded-xl p-5 space-y-4">
          <div>
            <label className="text-sm font-medium block mb-1.5">Wallabag JSON 导入</label>
            <p className="text-xs text-muted-foreground mb-3">选择从 Wallabag 导出的 JSON 文件进行导入</p>
            <div className="flex items-center gap-2 flex-wrap">
              <Input
                type="file"
                accept=".json"
                onChange={(e) => {
                  setImportFile(e.target.files?.[0] ?? null);
                  setImportResult('');
                }}
                className="text-sm rounded-lg h-9 flex-1 min-w-[200px]"
              />
              <Button
                onClick={() => importFile && importWallabag.mutate(importFile)}
                disabled={!importFile || importWallabag.isPending}
                className="rounded-lg h-9"
              >
                {importWallabag.isPending ? '导入中...' : '导入'}
              </Button>
            </div>
          </div>
          {importResult && (
            <p className={`text-sm font-medium ${importResult.startsWith('导入失败') ? 'text-destructive' : 'text-success'}`}>
              {importResult}
            </p>
          )}
        </div>
      </section>

      {/* Export */}
      <section>
        <div className="flex items-center gap-2 mb-4">
          <Download size={17} className="text-muted-foreground/60" />
          <h3 className="text-title font-semibold">导出</h3>
        </div>
        <div className="bg-card border border-border/60 rounded-xl p-5">
          <p className="text-sm text-muted-foreground mb-4">导出你的全部数据为 JSON 格式，可用于备份或迁移。</p>
          <Button
            onClick={() => exportAll.mutate()}
            disabled={exportAll.isPending}
            variant="outline"
            className="rounded-lg h-9"
          >
            <FileJson size={15} className="mr-2" />
            {exportAll.isPending ? '导出中...' : '导出全部数据 (JSON)'}
          </Button>
        </div>
      </section>
    </div>
  );
}
