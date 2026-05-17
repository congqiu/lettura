import { useState, useCallback, type ComponentType } from 'react';
import { useMutation } from '@tanstack/react-query';
import { apiPostRaw, apiGet } from '@/api/client';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Upload, Download, FileJson, Bookmark, Database, AlertCircle, CheckCircle2 } from 'lucide-react';

type ImportType = 'wallabag' | 'browser' | 'lettura';
type ExportScope = 'all' | 'unread' | 'archived' | 'starred';

interface PreviewInfo {
  total: number;
  valid: number;
  invalid: number;
  details?: string;
  /** If true, the file is NDJSON admin backup format — cannot be imported here */
  ndjsonFormat?: boolean;
}

const EXPORT_SCOPE_LABELS: Record<ExportScope, string> = {
  all: '全部数据',
  unread: '未读（收件箱）',
  archived: '已归档',
  starred: '收藏',
};

const IMPORT_TABS: ReadonlyArray<{
  value: ImportType;
  label: string;
  Icon: ComponentType<{ size?: number | string }>;
}> = [
  { value: 'wallabag', label: 'Wallabag JSON', Icon: FileJson },
  { value: 'browser', label: '浏览器书签 HTML', Icon: Bookmark },
  { value: 'lettura', label: 'Lettura 备份', Icon: Database },
];

function parsePreviewFromText(text: string, type: ImportType): PreviewInfo {
  if (type === 'wallabag') {
    try {
      const data = JSON.parse(text);
      const arr = Array.isArray(data) ? data : [];
      let valid = 0;
      let invalid = 0;
      for (const item of arr) {
        const url = item?.url;
        if (typeof url === 'string' && (url.trim().startsWith('http://') || url.trim().startsWith('https://'))) {
          valid++;
        } else {
          invalid++;
        }
      }
      return { total: arr.length, valid, invalid };
    } catch {
      return { total: 0, valid: 0, invalid: 0 };
    }
  }
  if (type === 'browser') {
    const parser = new DOMParser();
    const doc = parser.parseFromString(text, 'text/html');
    const links = Array.from(doc.querySelectorAll('a[href]')).filter((a) => {
      const href = a.getAttribute('href') || '';
      return href.startsWith('http://') || href.startsWith('https://');
    });
    return { total: links.length, valid: links.length, invalid: 0 };
  }
  // lettura — supports both legacy JSON bundle and NDJSON format
  try {
    const trimmed = text.trimStart();
    // NDJSON: first line is a metadata object with "type":"metadata"
    if (trimmed.startsWith('{')) {
      const firstLine = trimmed.split('\n')[0] ?? '';
      try {
        const firstObj = JSON.parse(firstLine);
        if (firstObj.type === 'metadata') {
          // Parse NDJSON format
          let entries = 0, tags = 0, annotations = 0, memos = 0;
          for (const line of trimmed.split('\n')) {
            const s = line.trim();
            if (!s) continue;
            try {
              const obj = JSON.parse(s);
              if (obj.type === 'entry') entries++;
              else if (obj.type === 'tag') tags++;
              else if (obj.type === 'annotation') annotations++;
              else if (obj.type === 'memo') memos++;
            } catch { /* skip malformed lines */ }
          }
          const details = `包含 ${entries} 篇文章${tags > 0 ? `、${tags} 个标签` : ''}${annotations > 0 ? `、${annotations} 条批注` : ''}${memos > 0 ? `、${memos} 条备忘录` : ''}。此为管理员备份格式（NDJSON），需通过管理员恢复功能导入`;
          return { total: entries, valid: 0, invalid: entries, details, ndjsonFormat: true };
        }
      } catch { /* not NDJSON, fall through */ }
    }
    // Legacy JSON bundle format
    const data = JSON.parse(text);
    const entries = Array.isArray(data?.entries) ? data.entries : [];
    const tagCount = Array.isArray(data?.tags) ? data.tags.length : 0;
    const annotationCount = Array.isArray(data?.annotations) ? data.annotations.length : 0;
    const memoCount = Array.isArray(data?.memos) ? data.memos.length : 0;
    const details = `包含 ${entries.length} 篇文章${tagCount > 0 ? `、${tagCount} 个标签` : ''}${annotationCount > 0 ? `、${annotationCount} 条批注` : ''}${memoCount > 0 ? `、${memoCount} 条备忘录` : ''}`;
    return { total: entries.length, valid: entries.length, invalid: 0, details };
  } catch {
    return { total: 0, valid: 0, invalid: 0 };
  }
}

interface TabButtonProps {
  value: ImportType;
  active: ImportType;
  label: string;
  Icon: ComponentType<{ size?: number | string }>;
  onSelect: (value: ImportType) => void;
}

function TabButton({ value, active, label, Icon, onSelect }: TabButtonProps) {
  const isActive = active === value;
  return (
    <button
      onClick={() => onSelect(value)}
      className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
        isActive
          ? 'bg-background text-foreground shadow-sm'
          : 'text-muted-foreground hover:text-foreground'
      }`}
    >
      <Icon size={14} />
      {label}
    </button>
  );
}

export default function DataPanel() {
  const [importType, setImportType] = useState<ImportType>('wallabag');
  const [importFile, setImportFile] = useState<File | null>(null);
  // Cache the file's text so handleImport doesn't re-read large backup files
  // (lettura exports can be tens of MB) — reading once also keeps the main
  // thread responsive during preview parsing.
  const [importText, setImportText] = useState<string | null>(null);
  const [preview, setPreview] = useState<PreviewInfo | null>(null);
  const [importResult, setImportResult] = useState<{
    message: string;
    type: 'success' | 'error';
  } | null>(null);

  const [exportScope, setExportScope] = useState<ExportScope>('all');
  const [fileInputKey, setFileInputKey] = useState(0);

  const handleSelectType = useCallback((value: ImportType) => {
    setImportType(value);
    setImportFile(null);
    setImportText(null);
    setPreview(null);
    setImportResult(null);
  }, []);

  const handleFileChange = useCallback(
    async (file: File | null) => {
      setImportFile(file);
      setImportResult(null);
      if (!file) {
        setImportText(null);
        setPreview(null);
        return;
      }
      const text = await file.text();
      setImportText(text);
      setPreview(parsePreviewFromText(text, importType));
    },
    [importType]
  );

  const buildImportMessage = (data: { imported: number; skipped: number; total?: number }, unit: string, label: string) => {
    if (data.imported === 0 && data.skipped > 0) {
      return `该${label}中的所有${unit}均已存在，无需重复导入（共 ${data.total ?? data.skipped} ${unit}）`;
    }
    if (data.imported > 0 && data.skipped > 0) {
      return `导入成功：${data.imported} ${unit}已导入，${data.skipped} ${unit}已存在被自动跳过`;
    }
    return `导入成功：${data.imported} ${unit}已导入`;
  };

  const onImportSuccess = (message: string) => {
    setImportResult({ message, type: 'success' });
    setImportFile(null);
    setImportText(null);
    setPreview(null);
    setFileInputKey((k) => k + 1);
  };

  const onImportError = (err: unknown) => {
    if (err instanceof Error && 'body' in err) {
      const body = (err as { body: { message?: string; error?: string } }).body;
      const msg = body?.message || body?.error || '导入失败，请检查文件格式';
      setImportResult({ message: msg, type: 'error' });
    } else {
      setImportResult({ message: '导入失败，请检查文件格式', type: 'error' });
    }
  };

  const importWallabag = useMutation({
    mutationFn: async (text: string) => {
      return apiPostRaw<{ imported: number; skipped: number; total: number }>('/import/wallabag', text, { 'Content-Type': 'application/json' });
    },
    onSuccess: (data) => onImportSuccess(buildImportMessage(data, '篇', '文件')),
    onError: onImportError,
  });

  const importBrowser = useMutation({
    mutationFn: async (text: string) => {
      return apiPostRaw<{ imported: number; skipped: number; total: number }>('/import/browser', text, { 'Content-Type': 'text/html' });
    },
    onSuccess: (data) => onImportSuccess(buildImportMessage(data, '条书签', '文件')),
    onError: onImportError,
  });

  const importLettura = useMutation({
    mutationFn: async (text: string) => {
      return apiPostRaw<{ imported: number; skipped: number; total: number }>('/import/lettura', text, { 'Content-Type': 'application/json' });
    },
    onSuccess: (data) => onImportSuccess(buildImportMessage(data, '篇', '备份')),
    onError: onImportError,
  });

  const handleImport = () => {
    if (!importFile || !importText) return;
    setImportResult(null);
    if (importType === 'wallabag') {
      importWallabag.mutate(importText);
    } else if (importType === 'browser') {
      importBrowser.mutate(importText);
    } else {
      importLettura.mutate(importText);
    }
  };

  const isImporting = importWallabag.isPending || importBrowser.isPending || importLettura.isPending;

  const getFileAccept = () => {
    if (importType === 'wallabag' || importType === 'lettura') return '.json,.ndjson';
    return '.html,.htm';
  };

  const getFileLabel = () => {
    if (importType === 'wallabag') return 'Wallabag JSON 文件';
    if (importType === 'browser') return '浏览器书签 HTML 文件';
    return 'Lettura 备份 JSON 文件';
  };

  const getFileHint = () => {
    if (importType === 'wallabag') return '选择从 Wallabag 导出的 JSON 文件';
    if (importType === 'browser') return '选择从 Chrome / Firefox / Safari 导出的书签 HTML 文件';
    return '选择从 Lettura 导出的备份 JSON 文件，支持跨账号迁移';
  };

  const getPreviewErrorText = () => {
    if (importType === 'wallabag') return '无法解析该文件，请确认是有效的 Wallabag JSON 格式';
    if (importType === 'browser') return '无法解析该文件，请确认是有效的书签 HTML 格式';
    return '无法解析该文件，请确认是有效的 Lettura 备份 JSON 格式';
  };

  const exportAll = useMutation({
    mutationFn: async (scope: ExportScope) => {
      const data = await apiGet('/export', { scope });
      const filename = `lettura-export-${scope}-${new Date().toISOString().slice(0, 10)}.json`;
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = filename;
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
          {/* Import type selector */}
          <div className="flex gap-1 p-1 bg-muted/50 rounded-lg w-fit">
            {IMPORT_TABS.map((tab) => (
              <TabButton
                key={tab.value}
                value={tab.value}
                active={importType}
                label={tab.label}
                Icon={tab.Icon}
                onSelect={handleSelectType}
              />
            ))}
          </div>

          <div>
            <label className="text-sm font-medium block mb-1.5">{getFileLabel()}</label>
            <p className="text-xs text-muted-foreground mb-3">{getFileHint()}</p>
            <div className="flex items-center gap-2 flex-wrap">
              <Input
                key={fileInputKey}
                type="file"
                accept={getFileAccept()}
                onChange={(e) => handleFileChange(e.target.files?.[0] ?? null)}
                className="text-sm rounded-lg h-9 flex-1 min-w-[200px]"
              />
              <Button
                onClick={handleImport}
                disabled={!importFile || !importText || isImporting || (preview !== null && preview.valid === 0) || (preview?.ndjsonFormat ?? false)}
                className="rounded-lg h-9"
              >
                {isImporting ? '导入中...' : '确认导入'}
              </Button>
            </div>
          </div>

          {/* Preview */}
          {preview && preview.total > 0 && (
            <div className="flex items-start gap-2 text-sm bg-muted/40 rounded-lg p-3">
              <AlertCircle size={15} className="text-primary mt-0.5 shrink-0" />
              <div className="space-y-0.5">
                <p>
                  共解析到 <span className="font-semibold">{preview.total}</span> 条记录
                </p>
                {preview.details && (
                  <p className="text-muted-foreground">{preview.details}</p>
                )}
                <p className="text-muted-foreground">
                  有效：{preview.valid} 条
                  {preview.invalid > 0 && `，无效（缺少 URL）：${preview.invalid} 条`}
                  {importType !== 'browser' && preview.valid > 0 && '，已存在的将自动跳过'}
                </p>
              </div>
            </div>
          )}

          {/* Preview error */}
          {preview && preview.total === 0 && importFile && (
            <div className="flex items-start gap-2 text-sm bg-destructive/10 rounded-lg p-3">
              <AlertCircle size={15} className="text-destructive mt-0.5 shrink-0" />
              <p className="text-destructive">{getPreviewErrorText()}</p>
            </div>
          )}

          {/* Result */}
          {importResult && (
            <div
              className={`flex items-start gap-2 text-sm rounded-lg p-3 ${
                importResult.type === 'success'
                  ? 'bg-success/10 text-success'
                  : 'bg-destructive/10 text-destructive'
              }`}
            >
              {importResult.type === 'success' ? (
                <CheckCircle2 size={15} className="text-success mt-0.5 shrink-0" />
              ) : (
                <AlertCircle size={15} className="text-destructive mt-0.5 shrink-0" />
              )}
              <p>{importResult.message}</p>
            </div>
          )}
        </div>
      </section>

      {/* Export */}
      <section>
        <div className="flex items-center gap-2 mb-4">
          <Download size={17} className="text-muted-foreground/60" />
          <h3 className="text-title font-semibold">导出</h3>
        </div>
        <div className="bg-card border border-border/60 rounded-xl p-5 space-y-4">
          <p className="text-sm text-muted-foreground">
            导出你的全部数据为 JSON 格式，包含文章、标签、批注、备忘录、标签规则、站点规则及其关联关系。
          </p>

          <div className="flex flex-col sm:flex-row items-start sm:items-center gap-3">
            <div className="flex items-center gap-2">
              <label className="text-sm font-medium whitespace-nowrap">导出范围</label>
              <select
                value={exportScope}
                onChange={(e) => setExportScope(e.target.value as ExportScope)}
                className="h-9 rounded-lg border border-input bg-background px-3 py-1 text-sm shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
              >
                {Object.entries(EXPORT_SCOPE_LABELS).map(([value, label]) => (
                  <option key={value} value={value}>
                    {label}
                  </option>
                ))}
              </select>
            </div>
            <Button
              onClick={() => exportAll.mutate(exportScope)}
              disabled={exportAll.isPending}
              variant="outline"
              className="rounded-lg h-9"
            >
              <FileJson size={15} className="mr-2" />
              {exportAll.isPending ? '导出中...' : '导出数据'}
            </Button>
          </div>
        </div>
      </section>
    </div>
  );
}
