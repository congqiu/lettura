import { BookOpen, Inbox, Star, StickyNote, Tag, SearchX } from 'lucide-react';
import { Button } from '@/components/ui/button';

const iconMap: Record<string, React.ComponentType<{ size?: number; className?: string }>> = {
  book: BookOpen,
  inbox: Inbox,
  star: Star,
  note: StickyNote,
  tag: Tag,
  search: SearchX,
};

interface Props {
  icon?: string;
  title: string;
  description?: string;
  action?: { label: string; onClick: () => void };
}

export default function EmptyState({ icon = 'inbox', title, description, action }: Props) {
  const Icon = iconMap[icon] || Inbox;
  return (
    <div className="flex flex-col items-center justify-center py-16 sm:py-20 text-center animate-fade-in-up">
      <div className="w-14 h-14 rounded-2xl bg-secondary flex items-center justify-center mb-5">
        <Icon size={26} className="text-muted-foreground/50" />
      </div>
      <h3 className="font-semibold text-base text-foreground mb-1.5">{title}</h3>
      {description && (
        <p className="text-sm text-muted-foreground mb-5 max-w-[260px]">{description}</p>
      )}
      {action && (
        <Button variant="outline" onClick={action.onClick} className="rounded-lg">
          {action.label}
        </Button>
      )}
    </div>
  );
}
