import { useEffect } from 'react';
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from '@/components/ui/dialog';
import {
  Command,
  CommandGroup,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from '@/components/ui/command';

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
      { keys: ['4'], desc: '便签' },
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

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) onClose(); }}>
      <DialogContent className="max-w-md max-h-[80vh] overflow-y-auto p-0">
        <DialogHeader className="px-4 py-3 border-b">
          <DialogTitle>键盘快捷键</DialogTitle>
        </DialogHeader>
        <Command className="[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-xs [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group-heading]]:text-muted-foreground [&_[cmdk-group]]:px-2 [&_[cmdk-item]]:px-2 [&_[cmdk-item]]:py-2">
          <CommandList>
            {SECTIONS.map((section, idx) => (
              <CommandGroup key={section.title} heading={section.title}>
                {section.shortcuts.map((shortcut) => (
                  <CommandItem key={shortcut.desc} className="flex items-center justify-between">
                    <span className="text-sm">{shortcut.desc}</span>
                    <div className="flex gap-1">
                      {shortcut.keys.map((key) => (
                        <CommandShortcut key={key}>{key}</CommandShortcut>
                      ))}
                    </div>
                  </CommandItem>
                ))}
                {idx < SECTIONS.length - 1 && <CommandSeparator />}
              </CommandGroup>
            ))}
          </CommandList>
        </Command>
      </DialogContent>
    </Dialog>
  );
}
