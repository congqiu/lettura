import { AlertTriangle } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface Props {
  message?: string;
  onRetry?: () => void;
}

export default function ErrorState({ message = '加载失败', onRetry }: Props) {
  return (
    <div className="flex flex-col items-center justify-center py-12 text-center">
      <div className="w-12 h-12 rounded-full bg-destructive/10 flex items-center justify-center mb-4">
        <AlertTriangle size={24} className="text-destructive" />
      </div>
      <p className="text-sm text-muted-foreground mb-4">{message}</p>
      {onRetry && <Button variant="outline" size="sm" onClick={onRetry}>重试</Button>}
    </div>
  );
}