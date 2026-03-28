import { useState } from 'react';
import { useMutation } from '@tanstack/react-query';
import api from '../api/client';

export default function SettingsPage() {
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
    <div className="max-w-2xl">
      <h2 className="text-xl font-semibold mb-6">设置</h2>

      <section className="mb-8">
        <h3 className="font-medium mb-3">导入</h3>
        <div className="space-y-2">
          <label className="text-sm text-gray-600 dark:text-gray-400 block mb-1">Wallabag JSON 导入</label>
          <div className="flex items-center gap-2">
            <input
              type="file"
              accept=".json"
              onChange={(e) => setImportFile(e.target.files?.[0] ?? null)}
              className="text-sm text-gray-600 dark:text-gray-400"
            />
            <button
              onClick={() => importFile && importWallabag.mutate(importFile)}
              disabled={!importFile || importWallabag.isPending}
              className="px-3 py-1 bg-blue-600 text-white text-sm rounded disabled:opacity-50"
            >
              {importWallabag.isPending ? '导入中...' : '导入'}
            </button>
          </div>
          {importResult && <p className="text-sm text-green-600 dark:text-green-400 mt-1">{importResult}</p>}
        </div>
      </section>

      <section className="mb-8">
        <h3 className="font-medium mb-3">导出</h3>
        <button
          onClick={() => exportAll.mutate()}
          disabled={exportAll.isPending}
          className="px-3 py-1.5 bg-gray-800 dark:bg-gray-700 text-white text-sm rounded hover:bg-gray-900 dark:hover:bg-gray-600 disabled:opacity-50"
        >
          {exportAll.isPending ? '导出中...' : '导出全部数据 (JSON)'}
        </button>
      </section>
    </div>
  );
}
