import { AlertTriangle } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface Props {
  message?: string;
  onRetry?: () => void;
}

export default function ErrorState({ message = '加载失败', onRetry }: Props) {
  return (
    <div className="flex flex-col items-center justify-center py-16 sm:py-20 text-center animate-fade-in-up">
      <div className="w-14 h-14 rounded-2xl bg-destructive/8 flex items-center justify-center mb-5">
        <AlertTriangle size={26} className="text-destructive/70" />
      </div>
      <p className="text-sm text-muted-foreground mb-5">{message}</p>
      {onRetry && (
        <Button variant="outline" size="sm" onClick={onRetry} className="rounded-lg">
          重试
        </Button>
      )}
    </div>
  );
}
