import { BookOpen, Inbox, Star } from 'lucide-react';
import { Button } from '@/components/ui/button';

const iconMap: Record<string, React.ComponentType<{ size?: number; className?: string }>> = {
  book: BookOpen,
  inbox: Inbox,
  star: Star,
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
    <div className="flex flex-col items-center justify-center py-16 text-center">
      <div className="w-12 h-12 rounded-full bg-secondary flex items-center justify-center mb-4">
        <Icon size={24} className="text-muted-foreground" />
      </div>
      <h3 className="font-semibold text-lg mb-1">{title}</h3>
      {description && <p className="text-sm text-muted-foreground mb-4">{description}</p>}
      {action && <Button variant="outline" onClick={action.onClick}>{action.label}</Button>}
    </div>
  );
}