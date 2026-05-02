import { useNavigate } from 'react-router-dom';
import { Badge } from '@/components/ui/badge';

interface TagBadgeProps {
  label: string;
  clickable?: boolean;
  onRemove?: () => void;
}

export default function TagBadge({ label, clickable = true, onRemove }: TagBadgeProps) {
  const navigate = useNavigate();

  const handleClick = () => {
    if (clickable) {
      navigate(`/?tag=${encodeURIComponent(label)}`);
    }
  };

  return (
    <Badge
      variant="secondary"
      className={`flex items-center gap-1 ${clickable ? 'cursor-pointer hover:bg-secondary/80' : ''}`}
      onClick={handleClick}
    >
      {label}
      {onRemove && (
        <button
          onClick={(e) => {
            e.stopPropagation();
            onRemove();
          }}
          className="hover:text-destructive font-bold transition-colors leading-none"
        >
          &times;
        </button>
      )}
    </Badge>
  );
}
