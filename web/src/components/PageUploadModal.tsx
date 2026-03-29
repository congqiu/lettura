import { useState, useRef, useCallback } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { uploadFiles, createPage } from '../api/pages';
import { Upload, X, Loader2, RefreshCw } from 'lucide-react';
import { toast } from './Toast';

interface Props {
  open: boolean;
  onClose: () => void;
}

export default function PageUploadModal({ open, onClose }: Props) {
  const qc = useQueryClient();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [uploadResult, setUploadResult] = useState<{
    upload_id: string;
    html_files: string[];
    default_entry: string;
    suggested_title: string;
    file_count: number;
  } | null>(null);
  const [entryFile, setEntryFile] = useState('');
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [password, setPassword] = useState('');
  const [dragOver, setDragOver] = useState(false);

  const handleFiles = useCallback(async (fileList: FileList | File[]) => {
    const arr = Array.from(fileList);
    try {
      const result = await uploadFiles(arr);
      setUploadResult(result);
      setEntryFile(result.default_entry);
      setTitle(result.suggested_title);
    } catch {
      toast('error', '上传失败');
    }
  }, []);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    if (e.dataTransfer.files.length > 0) {
      handleFiles(e.dataTransfer.files);
    }
  }, [handleFiles]);

  const generatePassword = () => {
    const chars = 'abcdefghijklmnopqrstuvwxyz0123456789';
    const pw = Array.from({ length: 8 }, () => chars[Math.floor(Math.random() * chars.length)]).join('');
    setPassword(pw);
  };

  const createMutation = useMutation({
    mutationFn: () => createPage({
      upload_id: uploadResult!.upload_id,
      entry_file: entryFile,
      title,
      description: description || undefined,
      password: password || undefined,
    }),
    onSuccess: (data) => {
      qc.invalidateQueries({ queryKey: ['pages'] });
      const url = `${window.location.origin}${data.url}`;
      navigator.clipboard.writeText(url);
      toast('success', `页面已发布，链接已复制: /p/${data.slug}`);
      handleClose();
    },
    onError: () => {
      toast('error', '创建失败');
    },
  });

  const handleClose = () => {
    setUploadResult(null);
    setEntryFile('');
    setTitle('');
    setDescription('');
    setPassword('');
    onClose();
  };

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div className="fixed inset-0 bg-black/40" onClick={handleClose} />
      <div className="relative bg-white dark:bg-gray-900 rounded-2xl shadow-2xl w-full max-w-lg max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-800">
          <h2 className="font-bold text-lg">上传页面</h2>
          <button onClick={handleClose} className="p-2 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-full">
            <X size={18} />
          </button>
        </div>
        <div className="p-4 space-y-4">
          {!uploadResult ? (
            <div
              onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
              onDragLeave={() => setDragOver(false)}
              onDrop={handleDrop}
              onClick={() => fileInputRef.current?.click()}
              className={`border-2 border-dashed rounded-xl p-8 text-center cursor-pointer transition-colors ${
                dragOver
                  ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/20'
                  : 'border-gray-300 dark:border-gray-700 hover:border-gray-400 dark:hover:border-gray-600'
              }`}
            >
              <Upload size={32} className="mx-auto text-gray-400 mb-3" />
              <p className="text-sm text-gray-600 dark:text-gray-400">
                拖拽文件到此处，或点击选择
              </p>
              <p className="text-xs text-gray-400 mt-1">
                支持 HTML / CSS / JS / 图片 / ZIP
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
            <>
              <div className="space-y-3">
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">入口文件</label>
                  {uploadResult.html_files.length > 1 ? (
                    <select
                      value={entryFile}
                      onChange={(e) => setEntryFile(e.target.value)}
                      className="w-full px-3 py-2 border border-gray-300 dark:border-gray-700 rounded-lg bg-white dark:bg-gray-800 text-sm"
                    >
                      {uploadResult.html_files.map(f => (
                        <option key={f} value={f}>{f}</option>
                      ))}
                    </select>
                  ) : (
                    <p className="text-sm text-gray-600 dark:text-gray-400 font-mono">{entryFile}</p>
                  )}
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">标题</label>
                  <input
                    type="text"
                    value={title}
                    onChange={(e) => setTitle(e.target.value)}
                    className="w-full px-3 py-2 border border-gray-300 dark:border-gray-700 rounded-lg bg-white dark:bg-gray-800 text-sm"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">描述（可选）</label>
                  <input
                    type="text"
                    value={description}
                    onChange={(e) => setDescription(e.target.value)}
                    className="w-full px-3 py-2 border border-gray-300 dark:border-gray-700 rounded-lg bg-white dark:bg-gray-800 text-sm"
                    placeholder="可选的页面描述"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">访问密码（可选）</label>
                  <div className="flex gap-2">
                    <input
                      type="text"
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                      className="flex-1 px-3 py-2 border border-gray-300 dark:border-gray-700 rounded-lg bg-white dark:bg-gray-800 text-sm font-mono"
                      placeholder="留空则无需密码"
                    />
                    <button
                      onClick={generatePassword}
                      className="px-3 py-2 text-sm border border-gray-300 dark:border-gray-700 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
                      title="自动生成密码"
                    >
                      <RefreshCw size={14} />
                    </button>
                  </div>
                </div>
                <p className="text-xs text-gray-400">{uploadResult.file_count} 个文件</p>
              </div>
              <button
                onClick={() => createMutation.mutate()}
                disabled={createMutation.isPending || !title}
                className="w-full py-2.5 bg-blue-600 text-white rounded-lg font-medium text-sm hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors flex items-center justify-center gap-2"
              >
                {createMutation.isPending && <Loader2 size={16} className="animate-spin" />}
                发布
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
