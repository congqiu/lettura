import { Inbox, FileText, BookOpen, StickyNote } from 'lucide-react';
import type { ReactNode } from 'react';

interface Props {
  icon: 'inbox' | 'file' | 'book' | 'note';
  title: string;
  description?: string;
  action?: ReactNode;
}

const ICONS = {
  inbox: Inbox,
  file: FileText,
  book: BookOpen,
  note: StickyNote,
} as const;

export default function EmptyState({ icon, title, description, action }: Props) {
  const Icon = ICONS[icon];
  return (
    <div className="flex flex-col items-center justify-center py-16 text-center">
      <div className="w-12 h-12 rounded-full bg-gray-100 dark:bg-gray-800 flex items-center justify-center mb-3">
        <Icon size={24} className="text-gray-400 dark:text-gray-500" />
      </div>
      <p className="text-sm font-medium text-gray-600 dark:text-gray-400 mb-1">{title}</p>
      {description && (
        <p className="text-xs text-gray-400 dark:text-gray-500 max-w-xs">{description}</p>
      )}
      {action && <div className="mt-3">{action}</div>}
    </div>
  );
}
