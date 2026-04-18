import { useEffect } from 'react';
import { X } from 'lucide-react';

interface Props {
  open: boolean;
  onClose: () => void;
}

const SECTIONS = [
  {
    title: '导航',
    shortcuts: [
      { keys: ['1'], desc: '未读列表' },
      { keys: ['2'], desc: '归档列表' },
      { keys: ['3'], desc: '收藏列表' },
      { keys: ['4'], desc: '收集箱' },
      { keys: ['h', '←'], desc: '返回上页' },
    ],
  },
  {
    title: '列表',
    shortcuts: [
      { keys: ['j'], desc: '下一篇' },
      { keys: ['k'], desc: '上一篇' },
      { keys: ['Enter', 'o'], desc: '打开文章' },
    ],
  },
  {
    title: '文章',
    shortcuts: [
      { keys: ['s'], desc: '收藏 / 取消收藏' },
      { keys: ['a'], desc: '归档 / 取消归档' },
      { keys: ['e'], desc: '编辑内容' },
    ],
  },
  {
    title: '全局',
    shortcuts: [
      { keys: ['?'], desc: '显示快捷键帮助' },
    ],
  },
];

export default function KeyboardShortcutsHelp({ open, onClose }: Props) {
  useEffect(() => {
    if (!open) return;
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div className="fixed inset-0 bg-black/40 backdrop-blur-sm" onClick={onClose} />
      <div className="relative bg-white dark:bg-gray-900 rounded-2xl shadow-2xl border border-gray-200 dark:border-gray-800 w-full max-w-md p-5 animate-in zoom-in-95 fade-in duration-150 max-h-[80vh] overflow-y-auto">
        <div className="flex items-center justify-between mb-4">
          <h2 className="font-semibold text-gray-900 dark:text-gray-100">键盘快捷键</h2>
          <button
            onClick={onClose}
            className="p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 rounded-full hover:bg-gray-100 dark:hover:bg-gray-800"
          >
            <X size={16} />
          </button>
        </div>

        <div className="space-y-5">
          {SECTIONS.map((section) => (
            <div key={section.title}>
              <h3 className="text-xs font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wider mb-2">
                {section.title}
              </h3>
              <div className="space-y-1.5">
                {section.shortcuts.map((shortcut) => (
                  <div key={shortcut.desc} className="flex items-center justify-between">
                    <span className="text-sm text-gray-700 dark:text-gray-300">{shortcut.desc}</span>
                    <div className="flex gap-1">
                      {shortcut.keys.map((key) => (
                        <kbd
                          key={key}
                          className="inline-flex items-center justify-center min-w-[24px] h-6 px-1.5 text-xs font-mono bg-gray-100 dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded text-gray-600 dark:text-gray-400"
                        >
                          {key}
                        </kbd>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
