import { useNavigate } from 'react-router-dom';
import { X } from 'lucide-react';
import { cn } from '@/lib/utils';

interface TagBadgeProps {
  label: string;
  clickable?: boolean;
  onRemove?: () => void;
  variant?: 'default' | 'active';
}

export default function TagBadge({ label, clickable = true, onRemove, variant = 'default' }: TagBadgeProps) {
  const navigate = useNavigate();

  const handleClick = () => {
    if (clickable) {
      navigate(`/?tag=${encodeURIComponent(label)}`);
    }
  };

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 text-[12px] font-medium px-2.5 py-1 rounded-full transition-all duration-150',
        variant === 'active'
          ? 'bg-primary/10 text-primary'
          : 'bg-secondary text-secondary-foreground hover:bg-secondary/80',
        clickable && !onRemove && 'cursor-pointer',
        onRemove && 'pr-1.5'
      )}
      onClick={handleClick}
    >
      {label}
      {onRemove && (
        <button
          onClick={(e) => {
            e.stopPropagation();
            onRemove();
          }}
          className="inline-flex items-center justify-center w-4 h-4 rounded-full hover:bg-destructive/10 hover:text-destructive transition-colors ml-0.5"
        >
          <X size={10} strokeWidth={2.5} />
        </button>
      )}
    </span>
  );
}
